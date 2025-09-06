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
use autoeq::optim;
use autoeq::optim::ObjectiveData;
use autoeq::plot;
use autoeq::score;
use clap::Parser;
use ndarray::Array1;
use std::collections::HashMap;
use std::error::Error;

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

fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    let (lower_bounds, upper_bounds) = autoeq::workflow::setup_bounds(args);

    let mut x = autoeq::workflow::initial_guess(args, &lower_bounds, &upper_bounds);

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
    // Generate plots - use provided output path or create default
    let output_path = args.output.clone().unwrap_or_else(|| {
        use std::path::Path;
        if let Some(speaker) = &args.speaker {
            // Use speaker name for default filename
            let safe_name = speaker.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
            Path::new(&format!("autoeq_{}", safe_name)).to_path_buf()
        } else {
            Path::new("autoeq_results").to_path_buf()
        }
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

    println!("📊 Generating plots: {}", output_path_cloned.display());
    let plot_handle = std::thread::spawn(move || {
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
            eprintln!("⚠️ Warning: Failed to generate plots: {}", e);
        } else {
            println!("✅ Plots generated successfully");
        }
    });

    plot_handle.join().expect("Plotting thread panicked");

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
    
    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args);
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
