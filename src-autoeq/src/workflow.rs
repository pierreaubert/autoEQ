//! Shared workflow helpers used by AutoEQ binaries
//!
//! This module centralizes the common pipeline steps for loading input data,
//! building target curves, preparing objective data, and running optimization.

use crate::{loss::ScoreLossData, optim, optim::ObjectiveData, read, Curve};
use ndarray::Array1;
use std::{collections::HashMap, error::Error};

/// Load the input curve from either local CSV (when no API params) or from
/// cached/API Plotly JSON for a given `speaker`/`version`/`measurement`.
///
/// Returns the main input `Curve` and optional CEA2034 spinorama curves when
/// the measurement requires them.
pub async fn load_input_curve(
    args: &crate::cli::Args,
) -> Result<(Curve, Option<HashMap<String, Curve>>), Box<dyn Error>> {
    let mut spin_data: Option<HashMap<String, Curve>> = None;

    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        let mut plot_data =
            read::fetch_measurement_plot_data(speaker, version, measurement).await?;
        let extracted_curve =
            read::extract_curve_by_name(&plot_data, measurement, &args.curve_name)?;
        // If EIR requested, fetch CEA2034 to extract spin data on the same grid
        if measurement == "Estimated In-Room Response" {
            plot_data = read::fetch_measurement_plot_data(speaker, version, "CEA2034").await?;
        }
        if measurement == "CEA2034" || measurement == "Estimated In-Room Response" {
            spin_data = Some(read::extract_cea2034_curves(
                &plot_data,
                "CEA2034",
                &extracted_curve.freq,
            )?);
        }
        extracted_curve
    } else {
        // No API params -> expect a CSV path
        let curve_path = args.curve.as_ref().ok_or(
            "Either --curve or all of --speaker, --version, and --measurement must be provided",
        )?;
        read::read_curve_from_csv(curve_path)?
    };

    Ok((input_curve, spin_data))
}

/// Build a target curve (and optional smoothed version) from CLI args and the input curve.
/// Returns (inverted_curve, smoothed_curve_opt).
pub fn build_target_curve(
    args: &crate::cli::Args,
    input_curve: &Curve,
) -> (Array1<f64>, Option<Array1<f64>>) {
    let base_target = if let Some(ref target_path) = args.target {
        let target_curve = read::read_curve_from_csv(target_path).unwrap();
        read::interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        match args.curve_name.as_str() {
            "Listening Window" => {
                let log_f_min = 1000.0_f64.log10();
                let log_f_max = 20000.0_f64.log10();
                let denom = log_f_max - log_f_min;
                Array1::from_shape_fn(input_curve.freq.len(), |i| {
                    let f_hz = input_curve.freq[i].max(1e-12);
                    let fl = f_hz.log10();
                    if fl < log_f_min {
                        0.0
                    } else if fl >= log_f_max {
                        -0.5
                    } else {
                        let t = (fl - log_f_min) / denom;
                        -0.5 * t
                    }
                })
            }
            "Sound Power" | "Early Reflections" | "Estimated In-Room Response" => {
                let slope =
                    crate::loss::curve_slope_per_octave_in_range(input_curve, 100.0, 10000.0)
                        .unwrap_or(-1.2)
                        - 0.2;
                let lo = 100.0_f64;
                let hi = 20000.0_f64;
                let hi_val = slope * (hi / lo).log2();
                Array1::from_shape_fn(input_curve.freq.len(), |i| {
                    let f = input_curve.freq[i].max(1e-12);
                    if f < lo {
                        0.0
                    } else if f >= hi {
                        hi_val
                    } else {
                        slope * (f / lo).log2()
                    }
                })
            }
            _ => Array1::zeros(input_curve.spl.len()),
        }
    };

    let target_curve = base_target - input_curve.spl.clone();
    let inverted_curve = read::clamp_positive_only(&target_curve, args.max_db);

    let mut smoothed_curve: Option<Array1<f64>> = None;
    if args.smooth {
        smoothed_curve = Some(read::smooth_one_over_n_octave(
            &input_curve.freq,
            &inverted_curve,
            args.smooth_n,
        ));
    }

    (inverted_curve, smoothed_curve)
}

