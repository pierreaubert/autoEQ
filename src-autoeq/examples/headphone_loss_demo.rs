//! CLI tool for computing headphone loss from frequency response files
//!
//! Usage:
//!   cargo run --example headphone_loss_demo -- --spl <file> [--target <file>]

use autoeq::loss::headphone_loss;
use clap::Parser;
use ndarray::Array1;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "headphone_loss_demo",
    about = "Compute headphone preference score from frequency response measurements",
    long_about = "Computes the headphone preference loss score based on the model from \n'A Statistical Model that Predicts Listeners' Preference Ratings of In-Ear Headphones' \nby Sean Olive et al. Lower scores indicate better predicted preference."
)]
struct Args {
    /// Path to SPL (frequency response) file (CSV or text with freq,spl columns)
    #[arg(long)]
    spl: PathBuf,

    /// Optional path to target frequency response file (CSV or text with freq,spl columns)
    #[arg(long)]
    target: Option<PathBuf>,
}

/// Load frequency response data from a CSV or text file
/// Expected formats:
/// - 2 columns: frequency, spl
/// - 4 columns: freq_left, spl_left, freq_right, spl_right (averaged)
fn load_frequency_response(
    path: &PathBuf,
) -> Result<(Array1<f64>, Array1<f64>), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    let mut detected_columns = 0;

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Skip header if it contains text
        if line_num == 0 && (line.contains("freq") || line.contains("Freq") || line.contains("Hz"))
        {
            continue;
        }

        // Parse line (handle both comma and whitespace separation)
        let parts: Vec<&str> = if line.contains(',') {
            line.split(',').map(|s| s.trim()).collect()
        } else {
            line.split_whitespace().collect()
        };

        // Detect number of columns on first data line
        if detected_columns == 0 && parts.len() >= 2 {
            detected_columns = parts.len();
            if detected_columns == 4 {
                println!(
                    "    Detected 4-column format (stereo) - averaging left and right channels"
                );
            }
        }

        if detected_columns == 2 && parts.len() >= 2 {
            // 2-column format: freq, spl
            if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                frequencies.push(freq);
                spl_values.push(spl);
            }
        } else if detected_columns == 4 && parts.len() >= 4 {
            // 4-column format: freq_left, spl_left, freq_right, spl_right
            // Assume frequencies are the same for left and right, average the SPL
            if let (Ok(freq_l), Ok(spl_l), Ok(_freq_r), Ok(spl_r)) = (
                parts[0].parse::<f64>(),
                parts[1].parse::<f64>(),
                parts[2].parse::<f64>(),
                parts[3].parse::<f64>(),
            ) {
                frequencies.push(freq_l);
                spl_values.push((spl_l + spl_r) / 2.0); // Average left and right
            }
        }
    }

    if frequencies.is_empty() {
        return Err("No valid frequency response data found in file".into());
    }

    Ok((Array1::from_vec(frequencies), Array1::from_vec(spl_values)))
}

/// Interpolate frequency response to a standard grid using linear interpolation in log space
///
/// # Arguments
/// * `freq_in` - Input frequency points
/// * `spl_in` - Input SPL values
/// * `freq_out` - Target frequency grid for interpolation
///
/// # Returns
/// * Interpolated SPL values on the target grid
fn interpolate_log_space(
    freq_in: &Array1<f64>,
    spl_in: &Array1<f64>,
    freq_out: &Array1<f64>,
) -> Array1<f64> {
    let n_out = freq_out.len();
    let n_in = freq_in.len();
    let mut spl_out = Array1::zeros(n_out);

    // Convert to log space for interpolation
    let log_freq_in: Vec<f64> = freq_in.iter().map(|f| f.ln()).collect();
    let log_freq_out: Vec<f64> = freq_out.iter().map(|f| f.ln()).collect();

    for i in 0..n_out {
        let target_log_freq = log_freq_out[i];

        // Find surrounding points for interpolation
        if target_log_freq <= log_freq_in[0] {
            // Extrapolate from first two points
            if n_in >= 2 {
                let slope = (spl_in[1] - spl_in[0]) / (log_freq_in[1] - log_freq_in[0]);
                spl_out[i] = spl_in[0] + slope * (target_log_freq - log_freq_in[0]);
            } else {
                spl_out[i] = spl_in[0];
            }
        } else if target_log_freq >= log_freq_in[n_in - 1] {
            // Extrapolate from last two points
            if n_in >= 2 {
                let slope = (spl_in[n_in - 1] - spl_in[n_in - 2])
                    / (log_freq_in[n_in - 1] - log_freq_in[n_in - 2]);
                spl_out[i] = spl_in[n_in - 1] + slope * (target_log_freq - log_freq_in[n_in - 1]);
            } else {
                spl_out[i] = spl_in[n_in - 1];
            }
        } else {
            // Linear interpolation between surrounding points
            let mut j = 0;
            while j < n_in - 1 && log_freq_in[j + 1] < target_log_freq {
                j += 1;
            }

            // Interpolate between j and j+1
            let t = (target_log_freq - log_freq_in[j]) / (log_freq_in[j + 1] - log_freq_in[j]);
            spl_out[i] = spl_in[j] * (1.0 - t) + spl_in[j + 1] * t;
        }
    }

    spl_out
}

