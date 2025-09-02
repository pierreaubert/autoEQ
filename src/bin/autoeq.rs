//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use autoeq::Curve;
use autoeq::iir;
use autoeq::loss::ScoreLossData;
use autoeq::optim;
use autoeq::optim::ObjectiveData;
use autoeq::plot;
use autoeq::read;
use autoeq::score;
use clap::Parser;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;

async fn load_input_curve(
    args: &autoeq::cli::Args,
) -> Result<(Curve, Option<HashMap<String, Curve>>), Box<dyn Error>> {
    let mut spin_data: Option<HashMap<String, Curve>> = None;

    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        let mut plot_data = read::fetch_measurement_plot_data(speaker, version, measurement).await?;
        let extracted_curve =
            read::extract_curve_by_name(&plot_data, measurement, &args.curve_name)?;
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
        // If no API parameters are provided, curve file must be provided
        let curve_path = args.curve.as_ref().ok_or(
            "Either --curve or all of --speaker, --version, and --measurement must be provided",
        )?;
        read::read_curve_from_csv(curve_path)?
    };

    Ok((input_curve, spin_data))
}

fn build_target_curve(
    args: &autoeq::cli::Args,
    input_curve: &Curve,
) -> (Array1<f64>, Option<Array1<f64>>) {
    let base_target = if let Some(ref target_path) = args.target {
        let target_curve = read::read_curve_from_csv(target_path).unwrap();
        println!(
            "✅ Loaded target curve with {} points.",
            target_curve.freq.len()
        );
        read::interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        match args.curve_name.as_str() {
            "Listening Window" => {
                // input_curve.freq is in Hz; use log10(Hz) to form a ramp from 1k to 20k
                let log_f_min = 1000.0_f64.log10();
                let log_f_max = 20000.0_f64.log10();
                let denom = log_f_max - log_f_min;
		println!("✅ Target curve is {}", args.curve_name);
                Array1::from_shape_fn(input_curve.freq.len(), |i| {
                    let f_hz = input_curve.freq[i].max(1e-12);
                    let fl = f_hz.log10();
                    if fl < log_f_min {
                        0.0
                    } else if fl >= log_f_max {
                        -0.5
                    } else {
                        let t = (fl - log_f_min) / denom; // 0..1 across 1k..20k in log10 domain
                        -0.5 * t
                    }
                })
            }
            "Sound Power" | "Early Reflections" | "Estimated In-Room Response" => {
                // 0 dB below 100 Hz; from 100..20k use straight line in log2(f) with slope
                // equal to the curve's slope (dB/octave) between 100..10k.
                let slope = autoeq::loss::curve_slope_per_octave_in_range(input_curve, 100.0, 10000.0)
                    .unwrap_or(-1.2)-0.2;
                let lo = 100.0_f64;
                let hi = 20000.0_f64;
                let hi_val = slope * (hi / lo).log2();
		println!("✅ Target curve is {} slope {:.2} db/oct", args.curve_name, slope);
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
            _ => {
		println!("✅ No target curve provided, using a flat 0 dB target.");
		Array1::zeros(input_curve.spl.len())
	    }
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

fn setup_objective_data(
    args: &autoeq::cli::Args,
    input_curve: &Curve,
    target_curve: &Array1<f64>,
    spin_data: &Option<HashMap<String, Curve>>,
) -> (ObjectiveData, bool) {
    let use_cea =
	(matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("CEA2034"))
	 ||
	 matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("Estimated In-Room Response"))
	 )
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
    };

    (objective_data, use_cea)
}

/// Build optimization parameter bounds for the optimizer
fn setup_bounds(args: &autoeq::cli::Args) -> (Vec<f64>, Vec<f64>) {
    let num_params = args.num_filters * 3;
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let spacing = 1.0;
    let gain_lower = -6.0 * args.max_db;
    let q_lower = args.min_q.max(0.1);
    let range = (args.max_freq.log10()-args.min_freq.log10()) / (args.num_filters as f64);
    for i in 0..args.num_filters {
	// start with a freq in range
	let f = args.min_freq.log10() + (i as f64) * range;
	// compute a low and high bounds
	let mut f_low : f64;
	let mut f_high : f64;
	if i == 0 {
	    // first one is bounded by min_freq and into the next band
	    f_low = args.min_freq.log10();
	    f_high = (f + spacing *range).min(args.max_freq.log10());
	} else if i == args.num_filters - 1 {
	    // last one same patter
	    f_low =  (f - spacing *range).max(args.min_freq.log10());
	    f_high = args.max_freq.log10();
	} else {
	    f_low  = (f - spacing *range).max(args.min_freq.log10());
	    f_high = (f + spacing *range).min(args.max_freq.log10());
	}
	if i>0 && f_low == lower_bounds[(i-1)*3] {
	    f_low += 20f64.log10();
	    f_high += 20f64.log10();
	}
	lower_bounds.extend_from_slice(&[f_low, q_lower, gain_lower]);
        upper_bounds.extend_from_slice(&[f_high, args.max_q, args.max_db]);
    }

    if args.iir_hp_pk {
        lower_bounds[0] = 20.0_f64.max(args.min_freq).log10();
        upper_bounds[0] = 120.0_f64.min(args.min_freq+20.0).log10();
        lower_bounds[1] = 1.0;
        upper_bounds[1] = 1.5; // should be computed as a function of max_db
        lower_bounds[2] = 0.0;
        upper_bounds[2] = 0.0;
    }

    // for i in 0..args.num_filters {
    // 	println!("{:2} Freq {:7.2} -- {:7.2}", i, 10f64.powf(lower_bounds[i*3]), 10f64.powf(upper_bounds[i*3]));
    // }

    // for i in 0..args.num_filters {
    // 	println!("{:2} Q    {:7.2} -- {:7.2}", i, lower_bounds[i*3+1], upper_bounds[i*3+1]);
    // }

    // for i in 0..args.num_filters {
    // 	println!("{:2} Gain {:7.2} -- {:7.2}", i, lower_bounds[i*3+2], upper_bounds[i*3+2]);
    // }

    (lower_bounds, upper_bounds)
}

/// Build an initial guess vector [f, Q, g] for each filter
fn initial_guess(args: &autoeq::cli::Args, lower_bounds: &Vec<f64>, upper_bounds: &Vec<f64>) -> Vec<f64> {
    let mut x = vec![];
    for i in 0..args.num_filters {
        let freq = lower_bounds[i*3].min(args.max_freq.log10());
        let q = (upper_bounds[i*3+1]*lower_bounds[i*3+1]).sqrt();
        let sign = if i % 2 == 0 { 0.5 } else { -0.5 };
        let gain = sign * upper_bounds[i*3+2].max(args.min_db);
        x.extend_from_slice(&[freq, q, gain]);
    }

    x
}

/// Print frequency spacing diagnostics and PEQ listing
fn print_freq_spacing(x: &Vec<f64>, args: &autoeq::cli::Args, label: &str) {
    let (sorted_freqs, adj_spacings) = optim::compute_sorted_freqs_and_adjacent_octave_spacings(x);
    let min_adj = adj_spacings.iter().cloned().fold(f64::INFINITY, f64::min);
    let freqs_fmt: Vec<String> = sorted_freqs.iter().map(|f| format!("{:.0}", f)).collect();
    let spacings_fmt: Vec<String> = adj_spacings.iter().map(|s| format!("{:.2}", s)).collect();
    if min_adj >= args.min_spacing_oct {
	println!("✅ Spacing diagnostics ({}):", label);
    } else {
	println!("⚠️ Spacing diagnostics ({}):", label);
    }
    println!("  - Sorted center freqs (Hz): [{}]", freqs_fmt.join(", "));
    println!(
        "  - Adjacent spacings (oct):   [{}]",
        spacings_fmt.join(", ")
    );
    if min_adj.is_finite() {
        println!(
            "  - Min adjacent spacing: {:.4} oct (constraint {:.4} oct)",
            min_adj, args.min_spacing_oct
        );
    } else {
        println!("  - Not enough filters to compute spacing.");
    }
    iir::peq_print(x, args.iir_hp_pk);
}

fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = setup_bounds(args);

    let mut x = initial_guess(args, &lower_bounds, &upper_bounds);

    // iir::peq_print(&x, args.iir_hp_pk);

    let result = optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args.algo,
        args.population,
        args.maxeval,
    );

    match result {
        Ok((status, val)) => {
            println!(
                "✅ Global optimization completed with status: {}. Objective function value: {:.6}",
                status, val
            );

            print_freq_spacing(&x, args, "global");
        }
        Err((e, final_value)) => {
            eprintln!("\n❌ Optimization failed: {:?}", e);
            eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
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
        );
        match local_result {
            Ok((local_status, local_val)) => {
                println!(
                    "✅ Running local refinement with {}... completed {} objective {:.6}",
                    args.local_algo, local_status, local_val
                );

                print_freq_spacing(&x, args, "local");
                iir::peq_print(&x, args.iir_hp_pk);
            }
            Err((e, final_value)) => {
                eprintln!("⚠️  Local refinement failed: {:?}", e);
                eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e).into());
            }
        }
    };

    Ok(x)
}

