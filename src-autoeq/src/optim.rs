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

use super::constraints::{
    constraint_ceiling, constraint_min_gain, viol_ceiling_from_spl, viol_min_gain_from_xs,
    viol_spacing_from_xs, x2peq, CeilingConstraintData, MinGainConstraintData,
};
use ndarray::Array1;
use nlopt::{Algorithm, Nlopt, Target};
use std::process;

// Optional alternative optimizer: metaheuristics-nature
// Implemented with dynamic dimension objective and penalty handling.
#[allow(unused_imports)]
use metaheuristics_nature as mh;
#[allow(unused_imports)]
use mh::methods::{De as MhDe, Fa as MhFa, Pso as MhPso, Rga as MhRga, Tlbo as MhTlbo};
#[allow(unused_imports)]
use mh::{Bounded as MhBounded, Fitness as MhFitness, ObjFunc as MhObjFunc, Solver as MhSolver};

use super::loss::{flat_loss, mixed_loss, score_loss, LossType, ScoreLossData};
use crate::de::{
    differential_evolution, CallbackAction, DEConfigBuilder, DEIntermediate, DEReport,
    Init, Mutation, ParallelConfig, Strategy,
};
// use ndarray::Array2; // unused
use super::init_sobol::init_sobol;
use super::read::smooth_gaussian;
use super::signal::find_peaks;

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

/// Frequency problem descriptor for smart initialization
#[derive(Debug, Clone)]
struct FrequencyProblem {
    /// Frequency in Hz
    frequency: f64,
    /// Magnitude of the problem (positive = boost needed, negative = cut needed)
    magnitude: f64,
    /// Suggested Q factor for this frequency
    q_factor: f64,
}

/// Smart initialization configuration
#[derive(Debug, Clone)]
pub struct SmartInitConfig {
    /// Number of different initial guesses to generate
    pub num_guesses: usize,
    /// Sigma for Gaussian smoothing of frequency response
    pub smoothing_sigma: f64,
    /// Minimum peak/dip height to consider
    pub min_peak_height: f64,
    /// Minimum distance between peaks/dips (in frequency points)
    pub min_peak_distance: usize,
    /// Critical frequencies to always consider (Hz)
    pub critical_frequencies: Vec<f64>,
    /// Random variation factor for guess diversification
    pub variation_factor: f64,
}

impl Default for SmartInitConfig {
    fn default() -> Self {
        Self {
            num_guesses: 5,
            smoothing_sigma: 2.0,
            min_peak_height: 1.0,
            min_peak_distance: 10,
            critical_frequencies: vec![100.0, 300.0, 1000.0, 3000.0, 8000.0, 16000.0],
            variation_factor: 0.1,
        }
    }
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

/// Compute objective function value including penalty terms for constraints
///
/// This function adds penalty terms to the base fitness when using algorithms
/// that don't support native constraint handling.
///
/// # Arguments
/// * `x` - Parameter vector
/// * `_gradient` - Gradient vector (unused, for NLOPT compatibility)
/// * `data` - Objective data containing penalty weights and parameters
///
/// # Returns
/// Base fitness value plus weighted penalty terms
fn compute_fitness_penalties(
    x: &[f64],
    _gradient: Option<&mut [f64]>,
    data: &mut ObjectiveData,
) -> f64 {
    let fit = compute_base_fitness(x, data);

    // When penalties are enabled (weights > 0), add them to the base fit so that
    // optimizers without nonlinear constraints can still respect our limits.
    let mut penalized = fit;
    let mut penalty_terms = Vec::new();

    if data.penalty_w_ceiling > 0.0 {
        let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);
        let viol = viol_ceiling_from_spl(&peq_spl, data.max_db, data.iir_hp_pk);
        let penalty = data.penalty_w_ceiling * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "ceiling_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_ceiling, penalty
            ));
        }
    }

    if data.penalty_w_spacing > 0.0 {
        let viol = viol_spacing_from_xs(x, data.min_spacing_oct);
        let penalty = data.penalty_w_spacing * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "spacing_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_spacing, penalty
            ));
        }
    }

    if data.penalty_w_mingain > 0.0 && data.min_db > 0.0 {
        let viol = viol_min_gain_from_xs(x, data.iir_hp_pk, data.min_db);
        let penalty = data.penalty_w_mingain * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "mingain_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_mingain, penalty
            ));
        }
    }

    // // Log fitness details every 1000 evaluations (approximate)
    // use std::sync::atomic::{AtomicUsize, Ordering};
    // static EVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
    // let count = EVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    // if count % 1000 == 0 || (count % 100 == 0 && !penalty_terms.is_empty()) {
    //     let param_summary: Vec<String> = (0..x.len()/3).map(|i| {
    //         let freq = 10f64.powf(x[i*3]);
    //         let q = x[i*3+1];
    //         let gain = x[i*3+2];
    //         format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
    //     }).collect();

    //     eprintln!("TRACE[{}]: fit={:.3e}, penalties=[{}], params=[{}]",
    //               count, fit, penalty_terms.join(", "), param_summary.join(", "));
    // }

    penalized
}

