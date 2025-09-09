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
use autoeq::constants::DATA_GENERATED;
use autoeq::iir;
use autoeq::optim;
use autoeq::optim::ObjectiveData;
use autoeq::plot;
use autoeq::cea2034 as score;
use clap::Parser;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::sync::oneshot;

extern crate blas_src;

async fn load_input_curve(
    args: &autoeq::cli::Args,
) -> Result<(Curve, Option<HashMap<String, Curve>>), Box<dyn Error>> {
    autoeq::workflow::load_input_curve(args).await
}

fn build_target_curve(
    args: &autoeq::cli::Args,
    input_curve: &Curve,
) -> (Array1<f64>, Option<Array1<f64>>) {
    autoeq::workflow::build_target_curve(args, input_curve)
}

fn setup_objective_data(
    args: &autoeq::cli::Args,
    input_curve: &Curve,
    target_curve: &Array1<f64>,
    spin_data: &Option<HashMap<String, Curve>>,
) -> (ObjectiveData, bool) {
    autoeq::workflow::setup_objective_data(args, input_curve, target_curve, spin_data)
}

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

async fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
    shutdown: Arc<AtomicBool>,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_bounds(args);

    let mut x = autoeq::workflow::initial_guess(args, &lower_bounds, &upper_bounds);

    // Check for shutdown before optimization
    if shutdown.load(Ordering::Relaxed) {
        return Err("Optimization cancelled by user".into());
    }

    let args_clone = args.clone();
    let objective_data_clone = objective_data.clone();
    let lower_bounds_clone = lower_bounds.clone();
    let upper_bounds_clone = upper_bounds.clone();
    let mut x_clone = x.clone();

    // Run optimization in a blocking task that can be cancelled
    let mut optimization_task = tokio::task::spawn_blocking(move || {
        let result = optim::optimize_filters(
            &mut x_clone,
            &lower_bounds_clone,
            &upper_bounds_clone,
            objective_data_clone,
            &args_clone.algo,
            args_clone.population,
            args_clone.maxeval,
            &args_clone,
        );
        (x_clone, result)
    });

    // Wait for optimization with periodic shutdown checks
    let (x_optimized, result) = loop {
        select! {
            result = &mut optimization_task => {
                match result {
                    Ok((x_opt, opt_result)) => break (x_opt, opt_result),
                    Err(e) => return Err(format!("Optimization task failed: {}", e).into()),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                if shutdown.load(Ordering::Relaxed) {
                    eprintln!("⚠️  Optimization interrupted by shutdown signal.");
                    optimization_task.abort();
                    return Err("Optimization cancelled by user".into());
                }
            }
        }
    };

    // Get the optimized parameters
    x = x_optimized;

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
        // Check for shutdown before local refinement
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("⚠️  Local refinement skipped due to shutdown signal.");
            return Ok(x);
        }

        let args_clone = args.clone();
        let objective_data_clone = objective_data.clone();
        let lower_bounds_clone = lower_bounds.clone();
        let upper_bounds_clone = upper_bounds.clone();
        let mut x_clone = x.clone();

        let mut refinement_task = tokio::task::spawn_blocking(move || {
            let result = optim::optimize_filters(
                &mut x_clone,
                &lower_bounds_clone,
                &upper_bounds_clone,
                objective_data_clone,
                &args_clone.local_algo,
                args_clone.population,
                args_clone.maxeval,
                &args_clone,
            );
            (x_clone, result)
        });

        let (x_refined, local_result) = loop {
            select! {
                result = &mut refinement_task => {
                    match result {
                        Ok((x_ref, opt_result)) => break (x_ref, opt_result),
                        Err(e) => return Err(format!("Refinement task failed: {}", e).into()),
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    if shutdown.load(Ordering::Relaxed) {
                        eprintln!("⚠️  Local refinement interrupted by shutdown signal.");
                        refinement_task.abort();
                        return Ok(x); // Return best result so far
                    }
                }
            }
        };

        x = x_refined;

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
    shutdown: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    // Generate plots - use provided output path or create default in DATA_GENERATED/autoeq
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
    let shutdown_clone = Arc::clone(&shutdown);

    // Create a channel to communicate with the plotting thread
    let (_plot_tx, _plot_rx) = oneshot::channel::<()>();

    println!("📊 Generating plots: {}", output_path_cloned.display());
    let plot_handle: JoinHandle<Result<(), String>> = std::thread::spawn(move || {
        // Check for shutdown signal before plotting
        if shutdown_clone.load(Ordering::Relaxed) {
            return Ok(());
        }

        let result = plot::plot_results(
            &args_cloned,
            &input_curve_cloned,
            smoothed_curve_cloned.as_ref(),
            &target_curve_cloned,
            &x_cloned,
            &output_path_cloned,
            spin_data_cloned.as_ref(),
            Some(&eq_response_cloned),
        );
        if let Err(e) = result {
            let error_msg = format!("Failed to generate plots: {}", e);
            eprintln!("⚠️ Warning: {}", error_msg);
            return Err(error_msg);
        } else {
            println!("✅ Plots generated successfully");
        }
        Ok(())
    });

    // Wait for plotting to complete or shutdown signal
    let plot_task = tokio::task::spawn_blocking(move || {
        plot_handle.join().unwrap_or_else(|_| {
            eprintln!("⚠️ Plotting thread panicked");
            Err("Plotting thread panicked".to_string())
        })
    });

    select! {
        result = plot_task => {
            match result {
                Ok(Ok(())) => {},
                Ok(Err(e)) => eprintln!("⚠️ Warning: Failed to generate plots: {}", e),
                Err(e) => eprintln!("⚠️ Warning: Plotting task failed: {}", e),
            }
        }
        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
            // Check for shutdown periodically
            if shutdown.load(Ordering::Relaxed) {
                eprintln!("🛑 Plotting interrupted by shutdown signal");
                return Ok(());
            }
        }
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

    // Check if user wants to see algorithm list
    if args.algo_list {
        autoeq::cli::display_algorithm_list();
    }

    // Check if user wants to see strategy list
    if args.strategy_list {
        autoeq::cli::display_strategy_list();
    }

    // Set up signal handling for graceful shutdown
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    // Spawn a dedicated signal handler task
    tokio::spawn(async move {
        let mut sigint_count = 0;
        loop {
            if let Err(e) = tokio::signal::ctrl_c().await {
                eprintln!("⚠️ Error setting up signal handler: {}", e);
                break;
            }

            sigint_count += 1;

            if sigint_count == 1 {
                eprintln!("\n🛑 Received interrupt signal (1/2). Attempting graceful shutdown...");
                eprintln!("📝 Press Ctrl+C again within 3 seconds to force immediate termination.");
                shutdown_clone.store(true, Ordering::Relaxed);

                // Wait 3 seconds for graceful shutdown
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(3) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if !shutdown_clone.load(Ordering::Relaxed) {
                        // Process completed gracefully
                        return;
                    }
                }
            } else {
                eprintln!("\n‼️ Received second interrupt signal. Forcing immediate termination!");
                std::process::exit(130); // Standard exit code for SIGINT
            }
        }
    });

    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args);
    // Main execution wrapped in select! for signal handling
    let main_task = async {
        let (input_curve, spin_data) = load_input_curve(&args).await?;

        // Check for shutdown signal during data loading
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("\n🛑 Interrupted during data loading. Exiting gracefully...");
            return Ok(());
        }

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

        // Check for shutdown signal before starting optimization
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("\n🛑 Interrupted before optimization. Exiting gracefully...");
            return Ok(());
        }

        eprintln!("🚀 Starting optimization... (Press Ctrl+C for graceful shutdown)");
        let x = perform_optimization(&args, &objective_data, Arc::clone(&shutdown)).await?;

        // Check for shutdown signal after optimization
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("\n🛑 Interrupted after optimization. Skipping plot generation...");
            println!("✅ Optimization completed successfully before shutdown.");
            return Ok(());
        }

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
            Arc::clone(&shutdown),
        )
        .await?;

        Ok(())
    };

    // Run the main task
    main_task.await
}

#[cfg(test)]
mod tests {
    use super::build_target_curve;
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

        let (inverted_curve, _smoothed) = build_target_curve(&args, &curve);
        // Since SPL is zero, inverted_curve == base_target
        assert!((inverted_curve[0] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[1] - 0.0).abs() < 1e-12);
        assert!((inverted_curve[2] - (-0.5)).abs() < 1e-12);
    }
}