/// Prepare the ObjectiveData and whether CEA2034-based scoring is active.
pub fn setup_objective_data(
    args: &crate::cli::Args,
    input_curve: &Curve,
    target_curve: &Array1<f64>,
    spin_data: &Option<HashMap<String, Curve>>,
) -> (ObjectiveData, bool) {
    let use_cea = (matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("CEA2034"))
        || matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("Estimated In-Room Response")))
        && args.speaker.is_some()
        && args.version.is_some()
        && spin_data.is_some();

    let score_data_opt = if use_cea {
        Some(ScoreLossData::new(spin_data.as_ref().unwrap()))
    } else {
        None
    };

    let objective_data = ObjectiveData {
        freqs: input_curve.freq.clone(),
        target_error: target_curve.clone(),
        srate: args.sample_rate,
        min_spacing_oct: args.min_spacing_oct,
        spacing_weight: args.spacing_weight,
        max_db: args.max_db,
        min_db: args.min_db,
        iir_hp_pk: args.iir_hp_pk,
        loss_type: args.loss,
        score_data: score_data_opt,
        // Penalties default to zero; configured per algorithm in optimize_filters
        penalty_w_ceiling: 0.0,
        penalty_w_spacing: 0.0,
        penalty_w_mingain: 0.0,
        // Integrality constraints - none for continuous optimization
        integrality: None,
    };

    (objective_data, use_cea)
}

/// Build optimization parameter bounds for the optimizer.
pub fn setup_bounds(args: &crate::cli::Args) -> (Vec<f64>, Vec<f64>) {
    let num_params = args.num_filters * 3;
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let spacing = 1.0;
    let gain_lower = -6.0 * args.max_db;
    let q_lower = args.min_q.max(0.1);
    let range = (args.max_freq.log10() - args.min_freq.log10()) / (args.num_filters as f64);
    for i in 0..args.num_filters {
        let f = args.min_freq.log10() + (i as f64) * range;
        let (mut f_low, mut f_high);
        if i == 0 {
            f_low = args.min_freq.log10();
            f_high = (f + spacing * range).min(args.max_freq.log10());
        } else if i == args.num_filters - 1 {
            f_low = (f - spacing * range).max(args.min_freq.log10());
            f_high = args.max_freq.log10();
        } else {
            f_low = (f - spacing * range).max(args.min_freq.log10());
            f_high = (f + spacing * range).min(args.max_freq.log10());
        }
        if i > 0 && f_low == lower_bounds[(i - 1) * 3] {
            f_low += 20f64.log10();
            f_high += 20f64.log10();
        }
        lower_bounds.extend_from_slice(&[f_low, q_lower, gain_lower]);
        upper_bounds.extend_from_slice(&[f_high, args.max_q, args.max_db]);
    }

    if args.iir_hp_pk {
        lower_bounds[0] = 20.0_f64.max(args.min_freq).log10();
        upper_bounds[0] = 120.0_f64.min(args.min_freq + 20.0).log10();
        lower_bounds[1] = 1.0;
        upper_bounds[1] = 1.5; // could be tuned as a function of max_db
        lower_bounds[2] = 0.0;
        upper_bounds[2] = 0.0;
    }

    (lower_bounds, upper_bounds)
}

/// Build an initial guess vector [f, Q, g] for each filter.
pub fn initial_guess(
    args: &crate::cli::Args,
    lower_bounds: &Vec<f64>,
    upper_bounds: &Vec<f64>,
) -> Vec<f64> {
    let mut x = vec![];
    for i in 0..args.num_filters {
        let freq = lower_bounds[i * 3].min(args.max_freq.log10());
        let q = (upper_bounds[i * 3 + 1] * lower_bounds[i * 3 + 1]).sqrt();
        let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
        let gain = sign * upper_bounds[i * 3 + 2].max(args.min_db);
        x.extend_from_slice(&[freq, q, gain]);
    }
    x
}