// ---------------- Metaheuristics objective and utilities ----------------
#[derive(Clone)]
struct MHObjective {
    data: ObjectiveData,
    bounds: Vec<[f64; 2]>,
}

impl MhBounded for MHObjective {
    fn bound(&self) -> &[[f64; 2]] {
        self.bounds.as_slice()
    }
}

impl MhObjFunc for MHObjective {
    type Ys = f64;
    fn fitness(&self, xs: &[f64]) -> Self::Ys {
        // Create mutable copy of data for compute_fitness_penalties
        let mut data_copy = self.data.clone();
        compute_fitness_penalties(xs, None, &mut data_copy)
    }
}

/// Optimize filter parameters using NLOPT algorithms
fn optimize_filters_nlopt(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    algo: Algorithm,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Decide whether to use penalties (for algorithms lacking inequality constraints)
    let use_penalties = match algo {
        Algorithm::Crs2Lm
        | Algorithm::Direct
        | Algorithm::DirectL
        | Algorithm::GMlsl
        | Algorithm::GMlslLds
        | Algorithm::Sbplx
        | Algorithm::StoGo
        | Algorithm::StoGoRand
        | Algorithm::Neldermead => true,
        _ => false,
    };

    // Prepare constraint data BEFORE moving objective_data into NLopt
    let ceiling_data = CeilingConstraintData {
        freqs: objective_data.freqs.clone(),
        srate: objective_data.srate,
        max_db: objective_data.max_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };
    let min_gain_data = MinGainConstraintData {
        min_db: objective_data.min_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };

    // Configure penalty weights when needed
    let mut objective_data = objective_data;
    if use_penalties {
        objective_data.penalty_w_ceiling = 1e4;
        objective_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1e3;
        objective_data.penalty_w_mingain = 1e3;
    } else {
        objective_data.penalty_w_ceiling = 0.0;
        objective_data.penalty_w_spacing = 0.0;
        objective_data.penalty_w_mingain = 0.0;
    }

    // Now create optimizer and move objective_data
    let mut optimizer = Nlopt::new(
        algo,
        num_params,
        compute_fitness_penalties,
        Target::Minimize,
        objective_data,
    );

    let _ = optimizer.set_lower_bounds(lower_bounds).unwrap();
    let _ = optimizer.set_upper_bounds(upper_bounds).unwrap();

    // Register inequality constraints when not using penalties.
    if !use_penalties {
        let _ = optimizer.add_inequality_constraint(constraint_ceiling, ceiling_data, 1e-6);
        // let _ = optimizer.add_inequality_constraint(constraint_spacing, spacing_data, 1e-9);
        let _ = optimizer.add_inequality_constraint(constraint_min_gain, min_gain_data, 1e-6);
    }

    let _ = optimizer.set_population(population);
    let _ = optimizer.set_maxeval(maxeval as u32);
    let _ = optimizer.set_stopval(1e-4).unwrap();
    let _ = optimizer.set_ftol_rel(1e-6).unwrap();
    let _ = optimizer.set_xtol_rel(1e-4).unwrap();

    let result = optimizer.optimize(x);

    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}

