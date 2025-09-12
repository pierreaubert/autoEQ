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

use super::constraints::x2peq;
use super::loss::{flat_loss, mixed_loss, score_loss, LossType, ScoreLossData};
use super::optim_de::optimize_filters_autoeq;
use super::optim_mh::optimize_filters_mh;
use super::optim_nlopt::optimize_filters_nlopt;
use ndarray::Array1;
use nlopt::Algorithm;
use std::process;

/// Algorithm metadata structure
#[derive(Debug, Clone)]
pub struct AlgorithmInfo {
    /// Algorithm name with library prefix (e.g., "nlopt:isres", "mh:de", "autoeq:de")
    pub name: &'static str,
    /// Library providing this algorithm (e.g., "NLOPT", "Metaheuristics", "AutoEQ")
    pub library: &'static str,
    /// Classification as global or local optimizer
    pub algorithm_type: AlgorithmType,
    /// Whether the algorithm supports linear constraint handling
    pub supports_linear_constraints: bool,
    /// Whether the algorithm supports nonlinear constraint handling
    pub supports_nonlinear_constraints: bool,
}

/// Algorithm classification
#[derive(Debug, Clone, PartialEq)]
pub enum AlgorithmType {
    /// Global optimization algorithm - explores entire solution space, good for finding global optimum
    Global,
    /// Local optimization algorithm - refines solution from starting point, fast but may get trapped in local optimum
    Local,
}

