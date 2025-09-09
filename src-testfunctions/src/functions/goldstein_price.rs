//! Goldstein Price test function

use ndarray::Array1;

/// Goldstein-Price function - multimodal, 2D only
/// Global minimum: f(x) = 3 at x = (0, -1)
/// Bounds: x_i in [-2, 2]
pub fn goldstein_price(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let term1 = 1.0 + (x1 + x2 + 1.0).powi(2) *
        (19.0 - 14.0*x1 + 3.0*x1.powi(2) - 14.0*x2 + 6.0*x1*x2 + 3.0*x2.powi(2));
    let term2 = 30.0 + (2.0*x1 - 3.0*x2).powi(2) *
        (18.0 - 32.0*x1 + 12.0*x1.powi(2) + 48.0*x2 - 36.0*x1*x2 + 27.0*x2.powi(2));
    term1 * term2
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goldstein_price_known_properties() {
        use ndarray::Array1;
        use crate::{get_function_metadata, FunctionMetadata};

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata.get("goldstein_price").expect("Function goldstein_price should have metadata");

        // Test 1: Verify global minima are within bounds
        for (minimum_coords, expected_value) in &meta.global_minima {
            assert!(minimum_coords.len() >= meta.bounds.len() || meta.bounds.len() == 1, 
                "Global minimum coordinates should match bounds dimensions");
            
            for (i, &coord) in minimum_coords.iter().enumerate() {
                if i < meta.bounds.len() {
                    let (lower, upper) = meta.bounds[i];
                    assert!(coord >= lower && coord <= upper,
                        "Global minimum coordinate {} = {} should be within bounds [{} {}]",
                        i, coord, lower, upper);
                }
            }
        }

        // Test 2: Verify function evaluates to expected values at global minima
        let tolerance = 1e-6; // Reasonable tolerance for numerical precision
        for (minimum_coords, expected_value) in &meta.global_minima {
            let x = Array1::from_vec(minimum_coords.clone());
            let actual_value = goldstein_price(&x);
            
            let error = (actual_value - expected_value).abs();
            assert!(error <= tolerance,
                "Function value at global minimum {:?} should be {}, got {}, error: {}",
                minimum_coords, expected_value, actual_value, error);
        }

        // Test 3: Basic function properties
        if !meta.global_minima.is_empty() {
            let (first_minimum, _) = &meta.global_minima[0];
            let x = Array1::from_vec(first_minimum.clone());
            let result = goldstein_price(&x);
            
            assert!(result.is_finite(), "Function should return finite values at global minimum");
            assert!(!result.is_nan(), "Function should not return NaN at global minimum");
        }
    }
}