/// Optimize filter parameters using metaheuristics algorithms
fn optimize_filters_mh(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    mh_name: &str,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Build bounds for metaheuristics (as pairs)
    assert_eq!(lower_bounds.len(), num_params);
    assert_eq!(upper_bounds.len(), num_params);
    let mut bounds: Vec<[f64; 2]> = Vec::with_capacity(num_params);
    for i in 0..num_params {
        bounds.push([lower_bounds[i], upper_bounds[i]]);
    }

    // Create objective with penalties (metaheuristics don't support constraints)
    let mut penalty_data = objective_data.clone();
    penalty_data.penalty_w_ceiling = 1e4;
    penalty_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1e3;
    penalty_data.penalty_w_mingain = 1e3;

    // Simple objective function wrapper for metaheuristics
    let mh_obj = MHObjective {
        data: penalty_data,
        bounds,
    };

    // Choose algorithm configuration
    // Use boxed builder to allow runtime selection with unified type
    let builder = match mh_name {
        "de" => MhSolver::build_boxed(MhDe::default(), mh_obj),
        "pso" => MhSolver::build_boxed(MhPso::default(), mh_obj),
        "rga" => MhSolver::build_boxed(MhRga::default(), mh_obj),
        "tlbo" => MhSolver::build_boxed(MhTlbo::default(), mh_obj),
        "fa" => MhSolver::build_boxed(MhFa::default(), mh_obj),
        _ => MhSolver::build_boxed(MhDe::default(), mh_obj),
    };

    // Estimate generations from maxeval and population
    let pop = population.max(1);
    let gens = ((maxeval.max(pop)) + pop - 1) / pop; // ceil(maxeval/pop)

    // Avoid accessing ctx.gen directly (reserved identifier in Rust 2024).
    // Instead, count down generations via the task FnMut closure.
    let mut left = gens as i64;
    let solver = builder
        .seed(0)
        .pop_num(pop)
        .task(move |_| {
            left -= 1;
            left <= 0
        })
        .solve();

    // Write back the best parameters
    let best_xs = solver.as_best_xs();
    if best_xs.len() == x.len() {
        x.copy_from_slice(best_xs);
    }
    let best_val = *solver.as_best_fit();
    Ok((format!("Metaheuristics({mh_name})"), best_val))
}

/// Common setup for DE-based optimization
///
/// Contains all the shared configuration parameters for both standard and adaptive DE algorithms.
struct DESetup {
    /// Parameter bounds as (lower, upper) tuples for optde
    bounds: Vec<(f64, f64)>,
    /// Objective data with penalty weights configured
    penalty_data: ObjectiveData,
    /// Population size (minimum 15)
    pop_size: usize,
    /// Maximum iterations derived from maxeval and population
    max_iter: usize,
}

/// Set up common DE parameters
///
/// Converts bounds format, configures penalty weights, and estimates population/iteration parameters.
///
/// # Arguments
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Base objective configuration
/// * `population` - Requested population size
/// * `maxeval` - Maximum function evaluations
///
/// # Returns
/// Configured DESetup with all common parameters
fn setup_de_common(
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    population: usize,
    maxeval: usize,
) -> DESetup {
    // Convert bounds format for optde
    let bounds: Vec<(f64, f64)> = lower_bounds
        .iter()
        .zip(upper_bounds.iter())
        .map(|(&lo, &hi)| (lo, hi))
        .collect();

    // Set up penalty-based objective data for DE
    let mut penalty_data = objective_data.clone();
    penalty_data.penalty_w_ceiling = 0.0;
    penalty_data.penalty_w_spacing = 0.0;
    penalty_data.penalty_w_mingain = 0.0;

    // Estimate parameters
    let pop_size = population.max(15); // minimum reasonable population
    let max_iter = maxeval.min(pop_size * 10);

    // Log setup configuration
    eprintln!(
        "DE Setup: {} filters, pop_size={}, max_iter={}, maxeval={}",
        bounds.len() / 3,
        pop_size,
        max_iter,
        maxeval
    );
    eprintln!(
        "  Penalty weights: ceiling={:.1e}, spacing={:.1e}, mingain={:.1e}",
        penalty_data.penalty_w_ceiling,
        penalty_data.penalty_w_spacing,
        penalty_data.penalty_w_mingain
    );
    eprintln!(
        "  Constraints: max_db={:.1}, min_spacing={:.3} oct, min_db={:.1}",
        penalty_data.max_db, penalty_data.min_spacing_oct, penalty_data.min_db
    );

    // // Log parameter bounds
    // for (i, &(lo, hi)) in bounds.iter().enumerate() {
    //     let param_type = match i % 3 {
    //         0 => "log10(freq)",
    //         1 => "Q",
    //         2 => "gain(dB)",
    //         _ => unreachable!(),
    //     };
    //     eprintln!("  Bound[{}] {}: [{:.3}, {:.3}]", i, param_type, lo, hi);
    // }

    DESetup {
        bounds,
        penalty_data,
        pop_size,
        max_iter,
    }
}