async fn plot_results(
    args: &autoeq::cli::Args,
    x: &Vec<f64>,
    _objective_data: &ObjectiveData,
    input_curve: &Curve,
    spin_data: &Option<HashMap<String, Curve>>,
    target_curve: &Array1<f64>,
    smoothed_curve: &Option<Array1<f64>>,
    cea_metrics_before: Option<score::ScoreMetrics>,
    use_cea: bool,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref output_path) = args.output {
        let eq_response =
            iir::compute_peq_response(&input_curve.freq, x, args.sample_rate, args.iir_hp_pk);

        let smoothed_curve_opt = smoothed_curve.as_ref().map(|smoothed| crate::Curve {
            freq: input_curve.freq.clone(),
            spl: smoothed.clone(),
        });

        let args_cloned = args.clone();
        let input_curve_cloned = input_curve.clone();
        let smoothed_curve_cloned = smoothed_curve_opt.as_ref().map(|c| c.clone());
        let target_curve_cloned = target_curve.clone();
        let x_cloned = x.clone();
        let output_path_cloned = output_path.clone();
        let spin_data_cloned = spin_data.clone();
        let eq_response_cloned = eq_response.clone();

        let plot_handle = std::thread::spawn(move || {
            let _ = plot::plot_results(
                &args_cloned,
                &input_curve_cloned,
                smoothed_curve_cloned.as_ref(),
                &target_curve_cloned,
                &x_cloned,
                &output_path_cloned,
                spin_data_cloned.as_ref(),
                Some(&eq_response_cloned),
            );
        });

        plot_handle.join().expect("Plotting thread panicked");
    }

    if use_cea {
        let freq = &input_curve.freq;
        let peq_after = iir::compute_peq_response(freq, x, args.sample_rate, args.iir_hp_pk);
        let metrics_after =
            score::compute_cea2034_metrics(freq, spin_data.as_ref().unwrap(), Some(&peq_after))
                .await?;
        if let Some(before) = cea_metrics_before {
            println!(
                "✅  Pre-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}Hz sm_pir={:.3}",
                before.pref_score,
                before.nbd_on,
                before.nbd_pir,
                10f64.powf(before.lfx),
                before.sm_pir
            );
        }
        println!(
            "✅ Post-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}hz sm_pir={:.3}",
            metrics_after.pref_score,
            metrics_after.nbd_on,
            metrics_after.nbd_pir,
            10f64.powf(metrics_after.lfx),
            metrics_after.sm_pir
        );
    }

    Ok(())
}

