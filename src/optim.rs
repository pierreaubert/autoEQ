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

use crate::constraints::{
    CeilingConstraintData, MinGainConstraintData, constraint_ceiling, constraint_min_gain,
    viol_ceiling_from_spl, viol_min_gain_from_xs, viol_spacing_from_xs, x2peq,
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

use crate::loss::{LossType, ScoreLossData, flat_loss, mixed_loss, score_loss};
use crate::optde::{
    CallbackAction, Crossover, DEConfigBuilder, DEIntermediate, DEReport, Init, Mutation, Strategy,
    differential_evolution,
};
use ndarray::Array2;

/// Parameters for AutoDE (AutoEQ Differential Evolution) algorithm
#[derive(Debug, Clone)]
pub struct AutoDEParams {
    /// Maximum number of iterations/generations
    pub max_iterations: usize,
    /// Population size (None = auto-sized based on problem dimension)
    pub population_size: Option<usize>,
    /// Mutation factor F ∈ [0, 2] (typical: 0.5-0.8)
    pub f: f64,
    /// Crossover probability CR ∈ [0, 1] (typical: 0.7-0.9)
    pub cr: f64,
    /// Convergence tolerance for objective function
    pub tolerance: f64,
    /// Random seed for reproducibility (None = random)
    pub seed: Option<u64>,
}

impl Default for AutoDEParams {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            population_size: None, // Auto-sized
            f: 0.8,
            cr: 0.9,
            tolerance: 1e-6,
            seed: None,
        }
    }
}

