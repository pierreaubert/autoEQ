//! Schwefel test function

use ndarray::Array1;

/// Schwefel function - multimodal with many local minima
/// Global minimum: f(x) = 0 at x = (420.9687, 420.9687, ..., 420.9687)
/// Bounds: x_i in [-500, 500]
pub fn schwefel(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum: f64 = x.iter().map(|&xi| xi * xi.abs().sqrt().sin()).sum();
    418.9829 * n - sum
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schwefel_known_properties() {
        use crate::{get_function_metadata, FunctionMetadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("schwefel")
            .expect("Function schwefel should have metadata");

        // Test 1: Verify global minima are within bounds
        for (minimum_coords, expected_value) in &meta.global_minima {
            assert!(
                minimum_coords.len() >= meta.bounds.len() || meta.bounds.len() == 1,
                "Global minimum coordinates should match bounds dimensions"
            );

            for (i, &coord) in minimum_coords.iter().enumerate() {
                if i < meta.bounds.len() {
                    let (lower, upper) = meta.bounds[i];
                    assert!(
                        coord >= lower && coord <= upper,
                        "Global minimum coordinate {} = {} should be within bounds [{} {}]",
                        i,
                        coord,
                        lower,
                        upper
                    );
                }
            }
        }

        // Test 2: Verify function evaluates to expected values at global minima
        for (minimum_coords, expected_value) in &meta.global_minima {
            let x = Array1::from_vec(minimum_coords.clone());
            let actual_value = schwefel(&x);

            let error = (actual_value - expected_value).abs();
            // Use adaptive tolerance based on magnitude of expected value
            let tolerance = if expected_value.abs() > 1.0 {
                1e-4 * expected_value.abs() // Relative tolerance for large values
            } else if expected_value.abs() == 0.0 {
                1e-4 // Higher tolerance for zero expected values (schwefel case)
            } else {
                1e-6 // Absolute tolerance for small non-zero values
            };

            assert!(error <= tolerance,
                "Function value at global minimum {:?} should be {}, got {}, error: {} (tolerance: {})",
                minimum_coords, expected_value, actual_value, error, tolerance);
        }

        // Test 3: Basic function properties
        if !meta.global_minima.is_empty() {
            let (first_minimum, _) = &meta.global_minima[0];
            let x = Array1::from_vec(first_minimum.clone());
            let result = schwefel(&x);

            assert!(
                result.is_finite(),
                "Function should return finite values at global minimum"
            );
            assert!(
                !result.is_nan(),
                "Function should not return NaN at global minimum"
            );
        }
    }
}
