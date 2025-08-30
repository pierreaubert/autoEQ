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

// Helper to distribute Q values across [min_q, max_q] in log-space
fn distribute_qs(num_filters: usize, min_q: f64, max_q: f64) -> Vec<f64> {
    if num_filters == 0 {
        return Vec::new();
    }

    if num_filters == 1 {
        return vec![(min_q * max_q).sqrt()];
    }
    let qmin = min_q.max(1e-6);
    let qmax = max_q.max(qmin * 1.000001);
    (0..num_filters)
        .map(|i| {
            let t = i as f64 / (num_filters as f64 - 1.0);
            (qmin.ln() + t * (qmax.ln() - qmin.ln())).exp()
        })
        .collect()
}

// Helper to distribute initial gain magnitudes across [mag_min, max_db]
// - If min_db > 0, mag_min = min_db (to satisfy barrier from the start)
// - If min_db == 0, mag_min = 0.1 * max_db (small but non-zero)
// Returned values are magnitudes (>=0), caller applies alternating signs.
fn distribute_gain_magnitudes(num_filters: usize, min_db: f64, max_db: f64) -> Vec<f64> {
    if num_filters == 0 {
        return Vec::new();
    }
    let mag_min = if min_db > 0.0 { min_db } else { 0.1 * max_db };
    if num_filters == 1 {
        return vec![(mag_min + max_db) * 0.5];
    }
    (0..num_filters)
        .map(|i| {
            let t = i as f64 / (num_filters as f64 - 1.0);
            mag_min + t * (max_db - mag_min)
        })
        .collect()
}

// New helper functions

async fn load_input_curve(
    args: &autoeq::cli::Args,
) -> Result<(Curve, Option<HashMap<String, Curve>>), Box<dyn Error>> {
    let mut spin_data: Option<HashMap<String, Curve>> = None;

    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        let plot_data = read::fetch_measurement_plot_data(speaker, version, measurement).await?;
        let extracted_curve =
            read::extract_curve_by_name(&plot_data, measurement, &args.curve_name)?;
        if measurement == "CEA2034" {
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
            "* Loaded target curve with {} points.",
            target_curve.freq.len()
        );
        read::interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        println!("* No target curve provided, using a flat 0 dB target.");
        Array1::zeros(input_curve.spl.len())
    };

    let mut inverted_curve = base_target - input_curve.spl.clone();
    if !args.iir_hp_pk {
        inverted_curve = read::clamp_positive_only(&inverted_curve, args.max_db);
    }

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
    let use_cea = matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("CEA2034"))
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

fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let num_params = args.num_filters * 3;
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let gain_lower = -6.0 * args.max_db;
    let q_lower = args.min_q.max(1.0e-6);
    for _ in 0..args.num_filters {
        lower_bounds.extend_from_slice(&[args.min_freq, q_lower, gain_lower]);
        upper_bounds.extend_from_slice(&[args.max_freq, args.max_q, args.max_db]);
    }

    if args.iir_hp_pk {
        lower_bounds[0] = 20.0;
        upper_bounds[0] = 120.0;
        lower_bounds[1] = 1.0;
        upper_bounds[1] = 1.5;
        lower_bounds[2] = 0.0;
        upper_bounds[2] = 0.0;
    }

    let mut x = vec![];
    let log_min = args.min_freq.ln();
    let log_max = args.max_freq.ln();
    let log_range = log_max - log_min;

    let q_vec = distribute_qs(args.num_filters, args.min_q, args.max_q / 2.0);
    let g_mags = distribute_gain_magnitudes(args.num_filters, args.min_db, args.max_db / 2.0);
    for i in 0..args.num_filters {
        let freq = (log_min + (i as f64 + 0.5) * log_range / args.num_filters as f64).exp();
        let q = q_vec[i];
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let gain = sign * g_mags[i];
        x.extend_from_slice(&[freq, q, gain]);
    }

    if args.iir_hp_pk {
        x[0] = 80.0;
        x[1] = 1.1;
        x[2] = 0.0;
    }

    iir::peq_print(&x, args.iir_hp_pk);

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
                "* Global optimization completed with status: {}. Objective function value: {:.6}",
                status, val
            );

            let (sorted_freqs, adj_spacings) =
                optim::compute_sorted_freqs_and_adjacent_octave_spacings(&x);
            let min_adj = adj_spacings.iter().cloned().fold(f64::INFINITY, f64::min);
            println!("* Spacing diagnostics (global):");
            let freqs_fmt: Vec<String> = sorted_freqs.iter().map(|f| format!("{:.0}", f)).collect();
            let spacings_fmt: Vec<String> =
                adj_spacings.iter().map(|s| format!("{:.2}", s)).collect();
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
            iir::peq_print(&x, args.iir_hp_pk);

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
                            "* Running local refinement with {}... completed {} objective {:.6}",
                            args.local_algo, local_status, local_val
                        );

                        let (sorted_freqs, adj_spacings) =
                            optim::compute_sorted_freqs_and_adjacent_octave_spacings(&x);
                        let min_adj = adj_spacings.iter().cloned().fold(f64::INFINITY, f64::min);
                        println!("* Spacing diagnostics (local):");
                        let freqs_fmt: Vec<String> =
                            sorted_freqs.iter().map(|f| format!("{:.0}", f)).collect();
                        let spacings_fmt: Vec<String> =
                            adj_spacings.iter().map(|s| format!("{:.2}", s)).collect();
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
                        iir::peq_print(&x, args.iir_hp_pk);
                    }
                    Err((e, final_value)) => {
                        eprintln!("⚠️  Local refinement failed: {:?}", e);
                        eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
                    }
                }
            }

            Ok(x)
        }
        Err((e, final_value)) => {
            eprintln!("\n❌ Optimization failed: {:?}", e);
            eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
            Err(e.into())
        }
    }
}