/// Create a standard logarithmic frequency grid
fn create_log_frequency_grid(n_points: usize, f_min: f64, f_max: f64) -> Array1<f64> {
    Array1::logspace(10.0, f_min.log10(), f_max.log10(), n_points)
}

/// Normalize frequency response by subtracting mean in 100Hz-12kHz range
fn normalize_response(
    freq: &Array1<f64>,
    spl: &Array1<f64>,
    f_min: f64,
    f_max: f64,
) -> Array1<f64> {
    let mut sum = 0.0;
    let mut count = 0;

    // Calculate mean in the specified frequency range
    for i in 0..freq.len() {
        if freq[i] >= f_min && freq[i] <= f_max {
            sum += spl[i];
            count += 1;
        }
    }

    if count > 0 {
        let mean = sum / count as f64;
        spl - mean // Subtract mean from all values
    } else {
        spl.clone() // Return unchanged if no points in range
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load SPL data
    println!("Loading SPL data from: {:?}", args.spl);
    let (freq, spl) = load_frequency_response(&args.spl)?;
    println!(
        "  Loaded {} frequency points from {:.1} Hz to {:.1} Hz",
        freq.len(),
        freq[0],
        freq[freq.len() - 1]
    );

    let score = if let Some(target_path) = args.target {
        // Load target data and compute loss relative to target
        println!("Loading target data from: {:?}", target_path);
        let (target_freq, target_spl) = load_frequency_response(&target_path)?;
        println!(
            "  Loaded {} frequency points from {:.1} Hz to {:.1} Hz",
            target_freq.len(),
            target_freq[0],
            target_freq[target_freq.len() - 1]
        );

        // Check if frequencies match
        let frequencies_match = freq.len() == target_freq.len()
            && freq
                .iter()
                .zip(target_freq.iter())
                .all(|(f1, f2)| (f1 - f2).abs() / f1.max(*f2) < 0.01); // 1% tolerance

        // Normalize both curves before comparison (mean in 100Hz-12kHz range)
        println!("  Normalizing curves (mean in 100Hz-12kHz range)");

        if frequencies_match {
            // Same frequency grid - normalize and use directly
            let spl_norm = normalize_response(&freq, &spl, 100.0, 12000.0);
            let target_norm = normalize_response(&target_freq, &target_spl, 100.0, 12000.0);

            println!("  Frequency grids match - computing headphone loss relative to target curve");
            // Compute deviation from normalized target
            let deviation = &spl_norm - &target_norm;
            headphone_loss(&freq, &deviation)
        } else {
            // Different grids - resample both to common grid
            println!(
                "  Frequency grids differ - resampling to common 200-point log grid (20-20000 Hz)"
            );

            // Create standard grid: 200 points from 20 Hz to 20 kHz
            let standard_freq = create_log_frequency_grid(200, 20.0, 20000.0);

            // Interpolate both curves to standard grid
            let spl_interp = interpolate_log_space(&freq, &spl, &standard_freq);
            let target_interp = interpolate_log_space(&target_freq, &target_spl, &standard_freq);

            // Normalize after interpolation
            let spl_norm = normalize_response(&standard_freq, &spl_interp, 100.0, 12000.0);
            let target_norm = normalize_response(&standard_freq, &target_interp, 100.0, 12000.0);

            println!(
                "  Interpolation complete - computing headphone loss relative to target curve"
            );
            // Compute deviation from normalized target
            let deviation = &spl_norm - &target_norm;
            headphone_loss(&standard_freq, &deviation)
        }
    } else {
        // Compute absolute headphone loss
        // If frequency grid is sparse, resample to standard grid
        if freq.len() < 50 {
            println!(
                "  Sparse frequency grid detected ({} points) - resampling to 200-point log grid",
                freq.len()
            );
            let standard_freq = create_log_frequency_grid(200, 20.0, 20000.0);
            let spl_interp = interpolate_log_space(&freq, &spl, &standard_freq);
            println!("  Computing absolute headphone loss on resampled data");
            headphone_loss(&standard_freq, &spl_interp)
        } else {
            println!("  Computing absolute headphone loss");
            headphone_loss(&freq, &spl)
        }
    };

    // Print results
    println!("\n{}", "=".repeat(50));
    println!("Headphone Loss Score: {:.3}", score);
    println!("{}", "=".repeat(50));
    println!("\nInterpretation:");
    println!("  Lower scores indicate better predicted preference");
    println!("  Score components:");
    println!("    - Slope deviation from -1 dB/octave");
    println!("    - RMS deviation in frequency bands");
    println!("    - Peak-to-peak variation penalties");
    println!("    - Frequency-weighted (bass/midrange > treble)");

    Ok(())
}
