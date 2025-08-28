use clap::Parser;
use ndarray::Array1;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

mod iir;
mod optim;
use optim::ObjectiveData;
mod plot;
mod read;
mod score;

/// A command-line tool to find optimal IIR filters to match a frequency curve.
#[derive(Parser, Debug)]
#[command(author, about, long_about = None)]
struct Args {
    /// Number of IIR filters to use for optimization.
    #[arg(short = 'n', long, default_value_t = 6)]
    num_filters: usize,

    /// Path to the input curve CSV file (format: frequency,spl).
    /// Required unless speaker, version, and measurement are provided for API data.
    #[arg(short, long)]
    curve: Option<PathBuf>,

    /// Path to the optional target curve CSV file (format: frequency,spl).
    /// If not provided, a flat 0 dB target is assumed.
    #[arg(short, long)]
    target: Option<PathBuf>,

    /// The sample rate for the IIR filters.
    #[arg(short, long, default_value_t = 48000.0)]
    sample_rate: f64,

    /// Maximum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 6.0)]
    max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 0.5)]
    min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 6.0)]
    max_q: f64,

    /// Minimum Q factor allowed for each filter.
    #[arg(long, default_value_t = 0.2)]
    min_q: f64,

    /// Minimum frequency allowed for each filter.
    #[arg(long, default_value_t = 20.0)]
    min_freq: f64,

    /// Maximum frequency allowed for each filter.
    #[arg(long, default_value_t = 20000.0)]
    max_freq: f64,

    /// Output PNG file for plotting results.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Speaker name for API data fetching.
    #[arg(long)]
    speaker: Option<String>,

    /// Version for API data fetching.
    #[arg(long)]
    version: Option<String>,

    /// Measurement type for API data fetching.
    #[arg(long)]
    measurement: Option<String>,

    /// Curve name inside CEA2034 plots to use (only when --measurement CEA2034)
    /// e.g., "Listening Window", "On Axis", "Early Reflections". Default: Listening Window
    #[arg(long, default_value = "Listening Window")]
    curve_name: String,

    /// Optimization algorithm to use (e.g., isres, cobyla)
    #[arg(long, default_value = "isres")]
    algo: String,

    /// Optional population size for population-based algorithms (e.g., ISRES)
    #[arg(long)]
    population: Option<usize>,

    /// Maximum number of evaluations for the optimizer
    #[arg(long, default_value_t = 10_000)]
    maxeval: usize,

    /// Whether to run a local refinement after global optimization
    #[arg(long, default_value_t = true)]
    refine: bool,

    /// Local optimizer to use for refinement (e.g., cobyla)
    #[arg(long, default_value = "cobyla")]
    local_algo: String,

    /// Minimum spacing between filter center frequencies in octaves (0 disables)
    #[arg(long, default_value_t = 0.4)]
    min_spacing_oct: f64,

    /// Weight for the spacing penalty in the objective function
    #[arg(long, default_value_t = 1.0)]
    spacing_weight: f64,

    /// Enable smoothing (regularization) of the inverted target curve
    #[arg(long, default_value_t = false)]
    smooth: bool,

    /// Smoothing level as 1/N octave (N in [1..24]). Example: N=6 => 1/6 octave smoothing
    #[arg(long, default_value_t = 6)]
    smooth_n: usize,

    /// If present/true: use a Highpass for the lowest-frequency IIR and do NOT clip the inverted curve.
    /// If false: use all Peak filters and clip the inverted curve on the positive side (current behaviour).
    #[arg(long, default_value_t = false)]
    iir_hp_pk: bool,
}

/// A struct to hold frequency and SPL data.
#[derive(Debug, Clone)]
pub struct Curve {
    pub freq: Array1<f64>,
    pub spl: Array1<f64>,
}

// Compute CEA2034 metrics on the provided frequency grid, optionally with a PEQ response applied.
// Reuses cached plot data if available to avoid redundant API calls.
// Extract all CEA2034 curves from plot data
fn extract_cea2034_curves(
    plot_data: &Value,
    measurement: &str,
    freq: &Array1<f64>,
) -> Result<HashMap<String, Curve>, Box<dyn Error>> {
    let mut curves = HashMap::new();

    // List of CEA2034 curves to extract
    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
        "Early Reflections DI",
        "Sound Power DI",
    ];

    // Extract each curve
    for name in &curve_names {
        match read::extract_curve_by_name(plot_data, measurement, name) {
            Ok(curve) => {
                // Interpolate to the target frequency grid
                let interpolated = read::interpolate(freq, &curve.freq, &curve.spl);
                curves.insert(
                    name.to_string(),
                    Curve {
                        freq: freq.clone(),
                        spl: interpolated,
                    },
                );
            }
            Err(e) => {
                eprintln!("⚠️  Could not extract curve '{}': {}", name, e);
            }
        }
    }

    Ok(curves)
}

