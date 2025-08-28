use autoeq::iir;
use autoeq::optim;
use autoeq::optim::ObjectiveData;
use autoeq::plot;
use autoeq::read;
use autoeq::score;
use autoeq::{LossType, ScoreLossData};
use clap::Parser;
use ndarray::Array1;
use serde_json::Value;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = autoeq::cli::Args::parse();
    let mut cea_plot_data: Option<Value> = None;

    // ----------------------------------------------------------------------
    // 1. Load the input curve (the one we want to match).
    // Use API data if speaker, version, and measurement are provided, otherwise use CSV file
    // ----------------------------------------------------------------------
    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        // Fetch full measurement once and cache it
        let plot_data = read::fetch_measurement_plot_data(speaker, version, measurement).await?;
        cea_plot_data = Some(plot_data.clone());
        read::extract_curve_by_name(&plot_data, measurement, &args.curve_name)?
    } else {
        // If no API parameters are provided, curve file must be provided
        let curve_path = args.curve.as_ref().ok_or(
            "Either --curve or all of --speaker, --version, and --measurement must be provided",
        )?;
        read::read_curve_from_csv(curve_path)?
    };

    // ----------------------------------------------------------------------
    // 2. Build inverted target from the selected curve, with positive-only clamp and optional smoothing.
    // Base target is flat 0 dB unless a target file was provided (we still invert the selected curve relative to it).
    // ----------------------------------------------------------------------
    let base_target = if let Some(ref target_path) = args.target {
        let target_curve = read::read_curve_from_csv(&target_path)?;
        println!(
            "* Loaded target curve with {} points.",
            target_curve.freq.len()
        );
        read::interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        println!("* No target curve provided, using a flat 0 dB target.");
        Array1::zeros(input_curve.spl.len())
    };

    // Inverted curve relative to base target
    let mut inverted = base_target.clone() - input_curve.spl.clone();
    // Clip positive side only if HP+PK mode is disabled
    if !args.iir_hp_pk {
        inverted = read::clamp_positive_only(&inverted, args.max_db);
    }

    // Optional smoothing regularization of the inverted curve
    let mut smoothed: Option<Array1<f64>> = None;
    if args.smooth {
        smoothed = Some(read::smooth_one_over_n_octave(
            &input_curve.freq,
            &inverted,
            args.smooth_n,
        ));
    }

    // ----------------------------------------------------------------------
    // 3. Define the optimization target error (use smoothed if provided)
    // ----------------------------------------------------------------------
    let target_error = smoothed.clone().unwrap_or_else(|| inverted.clone());

    // Determine if we have CEA2034 measurement data available (speaker+version+measurement provided and measurement is CEA2034)
    let use_cea = matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("CEA2034"))
        && args.speaker.is_some()
        && args.version.is_some()
        && cea_plot_data.is_some();

    // Prepare score loss data when requested and possible
    let score_data_opt: Option<ScoreLossData> = if args.loss == LossType::Score && use_cea {
        let measurement_name = args.measurement.as_deref().unwrap();
        match read::extract_cea2034_curves(
            cea_plot_data.as_ref().unwrap(),
            measurement_name,
            &input_curve.freq,
        ) {
            Ok(curves) => {
                let get_spl = |name: &str| curves.get(name).map(|c| c.spl.clone());
                let on = get_spl("On Axis");
                let lw = get_spl("Listening Window").or_else(|| get_spl("Lateral"));
                let sp = get_spl("Sound Power");
                let pir = get_spl("Estimated In-Room Response");
                match (on, lw, sp, pir) {
                    (Some(on), Some(lw), Some(sp), Some(pir)) => {
                        let intervals = score::octave_intervals(2, &input_curve.freq);
                        Some(ScoreLossData::new(on, lw, sp, pir, intervals))
                    }
                    _ => {
                        eprintln!(
                            "⚠️  Missing required CEA2034 curves for score-based loss; falling back to flat loss"
                        );
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "⚠️  Failed to extract CEA2034 curves for score-based loss: {}",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    let objective_data = ObjectiveData {
        freqs: input_curve.freq.clone(),
        target_error,
        srate: args.sample_rate,
        min_spacing_oct: args.min_spacing_oct,
        spacing_weight: args.spacing_weight,
        max_db: args.max_db,
        min_db: args.min_db,
        iir_hp_pk: args.iir_hp_pk,
        loss_type: args.loss,
        score_data: score_data_opt.clone(),
    };

    // If measurement is CEA2034 via API, compute score before optimization using score_peq_approx
    let mut cea_metrics_before: Option<score::ScoreMetrics> = None;
    if use_cea {
        let measurement_name = args.measurement.as_deref().unwrap();
        let curves_for_metrics = read::extract_cea2034_curves(
            cea_plot_data.as_ref().unwrap(),
            measurement_name,
            &input_curve.freq,
        )?;
        let metrics =
            score::compute_cea2034_metrics(&input_curve.freq, &curves_for_metrics, None).await?;
        cea_metrics_before = Some(metrics);
    }

    // ----------------------------------------------------------------------
    // 4. optimisation
    // ----------------------------------------------------------------------
    // Each filter has 3 parameters: frequency, Q, and gain.
    let num_params = args.num_filters * 3;

    // Define the bounds for each parameter: [freq_min, q_min, gain_min, freq_min, q_min, gain_min, ...]
    let mut lower_bounds = Vec::with_capacity(num_params);
    let mut upper_bounds = Vec::with_capacity(num_params);

    let gain_lower = -6.0 * args.max_db; // No strict negative minimum; allow deeper cuts
    for _ in 0..args.num_filters {
        lower_bounds.extend_from_slice(&[args.min_freq, args.min_q, gain_lower]); // Freq, Q, Gain
        upper_bounds.extend_from_slice(&[args.max_freq, args.max_q, args.max_db]);
    }

    // Initial guess for the parameters.
    // Distribute filters logarithmically across the frequency spectrum
    // and give them small non-zero initial gains to encourage better distribution
    let mut x = vec![];
    let log_min = args.min_freq.ln();
    let log_max = args.max_freq.ln();
    let log_range = log_max - log_min;

    for i in 0..args.num_filters {
        // Distribute frequencies logarithmically
        let freq = (log_min + (i as f64 + 0.5) * log_range / args.num_filters as f64).exp();
        // Alternate initial gain signs and satisfy |gain| >= min_db (if > 0)
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let mag = if args.min_db > 0.0 { args.min_db } else { 0.1 };
        let gain = sign * mag.min(args.max_db);
        x.extend_from_slice(&[freq, 1.0, gain]);
    }

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

            // 5. Optional local refinement
            if args.refine {
                let local_result = optim::refine_local(
                    &mut x,
                    &lower_bounds,
                    &upper_bounds,
                    objective_data.clone(),
                    &args.local_algo,
                    args.maxeval,
                );
                match local_result {
                    Ok((local_status, local_val)) => {
                        println!(
                            "* Running local refinement with {}... completed {} objective {:.6}",
                            args.local_algo, local_status, local_val
                        );
                    }
                    Err((e, final_value)) => {
                        eprintln!("⚠️  Local refinement failed: {:?}", e);
                        eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
                    }
                }
            }

            // 6. Plot results if output path is provided
            if let Some(ref output_path) = args.output {
                let cea2034_curves = if use_cea {
                    if let (Some(speaker), Some(version), Some(measurement)) = (
                        args.speaker.clone(),
                        args.version.clone(),
                        args.measurement.clone(),
                    ) {
                        let _speaker_name = speaker.as_str();
                        let _version_name = version.as_str();
                        let measurement_name = measurement.as_str();
                        match read::extract_cea2034_curves(
                            cea_plot_data.as_ref().unwrap(),
                            measurement_name,
                            &input_curve.freq,
                        ) {
                            Ok(curves) => Some(curves),
                            Err(e) => {
                                eprintln!("⚠️  Failed to extract CEA2034 curves: {}", e);
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Compute EQ response for plotting
                let eq_response = iir::compute_peq_response(
                    &input_curve.freq,
                    &x,
                    args.sample_rate,
                    args.iir_hp_pk,
                );

                plot::plot_results(
                    args.curve_name.as_ref(),
                    &input_curve,
                    &x,
                    args.num_filters,
                    args.sample_rate,
                    args.max_db,
                    smoothed.as_ref(),
                    output_path,
                    args.speaker.as_deref(),
                    args.measurement.as_deref(),
                    args.iir_hp_pk,
                    cea2034_curves.as_ref(),
                    Some(&eq_response),
                )
                .await?;
            }

            if use_cea {
                let freq = &input_curve.freq;
                let peq_after =
                    iir::compute_peq_response(freq, &x, args.sample_rate, args.iir_hp_pk);
                let measurement_name = args.measurement.as_deref().unwrap();
                let curves_for_metrics = read::extract_cea2034_curves(
                    cea_plot_data.as_ref().unwrap(),
                    measurement_name,
                    freq,
                )?;
                let metrics_after =
                    score::compute_cea2034_metrics(freq, &curves_for_metrics, Some(&peq_after))
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
        }
        Err((e, final_value)) => {
            eprintln!("\n❌ Optimization failed: {:?}", e);
            eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
        }
    }

    Ok(())
}