/// Create progress reporting callback - print every 100 iterations
///
/// Creates a callback function that prints optimization progress at regular intervals.
///
/// # Arguments
/// * `algo_name` - Algorithm name to display in progress messages
///
/// # Returns
/// Boxed callback function for DE optimization
fn create_de_callback(algo_name: &str) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
    let name = algo_name.to_string();
    let mut last_fitness = f64::INFINITY;
    let mut stall_count = 0;

    Box::new(move |intermediate: &DEIntermediate| -> CallbackAction {
        // Check for progress
        let improvement = if intermediate.fun < last_fitness {
            let delta = last_fitness - intermediate.fun;
            last_fitness = intermediate.fun;
            stall_count = 0;
            format!("(-{:.2e})", delta)
        } else {
            stall_count += 1;
            if stall_count >= 50 {
                format!("(STALL:{})", stall_count)
            } else {
                "(--)".to_string()
            }
        };

        // print when stalling
        if stall_count == 1 || stall_count % 25 == 0 {
            eprintln!(
                "{} iter {:4}  fitness={:.6e} {} conv={:.3e}",
                name, intermediate.iter, intermediate.fun, improvement, intermediate.convergence
            );
        }

        // Show parameter details every 100 iterations
        if intermediate.iter % 100 == 0 {
            let param_summary: Vec<String> = (0..intermediate.x.len() / 3)
                .map(|i| {
                    let freq = 10f64.powf(intermediate.x[i * 3]);
                    let q = intermediate.x[i * 3 + 1];
                    let gain = intermediate.x[i * 3 + 2];
                    format!("[f{:.0}Hz Q{:.2} G{:.2}dB]", freq, q, gain)
                })
                .collect();
            eprintln!("  --> Best params: {}", param_summary.join(" "));
        }

        CallbackAction::Continue
    })
}

/// Create objective function for DE optimization
///
/// Wraps the penalty-based fitness computation for use with the optde library.
///
/// # Arguments
/// * `penalty_data` - Objective data with penalty weights configured
///
/// # Returns
/// Closure that computes fitness from ndarray parameter vector
fn create_de_objective(penalty_data: ObjectiveData) -> impl Fn(&ndarray::Array1<f64>) -> f64 {
    move |x_arr: &ndarray::Array1<f64>| -> f64 {
        let x_slice = x_arr.as_slice().unwrap();
        let mut data_copy = penalty_data.clone();
        compute_fitness_penalties(x_slice, None, &mut data_copy)
    }
}

/// Process DE optimization results
///
/// Copies optimized parameters back to input array and formats status message.
///
/// # Arguments
/// * `x` - Mutable parameter array to update with optimized values
/// * `result` - DE optimization result containing optimal parameters and status
/// * `algo_name` - Algorithm name for status message formatting
///
/// # Returns
/// Result tuple with (status_message, objective_value)
fn process_de_results(
    x: &mut [f64],
    result: DEReport,
    algo_name: &str,
) -> Result<(String, f64), (String, f64)> {
    // Copy results back to input array
    if result.x.len() == x.len() {
        for i in 0..x.len() {
            x[i] = result.x[i];
        }
    }

    let status = if result.success {
        format!("AutoEQ {}: {}", algo_name, result.message)
    } else {
        format!("AutoEQ {}: {} (not converged)", algo_name, result.message)
    };

    Ok((status, result.fun))
}

