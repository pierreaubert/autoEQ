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
use super::optim::{get_all_algorithms, AlgorithmType};
use clap::Parser;
use std::path::PathBuf;
use std::process;

/// Shared CLI arguments for AutoEQ binaries.
#[derive(Parser, Debug, Clone)]
#[command(author, about, long_about = None)]
pub struct Args {
    /// Number of IIR filters to use for optimization.
    #[arg(short = 'n', long, default_value_t = 7)]
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
    #[arg(long, default_value_t = 3.0, value_parser = parse_nonnegative_f64)]
    pub max_db: f64,

    /// Minimum absolute dB gain allowed for each filter.
    #[arg(long, default_value_t = 1.0, value_parser = parse_strictly_positive_f64)]
    pub min_db: f64,

    /// Maximum Q factor allowed for each filter.
    #[arg(long, default_value_t = 3.0)]
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
    #[arg(long, default_value = "nlopt:cobyla")]
    pub algo: String,

    /// Optional population size for population-based algorithms (e.g., ISRES)
    #[arg(long, default_value_t = 300)]
    pub population: usize,

    /// Maximum number of evaluations for the optimizer
    #[arg(long, default_value_t = 2_000)]
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

    /// Display list of available optimization algorithms with descriptions and exit.
    #[arg(long, default_value_t = false)]
    pub algo_list: bool,
}

