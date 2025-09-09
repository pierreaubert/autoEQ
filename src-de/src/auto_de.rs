use ndarray::{Array1, Array2};
use crate::{
    AutoDEParams, DEConfigBuilder, Strategy, Mutation, Crossover, Init,
    differential_evolution,
};

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
