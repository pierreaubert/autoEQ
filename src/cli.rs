//! Common command-line interface definitions shared across binaries

use clap::Parser;
use std::path::PathBuf;

use crate::LossType;

/// Shared CLI arguments for AutoEQ binaries.
#[derive(Parser, Debug, Clone)]
#[command(author, about, long_about = None)]
pub struct Args {
    /// Number of IIR filters to use for optimization.
    #[arg(short = 'n', long, default_value_t = 6)]
    pub num_filters: usize,

    /// Path to the input curve CSV file (format: frequency,spl).
    /// Required unless speaker, version, and measurement are provided for API data.
    #[arg(short, long)]
    pub curve: Option<PathBuf>,

    /// Path to the optional target curve CSV file (format: frequency,spl).
    /// If not provided, a flat 0 dB target is assumed.
    #[arg(short, long)]
    pub target: Option<PathBuf>,

    /// The sample rate for the IIR filters.
    #[arg(short, long, default_value_t = 48000.0)]
    pub sample_rate: f64,

    /// Maximum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 6.0)]
    pub max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 0.5)]
    pub min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 6.0)]
    pub max_q: f64,

    /// Minimum Q factor allowed for each filter.
    #[arg(long, default_value_t = 0.2)]
    pub min_q: f64,

    /// Minimum frequency allowed for each filter.
    #[arg(long, default_value_t = 20.0)]
    pub min_freq: f64,

    /// Maximum frequency allowed for each filter.
    #[arg(long, default_value_t = 20000.0)]
    pub max_freq: f64,

    /// Output PNG file for plotting results.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Speaker name for API data fetching.
    #[arg(long)]
    pub speaker: Option<String>,

    /// Version for API data fetching.
    #[arg(long)]
    pub version: Option<String>,

    /// Measurement type for API data fetching.
    #[arg(long)]
    pub measurement: Option<String>,

    /// Curve name inside CEA2034 plots to use (only when --measurement CEA2034)
    /// e.g., "Listening Window", "On Axis", "Early Reflections". Default: Listening Window
    #[arg(long, default_value = "Listening Window")]
    pub curve_name: String,

    /// Optimization algorithm to use (e.g., isres, cobyla)
    #[arg(long, default_value = "isres")]
    pub algo: String,

    /// Optional population size for population-based algorithms (e.g., ISRES)
    #[arg(long)]
    pub population: Option<usize>,

    /// Maximum number of evaluations for the optimizer
    #[arg(long, default_value_t = 10_000)]
    pub maxeval: usize,

    /// Whether to run a local refinement after global optimization
    #[arg(long, default_value_t = true)]
    pub refine: bool,

    /// Local optimizer to use for refinement (e.g., cobyla)
    #[arg(long, default_value = "cobyla")]
    pub local_algo: String,

    /// Minimum spacing between filter center frequencies in octaves (0 disables)
    #[arg(long, default_value_t = 0.4)]
    pub min_spacing_oct: f64,

    /// Weight for the spacing penalty in the objective function
    #[arg(long, default_value_t = 1.0)]
    pub spacing_weight: f64,

    /// Enable smoothing (regularization) of the inverted target curve
    #[arg(long, default_value_t = false)]
    pub smooth: bool,

    /// Smoothing level as 1/N octave (N in [1..24]). Example: N=6 => 1/6 octave smoothing
    #[arg(long, default_value_t = 6)]
    pub smooth_n: usize,

    /// Loss function to optimize (flat or score).
    #[arg(long, value_enum, default_value_t = LossType::Flat)]
    pub loss: LossType,

    /// If present/true: use a Highpass for the lowest-frequency IIR and do NOT clip the inverted curve.
    /// If false: use all Peak filters and clip the inverted curve on the positive side (current behaviour).
    #[arg(long, default_value_t = false)]
    pub iir_hp_pk: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults() {
        // Simulate no CLI args: use default values
        use clap::Parser as _;
        let args = Args::parse_from(["autoeq-test"]);
        assert_eq!(args.num_filters, 6);
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.maxeval, 10_000);
        assert_eq!(args.curve_name, "Listening Window");
        assert!(!args.iir_hp_pk);
    }
}
