use clap::Parser;
use ndarray::Array1;
use std::error::Error;
use std::path::PathBuf;

mod optim;
mod plot;
mod read;
use optim::ObjectiveData;

/// A command-line tool to find optimal IIR filters to match a frequency curve.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of IIR filters to use for optimization (lowest-frequency filter is Highpass, others are Peak).
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
    #[arg(long, default_value_t = 20.0)]
    max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 0.0)]
    min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 10.0)]
    max_q: f64,

    /// Minimum Q factor allowed for each filter.
    #[arg(long, default_value_t = 0.1)]
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
    #[arg(long, default_value_t = 0.25)]
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
}

/// Extract sorted center frequencies from parameter vector and compute adjacent spacings in octaves.
fn compute_sorted_freqs_and_adjacent_octave_spacings(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() / 3;
    let mut freqs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        freqs.push(x[i * 3]);
    }
    freqs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let spacings: Vec<f64> = if freqs.len() < 2 {
        Vec::new()
    } else {
        freqs
            .windows(2)
            .map(|w| (w[1].max(1e-9) / w[0].max(1e-9)).log2().abs())
            .collect()
    };
    (freqs, spacings)
}

#[cfg(test)]
mod spacing_diag_tests {
    use super::compute_sorted_freqs_and_adjacent_octave_spacings;