/// Optimize filter parameters using AutoEQ custom algorithms
fn optimize_filters_autoeq(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    _autoeq_name: &str,
    population: usize,
    maxeval: usize,
    cli_args: &crate::cli::Args,
) -> Result<(String, f64), (String, f64)> {
    // Adaptive DE with advanced features and native constraints
    let setup = setup_de_common(
        lower_bounds,
        upper_bounds,
        objective_data.clone(),
        population,
        maxeval,
    );
    let base_objective_fn = create_de_objective(setup.penalty_data.clone());
    let callback = create_de_callback("autoeq::DE");

    // Create smart initialization based on frequency response analysis
    let num_filters = x.len() / 3;
    let smart_config = SmartInitConfig::default();

    // Use the inverted target as the response to analyze for problems
    let target_response = &setup.penalty_data.target_error;
    let freq_grid = &setup.penalty_data.freqs;

    eprintln!("🧠 Generating smart initial guesses based on frequency response analysis...");
    let smart_guesses = create_smart_initial_guesses(
        target_response,
        freq_grid,
        num_filters,
        &setup.bounds,
        &smart_config,
    );

    eprintln!("📊 Generated {} smart initial guesses", smart_guesses.len());

    // Generate Sobol quasi-random population for better space coverage
    let sobol_samples = init_sobol(
        x.len(),
        setup.pop_size.saturating_sub(smart_guesses.len()),
        &setup.bounds,
    );

    eprintln!(
        "🎯 Generated {} Sobol quasi-random samples",
        sobol_samples.len()
    );

    // Use the best smart guess as initial x0, fall back to Sobol initialization
    let best_initial_guess = if !smart_guesses.is_empty() {
        // Use the first (best) smart guess
        Array1::from(smart_guesses[0].clone())
    } else if !sobol_samples.is_empty() {
        // Fallback to the first Sobol sample if no smart guesses
        Array1::from(sobol_samples[0].clone())
    } else {
        // Ultimate fallback: use current x as initial guess
        Array1::from(x.to_vec())
    };

    eprintln!("🚀 Using smart initial guess with Sobol population initialization");

    // Parse strategy from CLI args
    use std::str::FromStr;
    let strategy = Strategy::from_str(&cli_args.strategy).unwrap_or_else(|_| {
        eprintln!(
            "⚠️ Warning: Invalid strategy '{}', falling back to CurrentToBest1Bin",
            cli_args.strategy
        );
        Strategy::CurrentToBest1Bin
    });

    // Set up adaptive configuration if using adaptive strategies
    let adaptive_config = if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
        Some(crate::de::AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: true, // Enable WLS for adaptive strategies
            w_f: cli_args.adaptive_weight_f,
            w_cr: cli_args.adaptive_weight_cr,
            ..crate::de::AdaptiveConfig::default()
        })
    } else {
        None
    };

    // Use constraint helpers for nonlinear constraints
    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_size)
        .tol(cli_args.tolerance)
        .atol(cli_args.atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(cli_args.recombination)
        .init(Init::LatinHypercube) // Use Latin Hypercube sampling for population
        .x0(best_initial_guess) // Use smart guess as initial best individual
        .disp(false)
        .callback(callback);

    // Add adaptive configuration if present
    if let Some(adaptive_cfg) = adaptive_config {
        config_builder = config_builder.adaptive(adaptive_cfg);
    }

    // Configure parallel evaluation
    let parallel_config = ParallelConfig {
        enabled: !cli_args.no_parallel,
        num_threads: if cli_args.parallel_threads == 0 {
            None // Use all available cores
        } else {
            Some(cli_args.parallel_threads)
        },
    };
    config_builder = config_builder.parallel(parallel_config);

    if !cli_args.no_parallel {
        eprintln!(
            "🚄 Parallel evaluation enabled with {} threads",
            cli_args.parallel_threads.eq(&0).then(|| "all available".to_string())
                .unwrap_or_else(|| cli_args.parallel_threads.to_string())
        );
    }

    // Add native constraint penalties for ceiling and spacing constraints
    let ceiling_penalty = {
        let penalty_data = setup.penalty_data.clone();
        move |x: &ndarray::Array1<f64>| -> f64 {
            let x_slice = x.as_slice().unwrap();
            let peq_spl = x2peq(
                &penalty_data.freqs,
                x_slice,
                penalty_data.srate,
                penalty_data.iir_hp_pk,
            );
            let viol = viol_ceiling_from_spl(&peq_spl, penalty_data.max_db, penalty_data.iir_hp_pk);
            viol
        }
    };
    config_builder = config_builder.add_penalty_ineq(
        Box::new(ceiling_penalty),
        setup.penalty_data.penalty_w_ceiling,
    );

    // Add spacing constraint as penalty
    // let spacing_penalty = {
    //     let penalty_data = setup.penalty_data.clone();
    //     move |x: &ndarray::Array1<f64>| -> f64 {
    //         let x_slice = x.as_slice().unwrap();
    //         viol_spacing_from_xs(x_slice, penalty_data.min_spacing_oct)
    //     }
    // };
    // config_builder = config_builder.add_penalty_ineq(
    //     Box::new(spacing_penalty),
    //     setup.penalty_data.penalty_w_spacing
    // );

    // Add min gain constraint as penalty
    // let mingain_penalty = {
    //     let penalty_data = setup.penalty_data.clone();
    //     move |x: &ndarray::Array1<f64>| -> f64 {
    //         let x_slice = x.as_slice().unwrap();
    //         viol_min_gain_from_xs(x_slice, penalty_data.iir_hp_pk, penalty_data.min_db)
    //     }
    // };
    // config_builder = config_builder.add_penalty_ineq(
    //     Box::new(mingain_penalty),
    //     setup.penalty_data.penalty_w_mingain
    // );

    let config = config_builder.build();
    let result = differential_evolution(&base_objective_fn, &setup.bounds, config);
    process_de_results(x, result, "AutoDE")
}