/// A command-line tool to find optimal IIR filters to match a frequency curve.

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = autoeq::cli::Args::parse();
    let (input_curve, spin_data) = load_input_curve(&args).await?;
    let (inverted_curve, smoothed_curve) = build_target_curve(&args, &input_curve);
    let target_curve = smoothed_curve.as_ref().unwrap_or(&inverted_curve);
    let (objective_data, use_cea) =
        setup_objective_data(&args, &input_curve, target_curve, &spin_data);

    let mut cea_metrics_before: Option<score::ScoreMetrics> = None;
    if use_cea {
        let metrics =
            score::compute_cea2034_metrics(&input_curve.freq, spin_data.as_ref().unwrap(), None)
                .await?;
        cea_metrics_before = Some(metrics);
    }

    let x = perform_optimization(&args, &objective_data)?;

    plot_results(
        &args,
        &x,
        &objective_data,
        &input_curve,
        &spin_data,
        target_curve,
        &smoothed_curve,
        cea_metrics_before,
        use_cea,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_target_curve, setup_bounds,
    };
    use autoeq::cli::Args;
    use clap::Parser;
    use ndarray::Array1;

    #[test]
    fn setup_bounds_hp_pk_mode_overrides_first_triplet() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 2;
        args.min_freq = 30.0;
        args.max_freq = 2000.0;
        args.min_q = 0.3;
        args.max_q = 8.0;
        args.max_db = 12.0;
        args.iir_hp_pk = true;

        let (lb, ub) = setup_bounds(&args);
        assert_eq!(lb.len(), args.num_filters * 3);
        assert_eq!(ub.len(), args.num_filters * 3);

        // First triplet should be overridden for HP
        assert!((lb[0] - 20.0_f64.max(args.min_freq).log10()).abs() < 1e-12);
        assert!((ub[0] - 120.0_f64.min(args.min_freq + 20.0).log10()).abs() < 1e-12);
        assert!((lb[1] - 1.0).abs() < 1e-12);
        assert!((ub[1] - 1.5).abs() < 1e-12);
        assert!((lb[2] - 0.0).abs() < 1e-12);
        assert!((ub[2] - 0.0).abs() < 1e-12);

        // Second filter should follow the general pattern
        let gain_lower = -6.0 * args.max_db;
        let q_lower = args.min_q.max(1.0e-6);
        assert!((lb[3] - args.min_freq.log10()).abs() < 1e-12);
        assert!((lb[4] - q_lower).abs() < 1e-12);
        assert!((lb[5] - gain_lower).abs() < 1e-12);
        assert!((ub[3] - args.max_freq.log10()).abs() < 1e-12);
        assert!((ub[4] - args.max_q).abs() < 1e-12);
        assert!((ub[5] - args.max_db).abs() < 1e-12);
    }

    #[test]
    fn listening_window_target_profile() {
        let mut args = Args::parse_from(["autoeq-test"]);
        // Ensure we hit the custom target branch and avoid clamping negatives
        args.curve_name = "Listening Window".to_string();
        args.iir_hp_pk = true;

        let freqs = Array1::from_vec(vec![500.0_f64, 1000.0_f64, 20000.0_f64]);
        let spl = Array1::<f64>::zeros(freqs.len());
        let curve = autoeq::Curve { freq: freqs, spl };

        let (inverted_curve, smoothed) = build_target_curve(&args, &curve);
        assert!(smoothed.is_none());
        // Since SPL is zero, inverted_curve == base_target
        assert!((inverted_curve[0] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[1] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[2] - (-0.5)).abs() < 1e-12);
    }

}
