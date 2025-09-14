//! CLI tool for computing headphone loss from frequency response files
//!
//! Usage:
//!   cargo run --example headphone_loss_demo -- --spl <file> [--target <file>]

use autoeq::loss::headphone_loss;
use autoeq::read::{load_frequency_response, normalize_both_curves};

use clap::Parser;
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

    // Compute headphone loss
    let score = if let Some(target_path) = args.target {
        // Load target data
        println!("Loading target data from: {:?}", target_path);
        let (target_freq, target_spl) = load_frequency_response(&target_path)?;
        println!(
            "  Loaded {} frequency points from {:.1} Hz to {:.1} Hz",
            target_freq.len(),
            target_freq[0],
            target_freq[target_freq.len() - 1]
        );

        let (loss_freq, deviation) = normalize_both_curves(&freq, &spl, Some((&target_freq, &target_spl)));
        headphone_loss(&loss_freq, &deviation)
    } else {
        let (loss_freq, deviation) = normalize_both_curves(&freq, &spl, None);
        headphone_loss(&loss_freq, &deviation)
    };

    // Print results
    println!("\n{}", "=".repeat(50));
    println!("Headphone Loss Score: {:.3}", score);
    println!("{}", "=".repeat(50));

    Ok(())
}