    #[test]
    fn adjacent_octave_spacings_basic() {
        // x: [f,q,g, f,q,g, f,q,g]
        let x = [100.0, 1.0, 0.0, 200.0, 1.0, 0.0, 400.0, 1.0, 0.0];
        let (freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        assert_eq!(freqs, vec![100.0, 200.0, 400.0]);
        assert!((spacings[0] - 1.0).abs() < 1e-12);
        assert!((spacings[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn adjacent_octave_spacings_two_filters() {
        let x = [1000.0, 1.0, 0.0, 1100.0, 1.0, 0.0];
        let (_freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        // log2(1100/1000) ~ 0.1375 octaves
        assert!(spacings.len() == 1);
        assert!((spacings[0] - (1100.0_f64 / 1000.0).log2().abs()).abs() < 1e-12);
    }
}

/// A struct to hold frequency and SPL data.
#[derive(Debug, Clone)]
pub struct Curve {
    pub freq: Array1<f64>,
    pub spl: Array1<f64>,
}

/// Linear interpolation function.
fn interpolate(
    target_freqs: &Array1<f64>,
    source_freqs: &Array1<f64>,
    source_spls: &Array1<f64>,
) -> Array1<f64> {
    let mut result = Array1::zeros(target_freqs.len());

    for (i, &target_freq) in target_freqs.iter().enumerate() {
        // Find the two nearest points in the source data
        let mut left_idx = 0;
        let mut right_idx = source_freqs.len() - 1;

        // Binary search for the closest points
        if target_freq <= source_freqs[0] {
            // Target frequency is below the range, use the first point
            result[i] = source_spls[0];
        } else if target_freq >= source_freqs[source_freqs.len() - 1] {
            // Target frequency is above the range, use the last point
            result[i] = source_spls[source_freqs.len() - 1];
        } else {
            // Find the two points that bracket the target frequency
            for j in 1..source_freqs.len() {
                if source_freqs[j] >= target_freq {
                    left_idx = j - 1;
                    right_idx = j;
                    break;
                }
            }

            // Linear interpolation
            let freq_left = source_freqs[left_idx];
            let freq_right = source_freqs[right_idx];
            let spl_left = source_spls[left_idx];
            let spl_right = source_spls[right_idx];

            let t = (target_freq - freq_left) / (freq_right - freq_left);
            result[i] = spl_left + t * (spl_right - spl_left);
        }
    }

    result
}

/// Clamp only positive dB values to +max_db, leave negatives unchanged.
fn clamp_positive_only(arr: &Array1<f64>, max_db: f64) -> Array1<f64> {
    arr.mapv(|v| if v > 0.0 { v.min(max_db) } else { v })
}

/// Simple 1/N-octave smoothing: for each frequency f_i, average values whose
/// frequency lies within [f_i * 2^(-1/(2N)), f_i * 2^(1/(2N))].
fn smooth_one_over_n_octave(
    freqs: &Array1<f64>,
    values: &Array1<f64>,
    n: usize,
) -> Array1<f64> {
    let n = n.max(1);
    let half_win = (2.0_f64).powf(1.0 / (2.0 * n as f64));
    let mut out = Array1::zeros(values.len());
    for i in 0..freqs.len() {
        let f = freqs[i].max(1e-12);
        let lo = f / half_win;
        let hi = f * half_win;
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for j in 0..freqs.len() {
            let fj = freqs[j];
            if fj >= lo && fj <= hi {
                sum += values[j];
                cnt += 1;
            }
        }
        out[i] = if cnt > 0 { sum / cnt as f64 } else { values[i] };
    }
    out
}

#[cfg(test)]
mod clamp_and_smooth_tests {
    use super::{clamp_positive_only, smooth_one_over_n_octave};
    use ndarray::Array1;

    #[test]
    fn clamp_positive_only_clamps_only_positive_side() {
        let arr = Array1::from(vec![-15.0, -1.0, 0.0, 1.0, 10.0, 25.0]);
        let out = clamp_positive_only(&arr, 12.0);
        assert_eq!(out.to_vec(), vec![-15.0, -1.0, 0.0, 1.0, 10.0, 12.0]);
    }

    #[test]
    fn smooth_one_over_n_octave_basic_monotonic() {
        // Simple check: with N large, window small -> output close to input
        let freqs = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let vals = Array1::from(vec![0.0, 1.0, 0.0, -1.0]);
        let out = smooth_one_over_n_octave(&freqs, &vals, 24);
        // Expect no drastic change
        for (o, v) in out.iter().zip(vals.iter()) {
            assert!((o - v).abs() <= 0.5);
        }
    }
}

/// A struct to hold filter data.
#[derive(Debug, Clone)]
struct FilterRow {
    freq: f64,
    q: f64,
    gain: f64,
    kind: &'static str,
}

fn build_sorted_filters(x: &[f64]) -> Vec<FilterRow> {
    let mut rows: Vec<FilterRow> = Vec::with_capacity(x.len() / 3);
    for i in 0..(x.len() / 3) {
        let freq = x[i * 3];
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];
        rows.push(FilterRow {
            freq,
            q,
            gain,
            kind: "Peak",
        });
    }
    rows.sort_by(|a, b| {
        a.freq
            .partial_cmp(&b.freq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Mark the lowest-frequency filter as Highpass for display purposes
    if !rows.is_empty() {
        rows[0].kind = "Highpass";
    }
    rows
}

#[cfg(test)]
mod filter_tests {
    use super::build_sorted_filters;

    #[test]
    fn sorts_by_freq_and_sets_type() {
        let x = vec![1000.0, 1.0, 0.0, 100.0, 2.0, 1.0, 500.0, 0.5, -1.0];
        let rows = build_sorted_filters(&x);
        let freqs: Vec<f64> = rows.iter().map(|r| r.freq).collect();
        assert_eq!(freqs, vec![100.0, 500.0, 1000.0]);
        assert!(rows[0].kind == "Highpass");
        assert!(rows.iter().skip(1).all(|r| r.kind == "Peak"));
        assert!((rows[0].q - 2.0).abs() < 1e-12 && (rows[0].gain - 1.0).abs() < 1e-12);
        assert!((rows[1].q - 0.5).abs() < 1e-12 && (rows[1].gain + 1.0).abs() < 1e-12);
        assert!((rows[2].q - 1.0).abs() < 1e-12 && (rows[2].gain - 0.0).abs() < 1e-12);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // 1. Load the input curve (the one we want to match).
    // Use API data if speaker, version, and measurement are provided, otherwise use CSV file
    let input_curve = if let (Some(speaker), Some(version), Some(measurement)) =
        (&args.speaker, &args.version, &args.measurement)
    {
        read::fetch_curve_from_api(speaker, version, measurement, &args.curve_name).await?
    } else {
        // If no API parameters are provided, curve file must be provided
        let curve_path = args.curve.as_ref().ok_or(
            "Either --curve or all of --speaker, --version, and --measurement must be provided",
        )?;
        read::read_curve_from_csv(curve_path)?
    };

    // 2. Build inverted target from the selected curve, with positive-only clamp and optional smoothing.
    // Base target is flat 0 dB unless a target file was provided (we still invert the selected curve relative to it).
    let base_target = if let Some(target_path) = args.target {
        let target_curve = read::read_curve_from_csv(&target_path)?;
        println!(
            "✅ Loaded target curve with {} points.",
            target_curve.freq.len()
        );
        interpolate(&input_curve.freq, &target_curve.freq, &target_curve.spl)
    } else {
        println!("✅ No target curve provided, using a flat 0 dB target.");
        Array1::zeros(input_curve.spl.len())
    };

    // Inverted curve relative to base target
    let mut inverted = base_target.clone() - input_curve.spl.clone();
    // Clamp only positive values to +max_db
    inverted = clamp_positive_only(&inverted, args.max_db);

    // Optional smoothing regularization of the inverted curve
    let mut smoothed: Option<Array1<f64>> = None;
    if args.smooth {
        smoothed = Some(smooth_one_over_n_octave(&input_curve.freq, &inverted, args.smooth_n));
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
    };

    // Each filter has 3 parameters: frequency, Q, and gain.
    let num_params = args.num_filters * 3;

    // Define the bounds for each parameter.
    // [freq_min, q_min, gain_min, freq_min, q_min, gain_min, ...]
    let mut lower_bounds = Vec::with_capacity(num_params);
    // [freq_max, q_max, gain_max, freq_max, q_max, gain_max, ...]
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
        // Initial gain respects minimum absolute amplitude if specified
        let gain = if args.min_db > 0.0 { args.min_db.copysign(1.0) } else { 0.1 };
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
                inv2 = clamp_positive_only(&inv2, args.max_db);
                let target_error2 = if args.smooth {
                    smooth_one_over_n_octave(&input_curve.freq, &inv2, args.smooth_n)
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
            let rows = build_sorted_filters(&x);
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
            let (sorted_freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
            if !sorted_freqs.is_empty() {
                println!("\n--- Spacing diagnostics ---");
                println!("Center freqs (Hz): {:?}", sorted_freqs);
                if !spacings.is_empty() {
                    let min_spacing = spacings
                        .iter()
                        .cloned()
                        .fold(f64::INFINITY, f64::min);
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
                )
                .await?;
            }
        }
        Err((e, final_value)) => {
            eprintln!("\n❌ Optimization failed: {:?}", e);
            eprintln!("   - Final Mean Squared Error: {:.6}", final_value);
        }
    }

    Ok(())
}

// Plotting moved to plot module