async fn compute_cea2034_metrics(
    freq: &Array1<f64>,
    args: &Args,
    cea_plot_data: &mut Option<Value>,
    peq: Option<&Array1<f64>>,
) -> Result<score::ScoreMetrics, Box<dyn Error>> {
    let speaker = args
        .speaker
        .as_ref()
        .ok_or("speaker must be provided when computing CEA2034 metrics")?;
    let version = args
        .version
        .as_ref()
        .ok_or("version must be provided when computing CEA2034 metrics")?;
    let measurement = args
        .measurement
        .as_ref()
        .ok_or("measurement must be provided when computing CEA2034 metrics")?;

    // Use cached plot data or fetch once
    let plot_data = if let Some(pd) = cea_plot_data {
        pd.clone()
    } else {
        let pd = read::fetch_measurement_plot_data(speaker, version, measurement).await?;
        *cea_plot_data = Some(pd.clone());
        pd
    };

    // Extract required traces and interpolate to the input frequency grid
    let on_ax = read::extract_curve_by_name(&plot_data, measurement, "On Axis")?;
    let lw_c = read::extract_curve_by_name(&plot_data, measurement, "Listening Window")?;
    let sp_c = read::extract_curve_by_name(&plot_data, measurement, "Sound Power")?;
    let on = read::interpolate(freq, &on_ax.freq, &on_ax.spl);
    let lw = read::interpolate(freq, &lw_c.freq, &lw_c.spl);
    let sp = read::interpolate(freq, &sp_c.freq, &sp_c.spl);

    // PIR may not be present for some datasets; fall back to LW+ER+SP formula
    let pir = match read::extract_curve_by_name(&plot_data, measurement, "Predicted In-Room") {
        Ok(pir_c) => read::interpolate(freq, &pir_c.freq, &pir_c.spl),
        Err(e) => {
            eprintln!("⚠️  PIR trace not found, computing from LW+ER+SP. {}", e);
            let er_c = read::extract_curve_by_name(&plot_data, measurement, "Early Reflections")?;
            let er = read::interpolate(freq, &er_c.freq, &er_c.spl);
            score::compute_pir_from_lw_er_sp(&lw, &er, &sp)
        }
    };

    // 1/2 octave intervals for band metrics
    let intervals = score::octave_intervals(2, freq);

    // Use provided PEQ or assume zero PEQ
    let peq_arr = peq
        .map(|p| p.clone())
        .unwrap_or_else(|| Array1::zeros(freq.len()));

    Ok(score::score_peq_approx(
        freq, &intervals, &lw, &sp, &pir, &on, &peq_arr,
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut cea_plot_data: Option<Value> = None;

    // 1. Load the input curve (the one we want to match).
    // Use API data if speaker, version, and measurement are provided, otherwise use CSV file
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

    // 2. Build inverted target from the selected curve, with positive-only clamp and optional smoothing.
    // Base target is flat 0 dB unless a target file was provided (we still invert the selected curve relative to it).
    let base_target = if let Some(ref target_path) = args.target {
        let target_curve = read::read_curve_from_csv(&target_path)?;
        println!(
            "✅ Loaded target curve with {} points.",
            target_curve.freq.len()
        );
        read::interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        println!("✅ No target curve provided, using a flat 0 dB target.");
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

    // 3. Define the optimization target error (use smoothed if provided)
    let target_error = smoothed.clone().unwrap_or_else(|| inverted.clone());

    let objective_data = ObjectiveData {
        freqs: input_curve.freq.clone(),
        target_error,
        srate: args.sample_rate,
        min_spacing_oct: args.min_spacing_oct,
        spacing_weight: args.spacing_weight,
        max_db: args.max_db,
        min_db: args.min_db,
        iir_hp_pk: args.iir_hp_pk,
    };

    // If measurement is CEA2034 via API, compute score before optimization using score_peq_approx
    let mut cea_metrics_before: Option<score::ScoreMetrics> = None;
    let use_cea = matches!(args.measurement.as_deref(), Some(m) if m.eq_ignore_ascii_case("CEA2034"))
        && args.speaker.is_some()
        && args.version.is_some();
    if use_cea {
        let metrics =
            compute_cea2034_metrics(&input_curve.freq, &args, &mut cea_plot_data, None).await?;
        cea_metrics_before = Some(metrics);
    }

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

    // 4. Set up and run the optimizer (delegated to optim module).
    let population: usize = args
        .population
        .unwrap_or_else(|| (20 * num_params).max(100));
    println!(
        "\n🚀 Starting optimization for {} filters (algo: {}, population: {}, maxeval: {})...",
        args.num_filters, args.algo, population, args.maxeval
    );
    let result = optim::optimize_filters(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data,
        &args.algo,
        args.population,
        args.maxeval,
    );

    match result {
        Ok((status, final_value)) => {
            println!("\n✅ Optimization finished!");
            println!("   - Status: {:?}", status);
            println!("   - Final Mean Squared Error: {:.6}", final_value);
            // Optional local refinement
            if args.refine {
                println!("\n🔧 Starting local refinement with {}...", args.local_algo);
                // Recreate objective data for the second stage
                let mut inv2 = base_target.clone() - input_curve.spl.clone();
                if !args.iir_hp_pk {
                    inv2 = read::clamp_positive_only(&inv2, args.max_db);
                }
                let target_error2 = if args.smooth {
                    read::smooth_one_over_n_octave(&input_curve.freq, &inv2, args.smooth_n)
                } else {
                    inv2
                };
                let local_data = ObjectiveData {
                    freqs: input_curve.freq.clone(),
                    target_error: target_error2,
                    srate: args.sample_rate,
                    min_spacing_oct: args.min_spacing_oct,
                    spacing_weight: args.spacing_weight,
                    max_db: args.max_db,
                    min_db: args.min_db,
                    iir_hp_pk: args.iir_hp_pk,
                };
                let local_maxeval: usize = (args.maxeval / 2).max(1);
                let res2 = optim::refine_local(
                    &mut x,
                    &lower_bounds,
                    &upper_bounds,
                    local_data,
                    &args.local_algo,
                    local_maxeval,
                );
                match res2 {
                    Ok((status2, value2)) => {
                        println!("   - Refinement Status: {:?}", status2);
                        println!("   - Refined Mean Squared Error: {:.6}", value2);
                    }
                    Err((e2, val2)) => {
                        eprintln!("   - Refinement failed: {:?}", e2);
                        eprintln!("   - Refinement last value: {:.6}", val2);
                    }
                }
            }
            println!("\n--- Optimal IIR Filters ---");
            println!(
                "| {:<5} | {:<10} | {:<10} | {:<10} | {:<8} |",
                "Filter", "Freq (Hz)", "Q", "Gain (dB)", "Type"
            );
            println!("|-------|------------|------------|------------|----------|");
            let rows = iir::build_sorted_filters(&x, args.iir_hp_pk);
            for (i, r) in rows.iter().enumerate() {
                println!(
                    "| {:<5} | {:<10.2} | {:<10.3} | {:<+10.3} | {:<8} |",
                    i + 1,
                    r.freq,
                    r.q,
                    r.gain,
                    r.kind
                );
            }
            println!("----------------------------------------------------------------");

            // Spacing diagnostics in octaves
            let (sorted_freqs, spacings) =
                optim::compute_sorted_freqs_and_adjacent_octave_spacings(&x);
            if !sorted_freqs.is_empty() {
                println!("\n--- Spacing diagnostics ---");
                println!("Center freqs (Hz): {:?}", sorted_freqs);
                if !spacings.is_empty() {
                    let min_spacing = spacings.iter().cloned().fold(f64::INFINITY, f64::min);
                    println!("Adjacent spacings (oct): {:?}", spacings);
                    println!("Min adjacent spacing: {:.3} oct", min_spacing);
                    if args.min_spacing_oct > 0.0 {
                        for (i, s) in spacings.iter().enumerate() {
                            if *s < args.min_spacing_oct {
                                println!(
                                    "⚠️  Filters {} and {} are {:.3} oct apart (< {:.3})",
                                    i + 1,
                                    i + 2,
                                    s,
                                    args.min_spacing_oct
                                );
                            }
                        }
                    }
                }
            }

            // Plot results if output file is specified
            if let Some(output_path) = &args.output {
                // Extract CEA2034 curves if this is a CEA2034 measurement
                let cea2034_curves = if use_cea {
                    if let Some(ref plot_data) = cea_plot_data {
                        match extract_cea2034_curves(
                            plot_data,
                            &args.measurement.as_ref().unwrap(),
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

            // If CEA2034, compute and print score after optimization using score_peq_approx
            if use_cea {
                let freq = &input_curve.freq;
                let peq_after =
                    iir::compute_peq_response(freq, &x, args.sample_rate, args.iir_hp_pk);
                let metrics_after =
                    compute_cea2034_metrics(freq, &args, &mut cea_plot_data, Some(&peq_after))
                        .await?;
                if let Some(before) = cea_metrics_before {
                    println!(
                        "\n📈  Pre-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.3} sm_pir={:.3}",
                        before.pref_score, before.nbd_on, before.nbd_pir, before.lfx, before.sm_pir
                    );
                    println!(
                        "   Δpref={:+.3} Δnbd_on={:+.3} Δnbd_pir={:+.3} Δlfx={:+.3} Δsm_pir={:+.3}",
                        metrics_after.pref_score - before.pref_score,
                        metrics_after.nbd_on - before.nbd_on,
                        metrics_after.nbd_pir - before.nbd_pir,
                        metrics_after.lfx - before.lfx,
                        metrics_after.sm_pir - before.sm_pir,
                    );
                }
                println!(
                    "\n📈 Post-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.3} sm_pir={:.3}",
                    metrics_after.pref_score,
                    metrics_after.nbd_on,
                    metrics_after.nbd_pir,
                    metrics_after.lfx,
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
