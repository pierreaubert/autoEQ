// AutoEQ DE-specific optimization code

use ndarray::Array1;
use std::sync::Arc;

use super::cli::PeqModel;
use super::constraints::{
    constraint_ceiling, constraint_min_gain, constraint_spacing, CeilingConstraintData,
    MinGainConstraintData, SpacingConstraintData,
};
use super::init_sobol::init_sobol;
use super::initial_guess::{create_smart_initial_guesses, SmartInitConfig};
use super::optim::{compute_fitness_penalties, ObjectiveData};
use crate::de::{
    differential_evolution, CallbackAction, DEConfigBuilder, DEIntermediate, DEReport, Init,
    Mutation, NonlinearConstraintHelper, ParallelConfig, Strategy,
};

/// Common setup for DE-based optimization
///
/// Contains all the shared configuration parameters for both standard and adaptive DE algorithms.
pub struct DESetup {
    /// Parameter bounds as (lower, upper) tuples for optde
    pub bounds: Vec<(f64, f64)>,
    /// Objective data with penalty weights configured
    pub penalty_data: ObjectiveData,
    /// Population size (minimum 15)
    pub pop_size: usize,
    /// Maximum iterations derived from maxeval and population
    pub max_iter: usize,
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
pub fn setup_de_common(
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

    // Estimate parameters
    let pop_size = population.max(15); // minimum reasonable population
    let max_iter = maxeval.min(pop_size * 10);

    // Set up objective data for DE with zero penalties since we use true constraints
    let mut penalty_data = objective_data.clone();
    penalty_data.penalty_w_ceiling = 0.0;
    penalty_data.penalty_w_spacing = 0.0;
    penalty_data.penalty_w_mingain = 0.0;

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
pub fn create_de_callback(
    algo_name: &str,
) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
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
                "(--) ".to_string()
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
pub fn create_de_objective(penalty_data: ObjectiveData) -> impl Fn(&Array1<f64>) -> f64 {
    move |x_arr: &Array1<f64>| -> f64 {
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
pub fn process_de_results(
    x: &mut [f64],
    result: DEReport,
    algo_name: &str,
) -> Result<(String, f64), (String, f64)> {
    // Copy results back to input array
    if result.x.len() == x.len() {
        for (i, &value) in result.x.iter().enumerate() {
            x[i] = value;
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
pub fn optimize_filters_autoeq(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    autoeq_name: &str,
    population: usize,
    maxeval: usize,
    cli_args: &crate::cli::Args,
) -> Result<(String, f64), (String, f64)> {
    // Create the callback with all the logging and user feedback
    let callback = create_de_callback("autoeq::DE");

    // Delegate to the callback-based version
    optimize_filters_autoeq_with_callback(
        x,
        lower_bounds,
        upper_bounds,
        objective_data,
        autoeq_name,
        population,
        maxeval,
        cli_args,
        callback,
    )
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
    mut callback: Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send>,
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

    // Create smart initialization based on frequency response analysis
    let num_filters = x.len() / 3;
    let smart_config = SmartInitConfig::default();

    // Use the inverted target as the response to analyze for problems
    let target_response = &setup.penalty_data.deviation;
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
            wls_enabled: false,                      // Disable WLS for stability
            w_max: 0.8,                              // Reduce max weight for more stability
            w_min: 0.2,                              // Increase min weight for more stability
            w_f: cli_args.adaptive_weight_f * 0.5,   // Make adaptation even more conservative
            w_cr: cli_args.adaptive_weight_cr * 0.5, // Make adaptation even more conservative
            f_m: 0.6,                                // Start with slightly higher F
            cr_m: 0.5,                               // Start with slightly lower CR
            wls_prob: 0.0,                           // Completely disable WLS
            wls_scale: 0.0,                          // Completely disable WLS
        })
    } else {
        None
    };

    // Adjust tolerance for adaptive strategies (they need much more relaxed convergence)
    let (tolerance, atolerance) =
        if matches!(strategy, Strategy::AdaptiveBin | Strategy::AdaptiveExp) {
            // Use much more relaxed tolerances for adaptive strategies - they converge differently
            (cli_args.tolerance * 10.0, cli_args.atolerance * 10.0)
        } else {
            (cli_args.tolerance, cli_args.atolerance)
        };

    // Use constraint helpers for nonlinear constraints
    let mut config_builder = DEConfigBuilder::new()
        .maxiter(setup.max_iter)
        .popsize(setup.pop_size)
        .tol(tolerance)
        .atol(atolerance)
        .strategy(strategy)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .recombination(cli_args.recombination)
        .init(Init::LatinHypercube) // Use Latin Hypercube sampling for population
        .x0(best_initial_guess) // Use smart guess as initial best individual
        .disp(false)
        .callback(Box::new(move |intermediate| callback(intermediate)));

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
            if cli_args.parallel_threads.eq(&0) {
                "all available".to_string()
            } else {
                cli_args.parallel_threads.to_string()
            }
        );
    }

    // Add native nonlinear constraints (always apply, not just in HP+PK mode)
    let mut config = config_builder.build();

    // Ceiling constraint (only applies in HP+PK mode)
    if setup.penalty_data.peq_model == PeqModel::HpPk && setup.penalty_data.max_db > 0.0 {
        let ceiling_data = CeilingConstraintData {
            freqs: setup.penalty_data.freqs.clone(),
            srate: setup.penalty_data.srate,
            max_db: setup.penalty_data.max_db,
            peq_model: setup.penalty_data.peq_model,
        };

        // Create nonlinear constraint helper for ceiling constraint
        let ceiling_constraint = NonlinearConstraintHelper {
            fun: Arc::new(move |x: &Array1<f64>| {
                let mut result = Array1::zeros(1);
                let mut data = ceiling_data.clone();
                result[0] = constraint_ceiling(x.as_slice().unwrap(), None, &mut data);
                result
            }),
            lb: Array1::from(vec![-f64::INFINITY]),
            ub: Array1::from(vec![0.0]),
        };

        // Apply constraint with appropriate penalty weights
        ceiling_constraint.apply_to(&mut config, 1e3, 1e3);
    }

    // Minimum gain constraint (applies in all modes)
    if setup.penalty_data.min_db > 0.0 {
        let min_gain_data = MinGainConstraintData {
            min_db: setup.penalty_data.min_db,
            peq_model: setup.penalty_data.peq_model,
        };

        // Create nonlinear constraint helper for minimum gain constraint
        let min_gain_constraint = NonlinearConstraintHelper {
            fun: Arc::new(move |x: &Array1<f64>| {
                let mut result = Array1::zeros(1);
                let mut data = min_gain_data.clone();
                result[0] = constraint_min_gain(x.as_slice().unwrap(), None, &mut data);
                result
            }),
            lb: Array1::from(vec![-f64::INFINITY]),
            ub: Array1::from(vec![0.0]),
        };

        // Apply constraint with appropriate penalty weights
        min_gain_constraint.apply_to(&mut config, 1e3, 1e3);
    }

    // Minimum spacing constraint (applies in all modes)
    if setup.penalty_data.min_spacing_oct > 0.0 {
        let spacing_data = SpacingConstraintData {
            min_spacing_oct: setup.penalty_data.min_spacing_oct,
        };

        // Create nonlinear constraint helper for minimum spacing constraint
        let spacing_constraint = NonlinearConstraintHelper {
            fun: Arc::new(move |x: &Array1<f64>| {
                let mut result = Array1::zeros(1);
                let mut data = spacing_data.clone();
                result[0] = constraint_spacing(x.as_slice().unwrap(), None, &mut data);
                result
            }),
            lb: Array1::from(vec![-f64::INFINITY]),
            ub: Array1::from(vec![0.0]),
        };

        // Apply constraint with appropriate penalty weights
        spacing_constraint.apply_to(&mut config, 1e3, 1e3);
    }

    let result = differential_evolution(&base_objective_fn, &setup.bounds, config);
    process_de_results(x, result, "AutoDE")
}