async fn handle_results(
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
                "*  Pre-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}Hz sm_pir={:.3}",
                before.pref_score,
                before.nbd_on,
                before.nbd_pir,
                10f64.powf(before.lfx),
                before.sm_pir
            );
        }
        println!(
            "* Post-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}hz sm_pir={:.3}",
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

    handle_results(
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
    use super::{distribute_gain_magnitudes, distribute_qs};

    #[test]
    fn q_distribution_within_bounds_and_spread() {
        let n = 5usize;
        let (min_q, max_q) = (0.2, 6.0);
        let qs = distribute_qs(n, min_q, max_q);
        assert_eq!(qs.len(), n);
        assert!(qs.iter().all(|&q| q >= min_q && q <= max_q));
        assert!(qs.windows(2).all(|w| w[0] <= w[1]));
        // Ensure not constant
        assert!((qs[0] - qs[qs.len() - 1]).abs() > 1e-9);
    }

    #[test]
    fn q_distribution_one_filter_is_geometric_mean() {
        let (min_q, max_q) = (0.5, 8.0);
        let qs = distribute_qs(1, min_q, max_q);
        assert_eq!(qs.len(), 1);
        let geom = (min_q * max_q).sqrt();
        assert!((qs[0] - geom).abs() < 1e-12);
    }

    #[test]
    fn q_distribution_zero_filters_empty() {
        let qs = distribute_qs(0, 0.5, 8.0);
        assert!(qs.is_empty());
    }

    #[test]
    fn gain_magnitude_distribution_within_bounds_and_spans_to_max() {
        let mags = distribute_gain_magnitudes(5, 1.0, 6.0);
        assert_eq!(mags.len(), 5);
        assert!(mags.iter().all(|&m| m >= 1.0 && m <= 6.0));
        assert!(mags.windows(2).all(|w| w[0] <= w[1]));
        assert!((mags.first().unwrap() - 1.0).abs() < 1e-12);
        assert!((mags.last().unwrap() - 6.0).abs() < 1e-12);
    }

    #[test]
    fn gain_magnitude_single_is_mid_between_min_and_max() {
        let mags = distribute_gain_magnitudes(1, 2.0, 8.0);
        assert_eq!(mags.len(), 1);
        assert!((mags[0] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn gain_magnitude_zero_filters_empty() {
        let mags = distribute_gain_magnitudes(0, 1.0, 6.0);
        assert!(mags.is_empty());
    }

    #[test]
    fn gain_magnitude_when_min_db_zero_starts_at_fraction_of_max() {
        // When min_db == 0, magnitudes should start at 0.1 * max_db and end at max_db
        let max_db = 10.0;
        let mags = distribute_gain_magnitudes(5, 0.0, max_db);
        assert_eq!(mags.len(), 5);
        assert!((mags.first().unwrap() - 0.1 * max_db).abs() < 1e-12);
        assert!((mags.last().unwrap() - max_db).abs() < 1e-12);
        assert!(mags.windows(2).all(|w| w[0] <= w[1]));
    }
}
