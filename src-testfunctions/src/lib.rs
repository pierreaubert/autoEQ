//! Optimization test functions library
//!
//! This library provides a comprehensive collection of test functions for optimization
//! algorithm benchmarking and validation. Functions are organized by category:
//!
//! - **Unimodal**: Single global optimum functions (sphere, rosenbrock, etc.)
//! - **Multimodal**: Multiple local minima functions (ackley, rastrigin, etc.)
//! - **Constrained**: Functions with constraints (keanes bump, binh-korn, etc.)
//! - **Composite**: Hybrid functions combining multiple characteristics
//! - **Modern**: Recent benchmark functions from CEC competitions and research
//!
//! # Example
//!
//! ```rust
//! use ndarray::Array1;
//! use autoeq_testfunctions::*;
//!
//! let x = Array1::from_vec(vec![0.0, 0.0]);
//! let result = sphere(&x);
//! assert_eq!(result, 0.0);
//!
//! // Get function metadata
//! let metadata = get_function_metadata();
//! let bounds = get_function_bounds("sphere");
//! ```

#![allow(unused)]

use ndarray::{Array1, Array2};
use std::collections::HashMap;

// Import all function modules
pub mod functions;
pub use functions::*;

/// Metadata for a test function including bounds, constraints, and other properties
#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    /// Function name
    pub name: String,
    /// Bounds for each dimension (min, max)
    pub bounds: Vec<(f64, f64)>,
    /// Global minima locations and values
    pub global_minima: Vec<(Vec<f64>, f64)>,
    /// Inequality constraint functions (should be <= 0 when satisfied)
    pub inequality_constraints: Vec<fn(&Array1<f64>) -> f64>,
    /// Equality constraint functions (should be = 0 when satisfied)
    pub equality_constraints: Vec<fn(&Array1<f64>) -> f64>,
    /// Description of the function
    pub description: String,
    /// Whether the function is multimodal
    pub multimodal: bool,
    /// Typical dimension(s) for the function
    pub dimensions: Vec<usize>,
}

/// Create bounds matrix for optimization (2 x n matrix)
/// bounds[[0, i]] = lower bound, bounds[[1, i]] = upper bound
pub fn create_bounds(n: usize, lower: f64, upper: f64) -> Array2<f64> {
    Array2::from_shape_fn((2, n), |(i, _)| if i == 0 { lower } else { upper })
}