/// Display available optimization algorithms with descriptions and exit
pub fn display_algorithm_list() -> ! {
    println!("Available Optimization Algorithms");
    println!("=================================\n");

    let algorithms = get_all_algorithms();

    // Group algorithms by library
    let mut nlopt_algos = Vec::new();
    let mut metaheuristics_algos = Vec::new();
    let mut autoeq_algos = Vec::new();

    for algo in &algorithms {
        match algo.library {
            "NLOPT" => nlopt_algos.push(algo),
            "Metaheuristics" => metaheuristics_algos.push(algo),
            "AutoEQ" => autoeq_algos.push(algo),
            _ => {} // Skip unknown libraries
        }
    }

    // Display NLOPT algorithms
    if !nlopt_algos.is_empty() {
        println!("📊 NLOPT Library Algorithms:");

        // Separate global and local algorithms
        let mut global = Vec::new();
        let mut local = Vec::new();

        for algo in nlopt_algos {
            match algo.algorithm_type {
                AlgorithmType::Global => global.push(algo),
                AlgorithmType::Local => local.push(algo),
            }
        }

        if !global.is_empty() {
            println!("   🌍 Global Optimizers (best for exploring solution space):");
            for algo in global {
                print!("   - {:<20}", algo.name);
                print!(" | Constraints: ");
                if algo.supports_nonlinear_constraints {
                    print!("✅ Nonlinear");
                } else if algo.supports_linear_constraints {
                    print!("🔶 Linear only");
                } else {
                    print!("❌ None");
                }

                // Add specific descriptions
                let description = match algo.name {
                    "nlopt:isres" => " | Improved Stochastic Ranking Evolution Strategy (recommended)",
                    "nlopt:ags" => " | Adaptive Geometric Search",
                    "nlopt:origdirect" => " | DIRECT global optimization (original version)",
                    "nlopt:crs2lm" => " | Controlled Random Search with local mutation",
                    "nlopt:direct" => " | DIRECT global optimization",
                    "nlopt:directl" => " | DIRECT-L (locally biased version)",
                    "nlopt:gmlsl" => " | Global Multi-Level Single-Linkage",
                    "nlopt:gmlsllds" => " | GMLSL with low-discrepancy sequence",
                    "nlopt:stogo" => " | Stochastic Global Optimization",
                    "nlopt:stogorand" => " | StoGO with randomized search",
                    _ => "",
                };
                println!("{}", description);
            }
            println!();
        }

        if !local.is_empty() {
            println!("   🎯 Local Optimizers (fast refinement from good starting points):");
            for algo in local {
                print!("   - {:<20}", algo.name);
                print!(" | Constraints: ");
                if algo.supports_nonlinear_constraints {
                    print!("✅ Nonlinear");
                } else if algo.supports_linear_constraints {
                    print!("🔶 Linear only");
                } else {
                    print!("❌ None");
                }

                let description = match algo.name {
                    "nlopt:cobyla" => " | Constrained Optimization BY Linear Approximations (recommended for local)",
                    "nlopt:bobyqa" => " | Bound Optimization BY Quadratic Approximation",
                    "nlopt:neldermead" => " | Nelder-Mead simplex algorithm",
                    "nlopt:sbplx" => " | Subplex (variant of Nelder-Mead)",
                    "nlopt:slsqp" => " | Sequential Least SQuares Programming",
                    _ => "",
                };
                println!("{}", description);
            }
            println!();
        }
    }

    // Display Metaheuristics algorithms
    if !metaheuristics_algos.is_empty() {
        println!("🧬 Metaheuristics Library Algorithms:");
        println!("   Nature-inspired global optimization (penalty-based constraints)\n");

        for algo in metaheuristics_algos {
            print!("   - {:<20}", algo.name);
            let description = match algo.name {
                "mh:de" => " | Differential Evolution (robust, good convergence)",
                "mh:pso" => " | Particle Swarm Optimization (fast exploration)",
                "mh:rga" => " | Real-coded Genetic Algorithm (diverse search)",
                "mh:tlbo" => " | Teaching-Learning-Based Optimization (parameter-free)",
                "mh:firefly" => " | Firefly Algorithm (multi-modal problems)",
                _ => "",
            };
            println!("{}", description);
        }
        println!();
    }

    // Display AutoEQ algorithms
    if !autoeq_algos.is_empty() {
        println!("🎵 AutoEQ Custom Algorithms:");
        println!("   Specialized algorithms developed for audio filter optimization\n");

        for algo in autoeq_algos {
            print!("   - {:<20}", algo.name);
            print!(" | Constraints: ");
            if algo.supports_nonlinear_constraints {
                print!("✅ Nonlinear");
            } else {
                print!("❌ Penalty-based");
            }

            let description = match algo.name {
                "autoeq:de" => " | Adaptive DE with constraint handling (experimental)",
                _ => "",
            };
            println!("{}", description);
        }
        println!();
    }

    println!("Usage Examples:");
    println!("==============\n");
    println!("  # Use ISRES (recommended global optimizer):");
    println!("  autoeq --algo nlopt:isres --curve input.csv\n");
    println!("  # Use COBYLA (fast local optimizer):");
    println!("  autoeq --algo nlopt:cobyla --curve input.csv\n");
    println!("  # Use Differential Evolution from metaheuristics:");
    println!("  autoeq --algo mh:de --curve input.csv\n");
    println!("  # Backward compatibility (maps to nlopt:cobyla):");
    println!("  autoeq --algo cobyla --curve input.csv\n");

    println!("Recommendations:");
    println!("===============\n");
    println!("  🎯 For best results: nlopt:isres (global) + --refine with nlopt:cobyla (local)");
    println!("  ⚡ For speed: nlopt:cobyla (if you have a good initial guess)");
    println!("  🧪 For experimentation: mh:de or mh:pso from metaheuristics library");
    println!("  ⚖️  For constrained problems: Prefer algorithms with ✅ Nonlinear constraint support");

    process::exit(0);
}

