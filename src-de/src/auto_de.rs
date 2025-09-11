use ndarray::{Array1, Array2};
use crate::{
    DEConfigBuilder, Strategy, Mutation, Crossover, Init,
    differential_evolution,
};

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
    F: Fn(&Array1<f64>) -> f64 + Sync,
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




#[cfg(test)]
mod tests {
    use super::*;
    use autoeq_testfunctions::{create_bounds, quadratic};


    #[test]
    fn test_auto_de_custom_parameters() {
        // Test with custom parameters
        let bounds = create_bounds(2, -5.0, 5.0);

        let params = AutoDEParams {
            max_iterations: 500,
            population_size: None, // Will use default based on dimension
            f: 0.7,                // Mutation factor
            cr: 0.8,               // Crossover probability
            tolerance: 1e-8,
            seed: Some(12345),
        };

        let result = auto_de(quadratic, &bounds, Some(params));

        assert!(
            result.is_some(),
            "AutoDE should find a solution with custom params"
        );
        let (x_opt, f_opt, iterations) = result.unwrap();

        // Should still find the optimum
        assert!(
            f_opt < 1e-6,
            "Function value too high with custom params: {}",
            f_opt
        );
        for &xi in x_opt.iter() {
            assert!(xi.abs() < 1e-3, "Solution component too far from 0: {}", xi);
        }

        // Should use specified max iterations
        assert!(
            iterations <= 500,
            "Used more iterations than specified: {}",
            iterations
        );
    }

    #[test]
    fn test_auto_de_parameter_validation() {
        let bounds = create_bounds(2, -5.0, 5.0);

        // Test invalid mutation factor
        let invalid_params = AutoDEParams {
            max_iterations: 100,
            population_size: None,
            f: 2.5, // Invalid: should be in [0, 2]
            cr: 0.5,
            tolerance: 1e-6,
            seed: None,
        };

        let result = auto_de(quadratic, &bounds, Some(invalid_params));
        assert!(result.is_none(), "Should fail with invalid mutation factor");

        // Test invalid crossover probability
        let invalid_params2 = AutoDEParams {
            max_iterations: 100,
            population_size: None,
            f: 0.5,
            cr: 1.5, // Invalid: should be in [0, 1]
            tolerance: 1e-6,
            seed: None,
        };

        let result2 = auto_de(quadratic, &bounds, Some(invalid_params2));
        assert!(
            result2.is_none(),
            "Should fail with invalid crossover probability"
        );
    }

    #[test]
    fn test_auto_de_convergence_tolerance() {
        let bounds = create_bounds(2, -5.0, 5.0);

        // Test with loose tolerance - should converge faster
        let loose_params = AutoDEParams {
            max_iterations: 1000,
            population_size: None,
            f: 0.5,
            cr: 0.7,
            tolerance: 1e-2, // Loose tolerance
            seed: Some(42),
        };

        let result = auto_de(quadratic, &bounds, Some(loose_params));
        assert!(result.is_some());
        let (_, f_opt, iterations) = result.unwrap();

        // Should converge with loose tolerance
        assert!(f_opt < 1e-2, "Function value should meet loose tolerance");

        // Test with tight tolerance
        let tight_params = AutoDEParams {
            max_iterations: 1000,
            population_size: None,
            f: 0.5,
            cr: 0.7,
            tolerance: 1e-10, // Very tight tolerance
            seed: Some(42),
        };

        let result2 = auto_de(quadratic, &bounds, Some(tight_params));
        if let Some((_, f_opt2, iterations2)) = result2 {
            // If it converges, should meet tight tolerance
            assert!(f_opt2 < 1e-8, "Function value should meet tight tolerance");
            // Might take more iterations
            assert!(
                iterations2 >= iterations,
                "Tight tolerance should take more iterations"
            );
        }
        // If it doesn't converge within max_iterations, that's also acceptable
    }

    #[test]
    fn test_auto_de_reproducibility() {
        // Test that same seed gives same results
        let bounds = create_bounds(3, -2.0, 2.0);

        let params = AutoDEParams {
            max_iterations: 200,
            population_size: Some(30),
            f: 0.6,
            cr: 0.8,
            tolerance: 1e-6,
            seed: Some(98765),
        };

        let result1 = auto_de(quadratic, &bounds, Some(params.clone()));
        let result2 = auto_de(quadratic, &bounds, Some(params));

        assert!(
            result1.is_some() && result2.is_some(),
            "Both runs should succeed"
        );
        let (x1, f1, iter1) = result1.unwrap();
        let (x2, f2, iter2) = result2.unwrap();

        // Same seed should give same results
        assert!(
            (f1 - f2).abs() < 1e-12,
            "Function values should be identical: {} vs {}",
            f1,
            f2
        );
        assert_eq!(iter1, iter2, "Iteration counts should be identical");
        for (i, (a, b)) in x1.iter().zip(x2.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-12,
                "Solution components should be identical: x[{}] = {} vs {}",
                i,
                a,
                b
            );
        }
    }

    #[test]
    fn test_auto_de_invalid_bounds() {
        use ndarray::Array2;

        // Test with invalid bounds (lower > upper)
        let mut bounds = Array2::zeros((2, 2));
        bounds[[0, 0]] = 5.0;
        bounds[[1, 0]] = 1.0; // Invalid: 5 > 1
        bounds[[0, 1]] = -1.0;
        bounds[[1, 1]] = 1.0; // Valid: -1 < 1

        let result = auto_de(quadratic, &bounds, None);
        assert!(result.is_none(), "Should fail with invalid bounds");
    }

    #[test]
    fn test_auto_de_empty_bounds() {
        use ndarray::Array2;

        // Test with empty bounds
        let bounds = Array2::zeros((2, 0));
        let result = auto_de(quadratic, &bounds, None);
        assert!(result.is_none(), "Should fail with empty bounds");
    }

    #[test]
    fn test_auto_de_default_parameters() {
        // Test that default parameters work correctly
        let bounds = create_bounds(3, -5.0, 5.0);
        let result = auto_de(quadratic, &bounds, None);

        assert!(
            result.is_some(),
            "AutoDE should work with default parameters"
        );
        let (x_opt, f_opt, _) = result.unwrap();

        assert!(
            f_opt < 1e-6,
            "Should find good solution with defaults: {}",
            f_opt
        );
        for &xi in x_opt.iter() {
            assert!(
                xi.abs() < 1e-2,
                "Solution component should be close to 0: {}",
                xi
            );
        }
    }

    #[test]
    fn test_auto_de_population_size_scaling() {
        let bounds = create_bounds(2, -5.0, 5.0);

        // Test explicit small population
        let small_pop_params = AutoDEParams {
            max_iterations: 100,
            population_size: Some(10), // Small population
            f: 0.8,
            cr: 0.9,
            tolerance: 1e-6,
            seed: Some(111),
        };

        let result1 = auto_de(quadratic, &bounds, Some(small_pop_params));
        assert!(result1.is_some(), "Should work with small population");

        // Test explicit large population
        let large_pop_params = AutoDEParams {
            max_iterations: 100,
            population_size: Some(100), // Large population
            f: 0.8,
            cr: 0.9,
            tolerance: 1e-6,
            seed: Some(111),
        };

        let result2 = auto_de(quadratic, &bounds, Some(large_pop_params));
        assert!(result2.is_some(), "Should work with large population");

        // Both should find good solutions
        let (_, f1, _) = result1.unwrap();
        let (_, f2, _) = result2.unwrap();
        assert!(f1 < 1e-4 && f2 < 1e-4, "Both should find good solutions");
    }
}

