//! CLI tool for computing headphone loss from frequency response files
//!
//! Usage:
//!   cargo run --example headphone_loss_demo -- --spl <file> [--target <file>]

use autoeq::loss::{headphone_loss, headphone_loss_with_target};
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
/// Expected format: frequency,spl (comma or whitespace separated)
fn load_frequency_response(path: &PathBuf) -> Result<(Array1<f64>, Array1<f64>), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    
    let mut frequencies = Vec::new();
    let mut spl_values = Vec::new();
    
    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let line = line.trim();
        
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        
        // Skip header if it contains text
        if line_num == 0 && (line.contains("freq") || line.contains("Freq") || line.contains("Hz")) {
            continue;
        }
        
        // Parse line (handle both comma and whitespace separation)
        let parts: Vec<&str> = if line.contains(',') {
            line.split(',').map(|s| s.trim()).collect()
        } else {
            line.split_whitespace().collect()
        };
        
        if parts.len() >= 2 {
            // Try to parse frequency and SPL
            if let (Ok(freq), Ok(spl)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                frequencies.push(freq);
                spl_values.push(spl);
            }
        }
    }
    
    if frequencies.is_empty() {
        return Err("No valid frequency response data found in file".into());
    }
    
    Ok((Array1::from_vec(frequencies), Array1::from_vec(spl_values)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Load SPL data
    println!("Loading SPL data from: {:?}", args.spl);
    let (freq, spl) = load_frequency_response(&args.spl)?;
    println!("  Loaded {} frequency points from {:.1} Hz to {:.1} Hz", 
             freq.len(), freq[0], freq[freq.len()-1]);
    
    let score = if let Some(target_path) = args.target {
        // Load target data and compute loss relative to target
        println!("Loading target data from: {:?}", target_path);
        let (target_freq, target_spl) = load_frequency_response(&target_path)?;
        
        // Check if frequencies match
        if freq.len() != target_freq.len() {
            eprintln!("Warning: SPL has {} points, target has {} points", 
                     freq.len(), target_freq.len());
        }
        
        // For simplicity, assume same frequency grid or interpolate if needed
        // Here we'll just use the SPL frequencies and assume target is on same grid
        if freq.len() == target_freq.len() && freq.iter().zip(target_freq.iter())
            .all(|(f1, f2)| (f1 - f2).abs() < 0.1) {
            // Same frequency grid
            println!("  Computing headphone loss relative to target curve");
            headphone_loss_with_target(&freq, &spl, &target_spl)
        } else {
            // Different grids - would need interpolation (simplified here)
            eprintln!("Error: Frequency grids don't match. Interpolation not implemented.");
            eprintln!("  SPL frequencies: {:.1} Hz to {:.1} Hz ({} points)", 
                     freq[0], freq[freq.len()-1], freq.len());
            eprintln!("  Target frequencies: {:.1} Hz to {:.1} Hz ({} points)", 
                     target_freq[0], target_freq[target_freq.len()-1], target_freq.len());
            return Err("Frequency grid mismatch".into());
        }
    } else {
        // Compute absolute headphone loss (assumes SPL is deviation from ideal)
        println!("  Computing absolute headphone loss");
        headphone_loss(&freq, &spl)
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
