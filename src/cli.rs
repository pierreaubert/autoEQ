//! AutoEQ - A library for audio equalization and filter optimization
//! Common command-line interface definitions shared across binaries
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

use crate::LossType;
use clap::Parser;
use std::path::PathBuf;

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
    #[arg(long, default_value_t = 6.0, value_parser = parse_nonnegative_f64)]
    pub max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 1.0, value_parser = parse_strictly_positive_f64)]
    pub min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 6.0)]
    pub max_q: f64,

    /// Minimum Q factor allowed for each filter.
    #[arg(long, default_value_t = 1.0)]
    pub min_q: f64,

    /// Minimum frequency allowed for each filter.
    #[arg(long, default_value_t = 60.0)]
    pub min_freq: f64,

    /// Maximum frequency allowed for each filter.
    #[arg(long, default_value_t = 16000.0)]
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
    #[arg(long, default_value_t = 30_000)]
    pub population: usize,

    /// Maximum number of evaluations for the optimizer
    #[arg(long, default_value_t = 200_000)]
    pub maxeval: usize,

    /// Whether to run a local refinement after global optimization
    #[arg(long, default_value_t = false)]
    pub refine: bool,

    /// Local optimizer to use for refinement (e.g., cobyla)
    #[arg(long, default_value = "cobyla")]
    pub local_algo: String,

    /// Minimum spacing between filter center frequencies in octaves (0 disables)
    #[arg(long, default_value_t = 0.5)]
    pub min_spacing_oct: f64,

    /// Weight for the spacing penalty in the objective function
    #[arg(long, default_value_t = 20.0)]
    pub spacing_weight: f64,

    /// Enable smoothing (regularization) of the inverted target curve
    #[arg(long, default_value_t = true)]
    pub smooth: bool,

    /// Smoothing level as 1/N octave (N in [1..24]). Example: N=6 => 1/6 octave smoothing
    #[arg(long, default_value_t = 2)]
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
        let args = Args::parse_from(["autoeq-test"]);
        assert_eq!(args.num_filters, 6);
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.maxeval, 10_000);
        assert_eq!(args.curve_name, "Listening Window");
        assert!(!args.iir_hp_pk);
    }

    #[test]
    fn min_db_must_be_strictly_positive_zero_rejected() {
        let res = Args::try_parse_from(["autoeq-test", "--min-db", "0.0"]);
        assert!(res.is_err());
    }

    #[test]
    fn min_db_must_be_strictly_positive_negative_rejected() {
        let res = Args::try_parse_from(["autoeq-test", "--min-db", "-0.1"]);
        assert!(res.is_err());
    }

    #[test]
    fn max_db_allows_zero() {
        let res = Args::try_parse_from(["autoeq-test", "--max-db", "0.0"]);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().max_db, 0.0);
    }

    #[test]
    fn max_db_rejects_negative() {
        let res = Args::try_parse_from(["autoeq-test", "--max-db", "-1.0"]);
        assert!(res.is_err());
    }
}

// Custom value parser to enforce strictly positive f64
fn parse_strictly_positive_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("invalid float: {s}"))?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err("value must be strictly positive (> 0)".to_string())
    }
}

// Custom value parser to enforce non-negative f64 (>= 0)
fn parse_nonnegative_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.parse().map_err(|_| format!("invalid float: {s}"))?;
    if v >= 0.0 {
        Ok(v)
    } else {
        Err("value must be non-negative (>= 0)".to_string())
    }
}
