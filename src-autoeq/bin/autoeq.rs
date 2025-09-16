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

use autoeq::cea2034 as score;
use autoeq::iir;
use autoeq::loss;
use autoeq::optim;
use autoeq::optim::ObjectiveData;
use autoeq::plot;
use autoeq::read;
use autoeq::Curve;
use autoeq_env::DATA_GENERATED;
use clap::Parser;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

// build_target_curve wrapper removed; call autoeq::workflow::build_target_curve directly

// No local setup_bounds wrapper; tests refer to autoeq::workflow::setup_bounds directly.

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
    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_bounds(args);

    let mut x = autoeq::workflow::initial_guess(args, &lower_bounds, &upper_bounds);

    let result = optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &args.algo,
        args.population,
        args.maxeval,
        &args,
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
        let result = optim::optimize_filters(
            &mut x,
            &lower_bounds,
            &upper_bounds,
            objective_data.clone(),
            &args.local_algo,
            args.population,
            args.maxeval,
            &args,
        );
        match result {
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

/// A command-line tool to find optimal IIR filters to match a frequency curve.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = autoeq::cli::Args::parse();

    // Check if user wants to see algorithm list
    if args.algo_list {
        autoeq::cli::display_algorithm_list();
    }

    // Check if user wants to see strategy list
    if args.strategy_list {
        autoeq::cli::display_strategy_list();
    }

    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args);

    // Load input data
    let (input_curve_raw, spin_data_raw) = autoeq::workflow::load_input_curve(&args).await?;

    // Build target
    let (inverted_curve_raw, smoothed_curve_raw) =
        autoeq::workflow::build_target_curve(&args, &input_curve_raw);
    let target_curve_raw = smoothed_curve_raw.as_ref().unwrap_or(&inverted_curve_raw);

    // resample everything
    let standard_freq = read::create_log_frequency_grid(200, 20.0, 20000.0);
    let input_curve = Curve {
        freq: standard_freq.clone(),
        spl: read::interpolate_log_space(
            &input_curve_raw.freq,
            &input_curve_raw.spl,
            &standard_freq,
        ),
    };
    let target_curve =
        read::interpolate_log_space(&input_curve_raw.freq, target_curve_raw, &standard_freq);
    let spin_data = spin_data_raw.map(|spin_data| {
        spin_data
            .into_iter()
            .map(|(name, curve)| {
                (
                    name,
                    Curve {
                        freq: standard_freq.clone(),
                        spl: read::interpolate_log_space(&curve.freq, &curve.spl, &standard_freq),
                    },
                )
            })
            .collect()
    });

    // Objective data
    let (objective_data, use_cea) =
        autoeq::workflow::setup_objective_data(&args, &input_curve, &target_curve, &spin_data);

    // Metrics before optimisation
    let mut cea2034_metrics_before: Option<score::ScoreMetrics> = None;
    let mut headphone_metrics_before: Option<f64> = None;
    match objective_data.loss_type {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            // Calculate normalized deviation like in headphone_loss_demo
            let (normalized_freq, normalized_deviation) = autoeq::read::normalize_both_curves(
                &input_curve.freq,
                &input_curve.spl,
                Some((&input_curve.freq, &target_curve)),
            );
            headphone_metrics_before = Some(loss::headphone_loss(
                &normalized_freq,
                &normalized_deviation,
            ));
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => {
            if use_cea {
                let metrics = score::compute_cea2034_metrics(
                    &input_curve.freq,
                    spin_data.as_ref().unwrap(),
                    None,
                )
                .await?;
                cea2034_metrics_before = Some(metrics);
            }
        }
    }

    // Optimize
    println!("🚀 Starting optimization...");
    let x = perform_optimization(&args, &objective_data)?;

    match objective_data.loss_type {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            if let Some(before) = headphone_metrics_before {
                println!("✅  Pre-Optimization Headphone Score: {:.3}", before);
            }
            let freq = &input_curve.freq;
            let peq_after = iir::compute_peq_response(freq, &x, args.sample_rate, args.iir_hp_pk);
            // Calculate normalized deviation for post-optimization like in headphone_loss_demo
            let corrected_spl = &input_curve.spl + &peq_after;
            let (normalized_freq_after, normalized_deviation_after) =
                autoeq::read::normalize_both_curves(
                    &input_curve.freq,
                    &corrected_spl,
                    Some((&input_curve.freq, &target_curve)),
                );
            let headphone_metrics_after =
                loss::headphone_loss(&normalized_freq_after, &normalized_deviation_after);
            println!(
                "✅ Post-Optimization Headphone Score: {:.3}",
                headphone_metrics_after
            );
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => {
            if use_cea {
                let freq = &input_curve.freq;
                let peq_after =
                    iir::compute_peq_response(freq, &x, args.sample_rate, args.iir_hp_pk);
                let metrics_after = score::compute_cea2034_metrics(
                    freq,
                    spin_data.as_ref().unwrap(),
                    Some(&peq_after),
                )
                .await?;
                if let Some(before) = cea2034_metrics_before {
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
        }
    }

    // Plot and report
    let output_path = args.output.clone().unwrap_or_else(|| {
        let mut path = PathBuf::from(DATA_GENERATED);
        path.push("autoeq");
        if let Some(speaker) = &args.speaker {
            // Use speaker name for default filename
            let safe_name = speaker.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            path.push(format!("autoeq_{}", safe_name));
        } else {
            path.push("autoeq_results");
        }
        path
    });

    println!("📊 Generating plots: {}", output_path.display());
    if let Err(e) = plot::plot_results(
        &args,
        &x,
        &objective_data,
        &input_curve,
        &spin_data,
        &target_curve,
        &smoothed_curve_raw,
        &output_path,
    )
    .await
    {
        eprintln!("⚠️ Warning: Failed to generate plots: {}", e);
    } else {
        println!("✅ Plots generated successfully");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
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

        let (lb, ub) = autoeq::workflow::setup_bounds(&args);
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

        let (inverted_curve, _smoothed) = autoeq::workflow::build_target_curve(&args, &curve);
        // Since SPL is zero, inverted_curve == base_target
        assert!((inverted_curve[0] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[1] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[2] - (-0.5)).abs() < 1e-12);
    }
}