/// Get metadata for all available test functions
pub fn get_function_metadata() -> HashMap<String, FunctionMetadata> {
    let mut metadata = HashMap::new();

    // Add metadata for each function
    metadata.insert(
        "ackley".to_string(),
        FunctionMetadata {
            name: "ackley".to_string(),
            bounds: vec![(-32.768, 32.768); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "beale".to_string(),
        FunctionMetadata {
            name: "beale".to_string(),
            bounds: vec![(-4.5, 4.5); 2],
            global_minima: vec![(vec![3.0, 0.5], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "booth".to_string(),
        FunctionMetadata {
            name: "booth".to_string(),
            bounds: vec![(-10.0, 10.0); 2],
            global_minima: vec![(vec![1.0, 3.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D unimodal function".to_string(),
            multimodal: false,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "matyas".to_string(),
        FunctionMetadata {
            name: "matyas".to_string(),
            bounds: vec![(-10.0, 10.0); 2],
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D unimodal function".to_string(),
            multimodal: false,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "himmelblau".to_string(),
        FunctionMetadata {
            name: "himmelblau".to_string(),
            bounds: vec![(-5.0, 5.0); 2],
            global_minima: vec![
                (vec![3.0, 2.0], 0.0),
                (vec![-2.805118, 3.131312], 0.0),
                (vec![-3.779310, -3.283186], 0.0),
                (vec![3.584428, -1.848126], 0.0),
            ],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D multimodal function with 4 global minima".to_string(),
            multimodal: true,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "sphere".to_string(),
        FunctionMetadata {
            name: "sphere".to_string(),
            bounds: vec![(-5.0, 5.0); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional quadratic function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "rosenbrock".to_string(),
        FunctionMetadata {
            name: "rosenbrock".to_string(),
            bounds: vec![(-2.048, 2.048); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![1.0, 1.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional banana function".to_string(),
            multimodal: false,
            dimensions: vec![2, 4, 10],
        },
    );

    metadata.insert(
        "rastrigin".to_string(),
        FunctionMetadata {
            name: "rastrigin".to_string(),
            bounds: vec![(-5.12, 5.12); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional highly multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2, 5],
        },
    );

    metadata.insert(
        "griewank".to_string(),
        FunctionMetadata {
            name: "griewank".to_string(),
            bounds: vec![(-600.0, 600.0); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2, 10],
        },
    );

    metadata.insert(
        "schwefel".to_string(),
        FunctionMetadata {
            name: "schwefel".to_string(),
            bounds: vec![(-500.0, 500.0); 2], // Default 2D, but can be N-dimensional
            global_minima: vec![(vec![420.9687, 420.9687], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "N-dimensional multimodal with many local minima".to_string(),
            multimodal: true,
            dimensions: vec![2, 5],
        },
    );

    // Add constrained functions
    metadata.insert(
        "rosenbrock_disk_constraint".to_string(),
        FunctionMetadata {
            name: "rosenbrock_disk_constraint".to_string(),
            bounds: vec![(-1.5, 1.5); 2],
            global_minima: vec![(vec![1.0, 1.0], 0.0)], // Note: actual constrained minimum will be different
            inequality_constraints: vec![rosenbrock_disk_constraint],
            equality_constraints: vec![],
            description: "Disk constraint: x^2 + y^2 <= 2".to_string(),
            multimodal: false,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "binh_korn_constraint1".to_string(),
        FunctionMetadata {
            name: "binh_korn_constraint1".to_string(),
            bounds: vec![(0.0, 5.0), (0.0, 3.0)],
            global_minima: vec![(vec![0.0, 0.0], 0.0)], // Approximate
            inequality_constraints: vec![binh_korn_constraint1, binh_korn_constraint2],
            equality_constraints: vec![],
            description: "Binh-Korn constraints: x1^2 + x2^2 <= 25 and (x1-8)^2 + (x2+3)^2 >= 7.7"
                .to_string(),
            multimodal: false,
            dimensions: vec![2],
        },
    );

    // Add other commonly used functions
    metadata.insert(
        "branin".to_string(),
        FunctionMetadata {
            name: "branin".to_string(),
            bounds: vec![(-5.0, 10.0), (0.0, 15.0)],
            global_minima: vec![
                (vec![-std::f64::consts::PI, 12.275], 0.397887),
                (vec![std::f64::consts::PI, 2.275], 0.397887),
                (vec![9.42478, 2.475], 0.397887),
            ],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D multimodal function with 3 global minima".to_string(),
            multimodal: true,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "goldstein_price".to_string(),
        FunctionMetadata {
            name: "goldstein_price".to_string(),
            bounds: vec![(-2.0, 2.0); 2],
            global_minima: vec![(vec![0.0, -1.0], 3.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2],
        },
    );

    metadata.insert(
        "six_hump_camel".to_string(),
        FunctionMetadata {
            name: "six_hump_camel".to_string(),
            bounds: vec![(-3.0, 3.0), (-2.0, 2.0)],
            global_minima: vec![
                (vec![0.0898, -0.7126], -1.0316),
                (vec![-0.0898, 0.7126], -1.0316),
            ],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "2D multimodal function".to_string(),
            multimodal: true,
            dimensions: vec![2],
        },
    );

    // NEW SFU FUNCTIONS
    metadata.insert(
        "gramacy_lee_2012".to_string(),
        FunctionMetadata {
            name: "gramacy_lee_2012".to_string(),
            bounds: vec![(0.5, 2.5)],
            global_minima: vec![(vec![0.548563444114526], -0.869011134989500)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "1D test function with challenging properties by Gramacy & Lee (2012)"
                .to_string(),
            multimodal: true,
            dimensions: vec![1],
        },
    );

    metadata.insert(
        "perm_0_d_beta".to_string(),
        FunctionMetadata {
            name: "perm_0_d_beta".to_string(),
            bounds: vec![(-1.0, 1.0), (-1.0, 1.0)], // Default 2D, scalable
            global_minima: vec![(vec![1.0, 0.5], 0.0)], // (1, 1/2) for 2D
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Bowl-shaped Perm function (0, d, β variant)".to_string(),
            multimodal: false,
            dimensions: vec![2, 4, 5],
        },
    );

    metadata.insert(
        "sum_squares".to_string(),
        FunctionMetadata {
            name: "sum_squares".to_string(),
            bounds: vec![(-10.0, 10.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Simple quadratic bowl-shaped function (weighted sum of squares)"
                .to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "power_sum".to_string(),
        FunctionMetadata {
            name: "power_sum".to_string(),
            bounds: vec![(0.0, 4.0); 4], // Up to 4D
            global_minima: vec![],       // No exact global minimum - inconsistent constraint system
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description:
                "Power Sum function - constrained optimization problem with inconsistent parameters"
                    .to_string(),
            multimodal: true,
            dimensions: vec![2, 3, 4],
        },
    );

    metadata.insert(
        "forrester_2008".to_string(),
        FunctionMetadata {
            name: "forrester_2008".to_string(),
            bounds: vec![(0.0, 1.0)],
            global_minima: vec![(vec![0.757249], -6.02074)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "1D function for metamodeling by Forrester et al. (2008)".to_string(),
            multimodal: true,
            dimensions: vec![1],
        },
    );

    metadata.insert(
        "hartman_4d".to_string(),
        FunctionMetadata {
            name: "hartman_4d".to_string(),
            bounds: vec![(0.0, 1.0); 4],
            global_minima: vec![(vec![0.1873, 0.1936, 0.5576, 0.2647], -3.72983)], // Verified value
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "4D multimodal Hartmann function with 4 local minima".to_string(),
            multimodal: true,
            dimensions: vec![4],
        },
    );

    metadata.insert(
        "perm_d_beta".to_string(),
        FunctionMetadata {
            name: "perm_d_beta".to_string(),
            bounds: vec![(-1.0, 1.0), (-1.0, 1.0)], // Default 2D, scalable
            global_minima: vec![(vec![1.0, 0.5], 0.0)], // (1/1, 1/2) for 2D
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Perm function (d, β variant)".to_string(),
            multimodal: false,
            dimensions: vec![2, 3, 4],
        },
    );

    metadata.insert(
        "shekel".to_string(),
        FunctionMetadata {
            name: "shekel".to_string(),
            bounds: vec![(0.0, 10.0); 4], // Up to 4D
            global_minima: vec![(vec![4.0, 4.0, 4.0, 4.0], -10.5364)], // For m=10
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Multimodal Shekel function with 10 local minima".to_string(),
            multimodal: true,
            dimensions: vec![4],
        },
    );

    // ADDITIONAL CEC AND MODERN BENCHMARK FUNCTIONS
    metadata.insert(
        "xin_she_yang_n1".to_string(),
        FunctionMetadata {
            name: "xin_she_yang_n1".to_string(),
            bounds: vec![(-5.0, 5.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Xin-She Yang N.1 function - newer benchmark function".to_string(),
            multimodal: true,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "discus".to_string(),
        FunctionMetadata {
            name: "discus".to_string(),
            bounds: vec![(-100.0, 100.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Ill-conditioned unimodal discus function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "elliptic".to_string(),
        FunctionMetadata {
            name: "elliptic".to_string(),
            bounds: vec![(-100.0, 100.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Separable ill-conditioned elliptic function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "cigar".to_string(),
        FunctionMetadata {
            name: "cigar".to_string(),
            bounds: vec![(-100.0, 100.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Ill-conditioned cigar function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "tablet".to_string(),
        FunctionMetadata {
            name: "tablet".to_string(),
            bounds: vec![(-100.0, 100.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Ill-conditioned tablet function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "different_powers".to_string(),
        FunctionMetadata {
            name: "different_powers".to_string(),
            bounds: vec![(-1.0, 1.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Unimodal function with different power scaling per dimension".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "ridge".to_string(),
        FunctionMetadata {
            name: "ridge".to_string(),
            bounds: vec![(-5.0, 5.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Challenging unimodal ridge function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "sharp_ridge".to_string(),
        FunctionMetadata {
            name: "sharp_ridge".to_string(),
            bounds: vec![(-5.0, 5.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Even more challenging sharp ridge function".to_string(),
            multimodal: false,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "katsuura".to_string(),
        FunctionMetadata {
            name: "katsuura".to_string(),
            bounds: vec![(0.0, 100.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![0.0, 0.0], 0.0)], // f(x) = factor*(product-1), minimum when product=1
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Fractal-like multimodal Katsuura function".to_string(),
            multimodal: true,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "happycat".to_string(),
        FunctionMetadata {
            name: "happycat".to_string(),
            bounds: vec![(-2.0, 2.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![-1.0, -1.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Recent CEC benchmark HappyCat function".to_string(),
            multimodal: true,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "expanded_griewank_rosenbrock".to_string(),
        FunctionMetadata {
            name: "expanded_griewank_rosenbrock".to_string(),
            bounds: vec![(-5.0, 5.0); 2], // Default 2D, scalable
            global_minima: vec![(vec![1.0, 1.0], 0.0)],
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Hybrid function combining Griewank and Rosenbrock characteristics"
                .to_string(),
            multimodal: true,
            dimensions: vec![2, 5, 10],
        },
    );

    metadata.insert(
        "gramacy_lee_function".to_string(),
        FunctionMetadata {
            name: "gramacy_lee_function".to_string(),
            bounds: vec![(0.0, 1.0)],
            global_minima: vec![(vec![0.0], 1.0)], // f(x) = exp(x*(x-0.5)*(x-1)) + x²/10, minimum at x=0
            inequality_constraints: vec![],
            equality_constraints: vec![],
            description: "Alternative Gramacy & Lee function for Gaussian process testing"
                .to_string(),
            multimodal: true,
            dimensions: vec![1],
        },
    );

    metadata
}

/// Helper function to get bounds for a specific function from metadata
/// Returns None if function is not found in metadata
pub fn get_function_bounds(function_name: &str) -> Option<Vec<(f64, f64)>> {
    let metadata = get_function_metadata();
    metadata.get(function_name).map(|meta| meta.bounds.clone())
}

/// Helper function to get bounds as a 2D array for optimization (compatible with existing tests)
/// Returns default bounds if function is not found
pub fn get_function_bounds_2d(function_name: &str, default_bounds: (f64, f64)) -> [(f64, f64); 2] {
    if let Some(bounds) = get_function_bounds(function_name) {
        if bounds.len() >= 2 {
            [bounds[0], bounds[1]]
        } else {
            [default_bounds; 2]
        }
    } else {
        [default_bounds; 2]
    }
}

/// Helper function to get bounds as a Vec for optimization (compatible with recorded tests)
/// Returns default bounds if function is not found
pub fn get_function_bounds_vec(function_name: &str, default_bounds: (f64, f64)) -> Vec<(f64, f64)> {
    if let Some(bounds) = get_function_bounds(function_name) {
        if bounds.len() >= 2 {
            bounds
        } else {
            vec![default_bounds; 2]
        }
    } else {
        vec![default_bounds; 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    /// Helper function to get a function by name and call it
    /// This uses a match statement to map function names to actual function calls
    fn call_function(name: &str, x: &Array1<f64>) -> Option<f64> {
        match name {
            // Unimodal functions
            "sphere" => Some(sphere(x)),
            "rosenbrock" => Some(rosenbrock(x)),
            "booth" => Some(booth(x)),
            "matyas" => Some(matyas(x)),
            "beale" => Some(beale(x)),
            "himmelblau" => Some(himmelblau(x)),
            "sum_squares" => Some(sum_squares(x)),
            "different_powers" => Some(different_powers(x)),
            "elliptic" => Some(elliptic(x)),
            "cigar" => Some(cigar(x)),
            "tablet" => Some(tablet(x)),
            "discus" => Some(discus(x)),
            "ridge" => Some(ridge(x)),
            "sharp_ridge" => Some(sharp_ridge(x)),
            "perm_0_d_beta" => Some(perm_0_d_beta(x)),
            "perm_d_beta" => Some(perm_d_beta(x)),

            // Multimodal functions
            "ackley" => Some(ackley(x)),
            "rastrigin" => Some(rastrigin(x)),
            "griewank" => Some(griewank(x)),
            "schwefel" => Some(schwefel(x)),
            "branin" => Some(branin(x)),
            "goldstein_price" => Some(goldstein_price(x)),
            "six_hump_camel" => Some(six_hump_camel(x)),
            "hartman_4d" => Some(hartman_4d(x)),
            "xin_she_yang_n1" => Some(xin_she_yang_n1(x)),
            "katsuura" => Some(katsuura(x)),
            "happycat" => Some(happycat(x)),

            // Modern functions
            "gramacy_lee_2012" => Some(gramacy_lee_2012(x)),
            "forrester_2008" => Some(forrester_2008(x)),
            "power_sum" => Some(power_sum(x)),
            "shekel" => Some(shekel(x)),
            "gramacy_lee_function" => Some(gramacy_lee_function(x)),

            // Composite functions
            "expanded_griewank_rosenbrock" => Some(expanded_griewank_rosenbrock(x)),

            // Constrained functions (skip constraint tests for now)
            "rosenbrock_disk_constraint" | "binh_korn_constraint1" => None,

            _ => None,
        }
    }

    #[test]
    fn test_all_function_minima() {
        let metadata = get_function_metadata();
        let tolerance = 1e-10; // Very small tolerance for exact matches
        let loose_tolerance = 1e-3; // Looser tolerance for approximate matches

        for (func_name, meta) in metadata.iter() {
            // Skip constrained functions as they require special handling
            if meta.inequality_constraints.len() > 0 || meta.equality_constraints.len() > 0 {
                continue;
            }

            println!("Testing function: {}", func_name);

            // Test each global minimum
            for (minimum_location, expected_value) in &meta.global_minima {
                let x = Array1::from_vec(minimum_location.clone());

                if let Some(actual_value) = call_function(func_name, &x) {
                    let error = (actual_value - expected_value).abs();

                    // Use different tolerances based on the expected value magnitude
                    let test_tolerance = if expected_value.abs() > 1.0 {
                        loose_tolerance * expected_value.abs()
                    } else {
                        loose_tolerance
                    };

                    println!(
                        "  {} at {:?}: expected {:.6}, got {:.6}, error {:.2e}",
                        func_name, minimum_location, expected_value, actual_value, error
                    );

                    assert!(
                        error <= test_tolerance,
                        "Function {} failed: at {:?}, expected {:.10}, got {:.10}, error {:.2e} > tolerance {:.2e}",
                        func_name, minimum_location, expected_value, actual_value, error, test_tolerance
                    );

                    println!("  ✓ {} passed with error {:.2e}", func_name, error);
                } else {
                    println!(
                        "  ⚠ Skipped {} (not implemented in test dispatcher)",
                        func_name
                    );
                }
            }
        }

        println!("\n🎉 All function minima tests completed!");
    }

    #[test]
    fn test_specific_challenging_functions() {
        let tolerance = 1e-5;

        // Test some particularly challenging functions with known good values

        // Gramacy & Lee 2012 - should be very precise
        let x = Array1::from_vec(vec![0.548563444114526]);
        let result = gramacy_lee_2012(&x);
        let expected = -0.869011134989500;
        assert!(
            (result - expected).abs() < tolerance,
            "Gramacy & Lee 2012: expected {}, got {}",
            expected,
            result
        );

        // Forrester 2008 - should be very precise
        let x = Array1::from_vec(vec![0.757249]);
        let result = forrester_2008(&x);
        let expected = -6.02074;
        assert!(
            (result - expected).abs() < tolerance,
            "Forrester 2008: expected {}, got {}",
            expected,
            result
        );

        // Hartmann 4D - should be close
        let x = Array1::from_vec(vec![0.1873, 0.1936, 0.5576, 0.2647]);
        let result = hartman_4d(&x);
        let expected = -3.72983;
        assert!(
            (result - expected).abs() < tolerance,
            "Hartmann 4D: expected {}, got {}",
            expected,
            result
        );

        // Shekel - should be close (looser tolerance due to numerical precision)
        let x = Array1::from_vec(vec![4.0, 4.0, 4.0, 4.0]);
        let result = shekel(&x);
        let expected = -10.5364;
        let shekel_tolerance = 1e-3; // Looser tolerance for Shekel
        assert!(
            (result - expected).abs() < shekel_tolerance,
            "Shekel: expected {}, got {}",
            expected,
            result
        );
    }

    #[test]
    fn test_simple_unimodal_functions() {
        let tolerance = 1e-12;

        // Test functions that should have exact zeros
        let x = Array1::from_vec(vec![0.0, 0.0]);

        assert_eq!(sphere(&x), 0.0);
        assert_eq!(sum_squares(&x), 0.0);
        assert_eq!(different_powers(&x), 0.0);
        assert_eq!(elliptic(&x), 0.0);
        assert_eq!(cigar(&x), 0.0);
        assert_eq!(tablet(&x), 0.0);
        assert_eq!(discus(&x), 0.0);
        assert_eq!(ridge(&x), 0.0);
        assert_eq!(sharp_ridge(&x), 0.0);
        assert_eq!(xin_she_yang_n1(&x), 0.0);

        // Test functions with specific minima
        let x = Array1::from_vec(vec![1.0, 1.0]);
        assert!((rosenbrock(&x) - 0.0).abs() < tolerance);
        assert!((expanded_griewank_rosenbrock(&x) - 0.0).abs() < tolerance);

        let x = Array1::from_vec(vec![1.0, 3.0]);
        assert!((booth(&x) - 0.0).abs() < tolerance);

        let x = Array1::from_vec(vec![0.0, 0.0]);
        assert!((matyas(&x) - 0.0).abs() < tolerance);

        let x = Array1::from_vec(vec![3.0, 0.5]);
        assert!((beale(&x) - 0.0).abs() < tolerance);
    }

    #[test]
    fn test_multimodal_functions() {
        let tolerance = 1e-10;

        // Test functions that should be zero at origin
        let x = Array1::from_vec(vec![0.0, 0.0]);

        assert!((ackley(&x) - 0.0).abs() < tolerance);
        assert!((rastrigin(&x) - 0.0).abs() < tolerance);
        assert!((griewank(&x) - 0.0).abs() < tolerance);

        // Test Schwefel at its known minimum
        let x = Array1::from_vec(vec![420.9687, 420.9687]);
        assert!((schwefel(&x) - 0.0).abs() < 1e-3); // Schwefel is less precise
    }

    #[test]
    fn test_perm_functions() {
        let tolerance = 1e-12;

        // Test Perm 0,d,β at (1, 1/2)
        let x = Array1::from_vec(vec![1.0, 0.5]);
        assert!((perm_0_d_beta(&x) - 0.0).abs() < tolerance);
        assert!((perm_d_beta(&x) - 0.0).abs() < tolerance);
    }

    #[test]
    fn test_function_metadata_completeness() {
        let metadata = get_function_metadata();

        // Ensure all functions have proper metadata
        for (name, meta) in metadata.iter() {
            assert!(!meta.name.is_empty(), "Function {} has empty name", name);
            assert!(!meta.bounds.is_empty(), "Function {} has no bounds", name);
            // Allow functions with no global minima (e.g., power_sum with inconsistent constraints)
            // assert!(!meta.global_minima.is_empty(), "Function {} has no global minima", name);
            assert!(
                !meta.description.is_empty(),
                "Function {} has no description",
                name
            );
            assert!(
                !meta.dimensions.is_empty(),
                "Function {} has no dimensions",
                name
            );

            // Check that bounds make sense
            for (lower, upper) in &meta.bounds {
                assert!(
                    lower < upper,
                    "Function {} has invalid bounds: {} >= {}",
                    name,
                    lower,
                    upper
                );
            }

            // Check that global minima have correct dimensionality
            for (location, _value) in &meta.global_minima {
                if !meta.bounds.is_empty() {
                    // Allow some flexibility for functions with variable dimensions
                    assert!(
                        location.len() <= meta.bounds.len() || meta.dimensions.len() > 1,
                        "Function {} has global minimum with wrong dimensions: {} vs bounds {}",
                        name,
                        location.len(),
                        meta.bounds.len()
                    );
                }
            }
        }

        println!("✓ All {} functions have complete metadata", metadata.len());
    }
}