/// Run global (and optional local refine) optimization and return the parameter vector.
pub fn perform_optimization(
    args: &crate::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = setup_bounds(args);
    let mut x = initial_guess(args, &lower_bounds, &upper_bounds);

    let result = optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args.algo,
        args.population,
        args.maxeval,
        args,
    );

    match result {
        Ok((_status, _val)) => {}
        Err((e, _final_value)) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e).into());
        }
    };

    if args.refine {
        let local_result = optim::optimize_filters(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args.local_algo,
            args.population,
            args.maxeval,
            args,
        );
        match local_result {
            Ok((_local_status, _local_val)) => {}
            Err((e, _final_value)) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e).into());
            }
        }
    }

    Ok(x)
}

/// Run optimization with a DE progress callback (only used for AutoEQ DE).
pub fn perform_optimization_with_callback(
    args: &crate::cli::Args,
    objective_data: &ObjectiveData,
    callback: Box<dyn FnMut(&crate::de::DEIntermediate) -> crate::de::CallbackAction + Send>,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = setup_bounds(args);
    let mut x = initial_guess(args, &lower_bounds, &upper_bounds);

    // Only AutoEQ algorithms currently support callbacks
    let result = optim::optimize_filters_autoeq_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args.algo,
        args.population,
        args.maxeval,
        args,
        callback,
    );

    match result {
        Ok((_status, _val)) => {}
        Err((e, _final_value)) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e).into());
        }
    };

    if args.refine {
        let local_result = optim::optimize_filters(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args.local_algo,
            args.population,
            args.maxeval,
            args,
        );
        match local_result {
            Ok((_local_status, _local_val)) => {}
            Err((e, _final_value)) => {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e).into());
            }
        }
    }

    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Args;
    use clap::Parser;

    fn zero_curve(freqs: Vec<f64>) -> Curve {
        let n = freqs.len();
        Curve {
            freq: Array1::from(freqs),
            spl: Array1::zeros(n),
        }
    }

    #[test]
    fn build_target_curve_respects_smoothing_flag() {
        // Prepare a simple input curve and default args
        let mut args = Args::parse_from(["autoeq-test"]);
        args.curve_name = "Listening Window".to_string();
        let curve = zero_curve(vec![100.0, 1000.0, 10000.0, 20000.0]);

        // No smoothing
        args.smooth = false;
        let (_inv_no_smooth, smoothed_none) = super::build_target_curve(&args, &curve);
        assert!(smoothed_none.is_none());

        // With smoothing
        args.smooth = true;
        let (inv_smooth, smoothed_some) = super::build_target_curve(&args, &curve);
        assert!(smoothed_some.is_some());
        let s = smoothed_some.unwrap();
        assert_eq!(s.len(), inv_smooth.len());
    }

    #[test]
    fn setup_objective_data_sets_use_cea_when_expected() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.speaker = Some("spk".to_string());
        args.version = Some("v".to_string());
        args.measurement = Some("CEA2034".to_string());

        // Minimal input/target curves
        let input_curve = zero_curve(vec![100.0, 1000.0]);
        let target = Array1::zeros(input_curve.freq.len());

        // Build minimal spin data with required keys
        let mut spin: HashMap<String, Curve> = HashMap::new();
        for k in [
            "On Axis",
            "Listening Window",
            "Sound Power",
            "Estimated In-Room Response",
        ] {
            spin.insert(k.to_string(), zero_curve(vec![100.0, 1000.0]));
        }
        let spin_opt = Some(spin);

        let (obj, use_cea) = super::setup_objective_data(&args, &input_curve, &target, &spin_opt);
        assert!(use_cea);
        assert!(obj.score_data.is_some());

        // If measurement not CEA2034/EIR -> use_cea must be false
        let mut args2 = args.clone();
        args2.measurement = Some("On Axis".to_string());
        let (obj2, use_cea2) =
            super::setup_objective_data(&args2, &input_curve, &target, &spin_opt);
        assert!(!use_cea2);
        assert!(obj2.score_data.is_none());

        // If spin data missing -> use_cea must be false
        let (obj3, use_cea3) = super::setup_objective_data(&args, &input_curve, &target, &None);
        assert!(!use_cea3);
        assert!(obj3.score_data.is_none());
    }
}