/// AutoEQ DE optimization with external progress callback
pub fn optimize_filters_autoeq_with_callback(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    _autoeq_name: &str,
    population: usize,
    maxeval: usize,
    cli_args: &crate::cli::Args,
    mut callback: Box<dyn FnMut(&crate::de::DEIntermediate) -> crate::de::CallbackAction + Send>,
) -> Result<(String, f64), (String, f64)> {
    // Reuse same setup as standard AutoEQ DE
    let setup = setup_de_common(
        lower_bounds,
        upper_bounds,
        objective_data.clone(),
        population,
        maxeval,
    );
    let base_objective_fn = create_de_objective(setup.penalty_data.clone());

    let num_filters = x.len() / 3;
    let smart_config = SmartInitConfig::default();

    let target_response = &setup.penalty_data.target_error;
    let freq_grid = &setup.penalty_data.freqs;

    let smart_guesses = create_smart_initial_guesses(
        target_response,
        freq_grid,
        num_filters,
        &setup.bounds,
        &smart_config,
    );

    let sobol_samples = init_sobol(
        x.len(),
        setup.pop_size.saturating_sub(smart_guesses.len()),
        &setup.bounds,
    );

    let best_initial_guess = if !smart_guesses.is_empty() {
        Array1::from(smart_guesses[0].clone())
    } else if !sobol_samples.is_empty() {
        Array1::from(sobol_samples[0].clone())
    } else {
        Array1::from(x.to_vec())
    };

    use std::str::FromStr;
    let strategy = Strategy::from_str(&cli_args.strategy).unwrap_or(Strategy::CurrentToBest1Bin);

    let adaptive_config = if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
        Some(crate::de::AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: true,
            w_f: cli_args.adaptive_weight_f,
            w_cr: cli_args.adaptive_weight_cr,
            ..crate::de::AdaptiveConfig::default()
        })
    } else {
        None
    };

    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_size)
        .tol(cli_args.tolerance)
        .atol(cli_args.atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(cli_args.recombination)
        .init(Init::LatinHypercube)
        .x0(best_initial_guess)
        .disp(false)
        .callback(Box::new(move |intermediate| callback(intermediate)));

    if let Some(adaptive_cfg) = adaptive_config {
        config_builder = config_builder.adaptive(adaptive_cfg);
    }

    // Configure parallel evaluation
    let parallel_config = ParallelConfig {
        enabled: !cli_args.no_parallel,
        num_threads: if cli_args.parallel_threads == 0 {
            None // Use all available cores
        } else {
            Some(cli_args.parallel_threads)
        },
    };
    config_builder = config_builder.parallel(parallel_config);

    let config = config_builder.build();
    let result = differential_evolution(&base_objective_fn, &setup.bounds, config);
    process_de_results(x, result, "AutoDE")
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

/// Generate integrality constraints for filter optimization
///
/// In the AutoEQ parameter encoding:
/// - Parameter 1 (frequency index): integer (when using frequency indexing)
/// - Parameter 2 (Q factor): continuous
/// - Parameter 3 (gain): continuous
///
/// # Arguments
/// * `num_filters` - Number of filters
/// * `use_freq_indexing` - Whether frequency is encoded as integer index (true) or continuous log10(Hz) (false)
///
/// # Returns
/// Vector of boolean values: true for integer parameters, false for continuous
pub fn generate_integrality_constraints(num_filters: usize, use_freq_indexing: bool) -> Vec<bool> {
    let mut constraints = Vec::with_capacity(num_filters * 4);

    for _i in 0..num_filters {
        // constraints.push(true);  // Filter type - integer, not yet implemented
        constraints.push(use_freq_indexing); // Frequency - integer if indexing, continuous if log10(Hz)
        constraints.push(false); // Q factor - continuous
        constraints.push(false); // Gain - continuous
    }

    constraints
}

/// Create smart initial guesses based on frequency response analysis
///
/// Analyzes the target frequency response to identify peaks and dips,
/// then generates initial parameter guesses that address these problems.
///
/// # Arguments
/// * `target_response` - Target frequency response to analyze
/// * `freq_grid` - Frequency grid corresponding to the response
/// * `num_filters` - Number of filters to optimize
/// * `bounds` - Parameter bounds for validation
/// * `config` - Smart initialization configuration
///
/// # Returns
/// Vector of initial guess parameter vectors
pub fn create_smart_initial_guesses(
    target_response: &Array1<f64>,
    freq_grid: &Array1<f64>,
    num_filters: usize,
    bounds: &[(f64, f64)],
    config: &SmartInitConfig,
) -> Vec<Vec<f64>> {
    // Smooth the response to reduce noise
    let smoothed = smooth_gaussian(target_response, config.smoothing_sigma);

    // Find peaks (need cuts) and dips (need boosts)
    let peaks = find_peaks(&smoothed, config.min_peak_height, config.min_peak_distance);
    let inverted = -&smoothed;
    let dips = find_peaks(&inverted, config.min_peak_height, config.min_peak_distance);

    let mut problems = Vec::new();

    // Add peaks (need cuts)
    for &peak_idx in &peaks {
        if peak_idx < freq_grid.len() {
            problems.push(FrequencyProblem {
                frequency: freq_grid[peak_idx],
                magnitude: -smoothed[peak_idx].abs(), // Negative for cuts
                q_factor: 1.0,
            });
        }
    }

    // Add dips (need boosts)
    for &dip_idx in &dips {
        if dip_idx < freq_grid.len() {
            problems.push(FrequencyProblem {
                frequency: freq_grid[dip_idx],
                magnitude: smoothed[dip_idx].abs(), // Positive for boosts
                q_factor: 0.7,                      // Lower Q for boosts
            });
        }
    }

    // Sort by magnitude (most problematic first)
    problems.sort_by(|a, b| {
        b.magnitude
            .abs()
            .partial_cmp(&a.magnitude.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Generate initial guesses
    let mut initial_guesses = Vec::new();

    for _guess_idx in 0..config.num_guesses {
        let mut guess = Vec::with_capacity(num_filters * 3); // [log10(freq), Q, gain] per filter
        let mut used_problems = problems.clone();

        // Fill with critical frequencies if not enough problems found
        while used_problems.len() < num_filters {
            for &critical_freq in &config.critical_frequencies {
                if critical_freq >= freq_grid[0] && critical_freq <= freq_grid[freq_grid.len() - 1]
                {
                    used_problems.push(FrequencyProblem {
                        frequency: critical_freq,
                        magnitude: 0.5,
                        q_factor: 1.0,
                    });
                }
                if used_problems.len() >= num_filters {
                    break;
                }
            }

            // Fill remaining with random frequencies if needed
            while used_problems.len() < num_filters {
                use rand::Rng;
                let mut rng = rand::rng();
                let rand_freq = rng.random_range(freq_grid[0]..freq_grid[freq_grid.len() - 1]);
                used_problems.push(FrequencyProblem {
                    frequency: rand_freq,
                    magnitude: rng.random_range(-2.0..2.0),
                    q_factor: 1.0,
                });
            }
        }

        // Create parameter vector for this guess
        for i in 0..num_filters {
            let problem = &used_problems[i % used_problems.len()];

            // Add some randomization to diversify guesses
            use rand::Rng;
            let mut rng = rand::rng();

            let freq_var = problem.frequency
                * (1.0 + rng.random_range(-config.variation_factor..config.variation_factor));
            let gain_var = problem.magnitude * (1.0 + rng.random_range(-0.2..0.2));
            let q_var = problem.q_factor * (1.0 + rng.random_range(-0.3..0.3));

            // Convert to log10(freq) and constrain to bounds
            let log_freq = freq_var.log10().max(bounds[i * 3].0).min(bounds[i * 3].1);
            let q_constrained = q_var.max(bounds[i * 3 + 1].0).min(bounds[i * 3 + 1].1);
            let gain_constrained = gain_var.max(bounds[i * 3 + 2].0).min(bounds[i * 3 + 2].1);

            guess.extend_from_slice(&[log_freq, q_constrained, gain_constrained]);
        }

        initial_guesses.push(guess);
    }

    initial_guesses
}

#[cfg(test)]
mod smart_init_tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_generate_integrality_constraints() {
        let constraints = generate_integrality_constraints(2, true);
        // 2 filters × 3 params each = 6 total params
        // Pattern: [true, false, false] repeated (freq indexed, Q continuous, gain continuous)
        assert_eq!(constraints.len(), 6);
        assert_eq!(constraints[0], true); // Frequency (indexed)
        assert_eq!(constraints[1], false); // Q factor (continuous)
        assert_eq!(constraints[2], false); // Gain (continuous)

        // Second filter
        assert_eq!(constraints[3], true); // Frequency (indexed)
        assert_eq!(constraints[4], false); // Q factor (continuous)
        assert_eq!(constraints[5], false); // Gain (continuous)

        // Test continuous frequency case
        let constraints_continuous = generate_integrality_constraints(2, false);
        assert_eq!(constraints_continuous.len(), 6);
        assert_eq!(constraints_continuous[0], false); // Frequency (continuous)
        assert_eq!(constraints_continuous[1], false); // Q factor (continuous)
        assert_eq!(constraints_continuous[2], false); // Gain (continuous)
        assert_eq!(constraints_continuous[3], false); // Frequency (continuous)
        assert_eq!(constraints_continuous[4], false); // Q factor (continuous)
        assert_eq!(constraints_continuous[5], false); // Gain (continuous)
    }

    #[test]
    fn test_create_smart_initial_guesses() {
        // Create a simple test case with a peak and dip
        let target_response = Array1::from(vec![0.0, 3.0, 0.0, -2.0, 0.0]);
        let freq_grid = Array1::from(vec![100.0, 200.0, 400.0, 800.0, 1600.0]);
        let bounds = vec![
            (100.0_f64.log10(), 1600.0_f64.log10()), // log10(freq)
            (0.5, 3.0),                              // Q
            (-6.0, 6.0),                             // Gain
        ];
        let config = SmartInitConfig::default();

        let guesses =
            create_smart_initial_guesses(&target_response, &freq_grid, 1, &bounds, &config);

        assert_eq!(guesses.len(), config.num_guesses);
        for guess in &guesses {
            assert_eq!(guess.len(), 3); // 1 filter × 3 params
                                        // Check bounds
            assert!(guess[0] >= bounds[0].0 && guess[0] <= bounds[0].1);
            assert!(guess[1] >= bounds[1].0 && guess[1] <= bounds[1].1);
            assert!(guess[2] >= bounds[2].0 && guess[2] <= bounds[2].1);
        }
    }
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