/// Validate CLI arguments and exit with error message if validation fails
pub fn validate_args(args: &Args) -> Result<(), String> {
    // Check if algorithm is valid
    if let Some(_) = crate::optim::find_algorithm_info(&args.algo) {
        // Algorithm is valid
    } else {
        return Err(format!(
            "Unknown algorithm: '{}'. Use --algo-list to see available algorithms.",
            args.algo
        ));
    }

    // Check if local algorithm is valid (when refine is enabled)
    if args.refine {
        if let Some(_) = crate::optim::find_algorithm_info(&args.local_algo) {
            // Local algorithm is valid
        } else {
            return Err(format!(
                "Unknown local algorithm: '{}'. Use --algo-list to see available algorithms.",
                args.local_algo
            ));
        }
    }

    // Check min/max Q factor constraints
    if args.min_q > args.max_q {
        return Err(format!(
            "Invalid Q factor range: min_q ({}) must be <= max_q ({})",
            args.min_q, args.max_q
        ));
    }

    // Check min/max frequency constraints
    if args.min_freq > args.max_freq {
        return Err(format!(
            "Invalid frequency range: min_freq ({}) must be <= max_freq ({})",
            args.min_freq, args.max_freq
        ));
    }

    // Check min/max dB constraints
    if args.min_db > args.max_db {
        return Err(format!(
            "Invalid dB range: min_db ({}) must be <= max_db ({})",
            args.min_db, args.max_db
        ));
    }

    // Check frequency bounds (reasonable audio range)
    if args.min_freq < 20.0 {
        return Err(format!(
            "Invalid min_freq: {} Hz. Must be >= 20 Hz (reasonable audio range)",
            args.min_freq
        ));
    }

    if args.max_freq > 20000.0 {
        return Err(format!(
            "Invalid max_freq: {} Hz. Must be <= 20,000 Hz (reasonable audio range)",
            args.max_freq
        ));
    }

    // Check smoothing parameters
    if args.smooth_n < 1 || args.smooth_n > 24 {
        return Err(format!(
            "Invalid smooth_n: {}. Must be in range [1..24]",
            args.smooth_n
        ));
    }

    // Check that population size is reasonable
    if args.population == 0 {
        return Err("Population size must be > 0".to_string());
    }

    // Check that maxeval is reasonable
    if args.maxeval == 0 {
        return Err("Maximum evaluations must be > 0".to_string());
    }

    // Check that num_filters is reasonable
    if args.num_filters == 0 {
        return Err("Number of filters must be > 0".to_string());
    }

    if args.num_filters > 50 {
        return Err(format!(
            "Number of filters ({}) is very high. Consider using <= 50 filters for reasonable performance",
            args.num_filters
        ));
    }

    Ok(())
}

/// Validate arguments and exit with error if validation fails
pub fn validate_args_or_exit(args: &Args) {
    if let Err(error) = validate_args(args) {
        eprintln!("❌ Validation Error: {}", error);
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults() {
        // Simulate no CLI args: use default values
        let args = Args::parse_from(["autoeq-test"]);
        assert_eq!(args.num_filters, 7);
        assert_eq!(args.sample_rate, 48000.0);
        assert_eq!(args.maxeval, 2000);
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

    #[test]
    fn validate_args_valid_config() {
        let args = Args::parse_from(["autoeq-test"]);
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn validate_args_invalid_algorithm() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.algo = "invalid-algo".to_string();
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown algorithm"));
    }

    #[test]
    fn validate_args_invalid_local_algorithm() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.refine = true;
        args.local_algo = "invalid-local-algo".to_string();
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown local algorithm"));
    }

    #[test]
    fn validate_args_min_q_greater_than_max_q() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_q = 5.0;
        args.max_q = 2.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid Q factor range"));
    }

    #[test]
    fn validate_args_min_freq_greater_than_max_freq() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_freq = 1000.0;
        args.max_freq = 500.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid frequency range"));
    }

    #[test]
    fn validate_args_min_db_greater_than_max_db() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_db = 5.0;
        args.max_db = 2.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid dB range"));
    }

    #[test]
    fn validate_args_min_freq_too_low() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.min_freq = 10.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be >= 20 Hz"));
    }

    #[test]
    fn validate_args_max_freq_too_high() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.max_freq = 25000.0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be <= 20,000 Hz"));
    }

    #[test]
    fn validate_args_zero_population() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.population = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Population size must be > 0"));
    }

    #[test]
    fn validate_args_zero_num_filters() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Number of filters must be > 0"));
    }

    #[test]
    fn validate_args_too_many_filters() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.num_filters = 100;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("very high"));
    }

    #[test]
    fn validate_args_invalid_smooth_n() {
        let mut args = Args::parse_from(["autoeq-test"]);
        args.smooth_n = 0;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be in range [1..24]"));

        args.smooth_n = 25;
        let result = validate_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must be in range [1..24]"));
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