/// Get all available algorithms with their metadata
pub fn get_all_algorithms() -> Vec<AlgorithmInfo> {
    vec![
        // NLOPT algorithms - Global with nonlinear constraint support
        AlgorithmInfo {
            name: "nlopt:isres",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        AlgorithmInfo {
            name: "nlopt:ags",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: true,
        },
        AlgorithmInfo {
            name: "nlopt:origdirect",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: true,
        },
        // NLOPT algorithms - Global without nonlinear constraint support
        AlgorithmInfo {
            name: "nlopt:crs2lm",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:direct",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:directl",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:gmlsl",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:gmlsllds",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:sbplx",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:slsqp",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        AlgorithmInfo {
            name: "nlopt:stogo",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:stogorand",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        // NLOPT algorithms - Local
        AlgorithmInfo {
            name: "nlopt:bobyqa",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "nlopt:cobyla",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
        AlgorithmInfo {
            name: "nlopt:neldermead",
            library: "NLOPT",
            algorithm_type: AlgorithmType::Local,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        // Metaheuristics algorithms (all global, no constraint support)
        AlgorithmInfo {
            name: "mh:de",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:pso",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:rga",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:tlbo",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "mh:firefly",
            library: "Metaheuristics",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: false,
            supports_nonlinear_constraints: false,
        },
        AlgorithmInfo {
            name: "autoeq:de",
            library: "AutoEQ",
            algorithm_type: AlgorithmType::Global,
            supports_linear_constraints: true,
            supports_nonlinear_constraints: true,
        },
    ]
}

/// Find algorithm info by name (supports both prefixed and unprefixed names for backward compatibility)
pub fn find_algorithm_info(name: &str) -> Option<AlgorithmInfo> {
    let algorithms = get_all_algorithms();

    // First try exact match
    if let Some(algo) = algorithms
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(name))
    {
        return Some(algo.clone());
    }

    // Then try without prefix for backward compatibility
    let name_lower = name.to_lowercase();
    for algo in &algorithms {
        if let Some(suffix) = algo.name.split(':').nth(1) {
            if suffix.eq_ignore_ascii_case(&name_lower) {
                return Some(algo.clone());
            }
        }
    }

    None
}

/// Data structure for holding objective function parameters
///
/// This struct contains all the data needed to compute the objective function
/// for filter optimization.
#[derive(Debug, Clone)]
pub struct ObjectiveData {
    /// Frequency points for evaluation
    pub freqs: Array1<f64>,
    /// Target error values
    pub target_error: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    #[allow(dead_code)]
    /// Minimum spacing between filters in octaves
    pub min_spacing_oct: f64,
    /// Weight for spacing penalty term
    pub spacing_weight: f64,
    /// Maximum allowed dB level
    pub max_db: f64,
    /// Minimum absolute gain for filters
    pub min_db: f64,
    /// Whether to use highpass/peak filter configuration
    pub iir_hp_pk: bool,
    /// Type of loss function to use
    pub loss_type: LossType,
    /// Optional score data for Score loss type
    pub score_data: Option<ScoreLossData>,
    /// Penalty weights used when the optimizer does not support nonlinear constraints
    /// If zero, penalties are disabled and true constraints (if any) are used.
    /// Penalty for ceiling constraint
    pub penalty_w_ceiling: f64,
    /// Penalty for spacing constraint
    pub penalty_w_spacing: f64,
    /// Penalty for min gain constraint
    pub penalty_w_mingain: f64,
    /// Integrality constraints - true for integer parameters, false for continuous
    pub integrality: Option<Vec<bool>>,
}

/// Determine algorithm type and return normalized name
#[derive(Debug, Clone)]
pub enum AlgorithmCategory {
    /// NLOPT library algorithm with specific algorithm type
    Nlopt(Algorithm),
    /// Metaheuristics library algorithm with algorithm name
    Metaheuristics(String),
    /// AutoEQ custom algorithm with algorithm name
    AutoEQ(String),
}

/// Parse algorithm name and return category with normalized name
pub fn parse_algorithm_name(name: &str) -> Option<AlgorithmCategory> {
    if let Some(algo_info) = find_algorithm_info(name) {
        let normalized_name = algo_info.name;

        if normalized_name.starts_with("nlopt:") {
            let nlopt_name = normalized_name.strip_prefix("nlopt:").unwrap();
            let nlopt_algo = match nlopt_name {
                "bobyqa" => Algorithm::Bobyqa,
                "cobyla" => Algorithm::Cobyla,
                "neldermead" => Algorithm::Neldermead,
                "isres" => Algorithm::Isres,
                "ags" => Algorithm::Ags,
                "origdirect" => Algorithm::OrigDirect,
                "crs2lm" => Algorithm::Crs2Lm,
                "direct" => Algorithm::Direct,
                "directl" => Algorithm::DirectL,
                "gmlsl" => Algorithm::GMlsl,
                "gmlsllds" => Algorithm::GMlslLds,
                "sbplx" => Algorithm::Sbplx,
                "slsqp" => Algorithm::Slsqp,
                "stogo" => Algorithm::StoGo,
                "stogorand" => Algorithm::StoGoRand,
                _ => Algorithm::Isres, // fallback
            };
            return Some(AlgorithmCategory::Nlopt(nlopt_algo));
        } else if normalized_name.starts_with("mh:") {
            let mh_name = normalized_name.strip_prefix("mh:").unwrap();
            return Some(AlgorithmCategory::Metaheuristics(mh_name.to_string()));
        } else if normalized_name.starts_with("autoeq:") {
            let autoeq_name = normalized_name.strip_prefix("autoeq:").unwrap();
            return Some(AlgorithmCategory::AutoEQ(autoeq_name.to_string()));
        }
    }

    None
}

/// Compute the base fitness value (without penalties) for given parameters
///
/// This is the unified fitness function used by both NLOPT and metaheuristics optimizers.
pub fn compute_base_fitness(x: &[f64], data: &ObjectiveData) -> f64 {
    let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);

    match data.loss_type {
        LossType::Flat => {
            let error = &peq_spl - &data.target_error;
            flat_loss(&data.freqs, &error)
        }
        LossType::Mixed => {
            if let Some(ref sd) = data.score_data {
                mixed_loss(sd, &data.freqs, &peq_spl)
            } else {
                eprintln!("Error: mixed loss requested but score data is missing");
                process::exit(1);
            }
        }
        LossType::Score => {
            if let Some(ref sd) = data.score_data {
                let error = &peq_spl - &data.target_error;
                let s = score_loss(sd, &data.freqs, &peq_spl);
                let p = flat_loss(&data.freqs, &error) / 3.0;
                s + p
            } else {
                eprintln!("Error: score loss requested but score data is missing");
                process::exit(1);
            }
        }
    }
}







/// Optimize filter parameters using global optimization algorithms
///
/// # Arguments
/// * `x` - Initial parameter vector to optimize (modified in place)
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Data structure containing optimization parameters
/// * `algo` - Optimization algorithm name (e.g., "isres", "cobyla")
/// * `population` - Population size for population-based algorithms
/// * `maxeval` - Maximum number of function evaluations
/// * `cli_args` - CLI arguments containing DE parameters (tolerance, strategy, etc.)
///
/// # Returns
/// * Result containing (status, optimal value) or (error, value)
///
/// # Details
/// Dispatches to appropriate library-specific optimizer based on algorithm name.
/// The parameter vector is organized as [freq, Q, gain] triplets for each filter.
pub fn optimize_filters(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    algo: &str,
    population: usize,
    maxeval: usize,
    cli_args: &crate::cli::Args,
) -> Result<(String, f64), (String, f64)> {
    // Parse algorithm and dispatch to appropriate function
    match parse_algorithm_name(algo) {
        Some(AlgorithmCategory::Nlopt(nlopt_algo)) => optimize_filters_nlopt(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            nlopt_algo,
            population,
            maxeval,
        ),
        Some(AlgorithmCategory::Metaheuristics(mh_name)) => optimize_filters_mh(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            &mh_name,
            population,
            maxeval,
        ),
        Some(AlgorithmCategory::AutoEQ(autoeq_name)) => optimize_filters_autoeq(
            x,
            lower_bounds,
            upper_bounds,
            objective_data,
            &autoeq_name,
            population,
            maxeval,
            cli_args,
        ),
        None => Err((format!("Unknown algorithm: {}", algo), f64::INFINITY)),
    }
}

/// Extract sorted center frequencies from parameter vector and compute adjacent spacings in octaves.
pub fn compute_sorted_freqs_and_adjacent_octave_spacings(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() / 3;
    let mut freqs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        freqs.push(10f64.powf(x[i * 3]));
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
        let x = [
            100f64.log10(),
            1.0,
            0.0,
            200f64.log10(),
            1.0,
            0.0,
            400f64.log10(),
            1.0,
            0.0,
        ];
        let (_freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        assert!((spacings[0] - 1.0).abs() < 1e-12);
        assert!((spacings[1] - 1.0).abs() < 1e-12);
    }
}