/// Simplified AutoDE interface for general optimization problems
///
/// This function provides an easy-to-use interface to our differential evolution
/// implementation for general optimization problems outside of filter optimization.
///
/// # Arguments
/// * `objective` - Objective function to minimize: f(x) -> f64
/// * `bounds` - Bounds matrix (2 × n): bounds[[0, i]] = lower bound, bounds[[1, i]] = upper bound
/// * `params` - Optional parameters (uses default if None)
///
/// # Returns
/// * Some((x_opt, f_opt, iterations)) on success
/// * None on failure (invalid parameters or bounds)
///
/// # Example
/// ```ignore
/// use ndarray::Array2;
/// use autoeq::optim::{auto_de, AutoDEParams};
///
/// // Minimize f(x) = x[0]^2 + x[1]^2 subject to -5 <= x[i] <= 5
/// let quadratic = |x: &ndarray::Array1<f64>| x.iter().map(|&xi| xi * xi).sum();
/// let bounds = Array2::from_shape_vec((2, 2), vec![-5.0, -5.0, 5.0, 5.0]).unwrap();
///
/// if let Some((x_opt, f_opt, iterations)) = auto_de(quadratic, &bounds, None) {
///     println!("Found optimum: x = {:?}, f = {:.6}, iterations = {}", x_opt, f_opt, iterations);
/// }
/// ```
pub fn auto_de<F>(
    objective: F,
    bounds: &Array2<f64>,
    params: Option<AutoDEParams>,
) -> Option<(Array1<f64>, f64, usize)>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    let params = params.unwrap_or_default();

    // Validate parameters
    if params.f < 0.0 || params.f > 2.0 {
        return None; // Invalid mutation factor
    }
    if params.cr < 0.0 || params.cr > 1.0 {
        return None; // Invalid crossover probability
    }

    // Validate bounds
    if bounds.shape().len() != 2 || bounds.shape()[0] != 2 {
        return None; // Invalid bounds shape
    }

    let n_vars = bounds.shape()[1];
    if n_vars == 0 {
        return None; // Empty bounds
    }

    // Check bounds validity and convert to tuples
    let mut bounds_tuples = Vec::with_capacity(n_vars);
    for i in 0..n_vars {
        let lower = bounds[[0, i]];
        let upper = bounds[[1, i]];
        if lower > upper {
            return None; // Lower bound > upper bound
        }
        bounds_tuples.push((lower, upper));
    }

    // Set up population size
    let pop_size = params.population_size.unwrap_or_else(|| {
        // Default: 15 * dimension, with reasonable min/max
        (15 * n_vars).max(30).min(300)
    });

    // Create DE configuration using builder pattern
    let mut config_builder = DEConfigBuilder::new()
        .strategy(Strategy::RandToBest1Bin)
        .mutation(Mutation::Factor(params.f))
        .recombination(params.cr)
        .crossover(Crossover::Binomial)
        .popsize(pop_size)
        .maxiter(params.max_iterations)
        .init(Init::Random)
        .tol(params.tolerance)
        .atol(params.tolerance * 0.1);

    // Set seed if provided
    if let Some(seed) = params.seed {
        config_builder = config_builder.seed(seed);
    }

    let config = config_builder.build();
    let report = differential_evolution(&objective, &bounds_tuples, config);

    Some((report.x, report.fun, report.nit))
}

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
            penalty_terms.push(format!("ceiling_viol={:.3e}*{:.1e}={:.3e}", viol, data.penalty_w_ceiling, penalty));
        }
    }

    if data.penalty_w_spacing > 0.0 {
        let viol = viol_spacing_from_xs(x, data.min_spacing_oct);
        let penalty = data.penalty_w_spacing * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!("spacing_viol={:.3e}*{:.1e}={:.3e}", viol, data.penalty_w_spacing, penalty));
        }
    }

    if data.penalty_w_mingain > 0.0 && data.min_db > 0.0 {
        let viol = viol_min_gain_from_xs(x, data.iir_hp_pk, data.min_db);
        let penalty = data.penalty_w_mingain * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!("mingain_viol={:.3e}*{:.1e}={:.3e}", viol, data.penalty_w_mingain, penalty));
        }
    }

    // Log fitness details every 1000 evaluations (approximate)
    use std::sync::atomic::{AtomicUsize, Ordering};
    static EVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let count = EVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count % 1000 == 0 || !penalty_terms.is_empty() {
        let param_summary: Vec<String> = (0..x.len()/3).map(|i| {
            let freq = 10f64.powf(x[i*3]);
            let q = x[i*3+1];
            let gain = x[i*3+2];
            format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
        }).collect();

        eprintln!("TRACE[{}]: fit={:.3e}, penalties=[{}], params=[{}]",
                  count, fit, penalty_terms.join(", "), param_summary.join(", "));
    }

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
    penalty_data.penalty_w_ceiling = 100.0;
    penalty_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1.0;
    penalty_data.penalty_w_mingain = 10.0;

    // Estimate parameters
    let pop_size = population.max(15); // minimum reasonable population
    let max_iter = maxeval.min(pop_size*10);

    // Log setup configuration
    eprintln!("DE Setup: {} filters, pop_size={}, max_iter={}, maxeval={}",
              bounds.len()/3, pop_size, max_iter, maxeval);
    eprintln!("  Penalty weights: ceiling={:.1e}, spacing={:.1e}, mingain={:.1e}",
              penalty_data.penalty_w_ceiling, penalty_data.penalty_w_spacing, penalty_data.penalty_w_mingain);
    eprintln!("  Constraints: max_db={:.1}, min_spacing={:.3} oct, min_db={:.1}",
              penalty_data.max_db, penalty_data.min_spacing_oct, penalty_data.min_db);

    // Log parameter bounds
    for (i, &(lo, hi)) in bounds.iter().enumerate() {
        let param_type = match i % 3 {
            0 => "log10(freq)",
            1 => "Q",
            2 => "gain(dB)",
            _ => unreachable!(),
        };
        eprintln!("  Bound[{}] {}: [{:.3}, {:.3}]", i, param_type, lo, hi);
    }

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

        // Print progress every 10 iterations, and always when there's improvement or stalling
        if intermediate.iter % 10 == 0 || stall_count == 1 || stall_count % 25 == 0 {
            eprintln!("{} iter {:4}  fitness={:.6e} {} conv={:.3e}",
                     name, intermediate.iter, intermediate.fun, improvement, intermediate.convergence);
        }

        // Show parameter details every 100 iterations
        if intermediate.iter % 100 == 0 {
            let param_summary: Vec<String> = (0..intermediate.x.len()/3).map(|i| {
                let freq = 10f64.powf(intermediate.x[i*3]);
                let q = intermediate.x[i*3+1];
                let gain = intermediate.x[i*3+2];
                format!("[f{:.0}Hz Q{:.2} G{:.2}dB]", freq, q, gain)
            }).collect();
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
    autoeq_name: &str,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {

    // Adaptive DE with advanced features and native constraints
    let setup = setup_de_common(lower_bounds, upper_bounds, objective_data.clone(), population, maxeval);
    let base_objective_fn = create_de_objective(setup.penalty_data.clone());
    let callback = create_de_callback("AutoEQ AutoDE");

    // Use constraint helpers for nonlinear constraints
    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_size)
        .tol(1e-3)
        .atol(1e-4)
        .strategy(Strategy::CurrentToBest1Bin)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(0.9)
        .init(Init::LatinHypercube)
        .disp(false)
        .callback(callback);

    // Add native constraint penalties for ceiling and spacing constraints
    let ceiling_penalty = {
        let penalty_data = setup.penalty_data.clone();
        move |x: &ndarray::Array1<f64>| -> f64 {
            let x_slice = x.as_slice().unwrap();
            let peq_spl = x2peq(&penalty_data.freqs, x_slice, penalty_data.srate, penalty_data.iir_hp_pk);
            let viol = viol_ceiling_from_spl(&peq_spl, penalty_data.max_db, penalty_data.iir_hp_pk);
            viol
        }
    };
    config_builder = config_builder.add_penalty_ineq(
        Box::new(ceiling_penalty),
        setup.penalty_data.penalty_w_ceiling
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

