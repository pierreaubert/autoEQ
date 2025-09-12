//! Gramacy Lee 2012 test function

use ndarray::Array1;

/// Gramacy & Lee (2012) Function - 1D test function with challenging properties  
/// f(x) = sin(10*pi*x) / (2*x) + (x-1)^4
/// Global minimum: f(x) ≈ -0.869011134989500 at x ≈ 0.548563444114526
/// Bounds: x in [0.5, 2.5]
pub fn gramacy_lee_2012(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let pi = std::f64::consts::PI;
    (10.0 * pi * x1).sin() / (2.0 * x1) + (x1 - 1.0).powi(4)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gramacy_lee_2012_known_properties() {
        use crate::{get_function_metadata, FunctionMetadata};
        use ndarray::Array1;

        // Get metadata for this function
        let metadata = get_function_metadata();
        let meta = metadata
            .get("gramacy_lee_2012")
            .expect("Function gramacy_lee_2012 should have metadata");

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
        let tolerance = 1e-6; // Reasonable tolerance for numerical precision
        for (minimum_coords, expected_value) in &meta.global_minima {
            let x = Array1::from_vec(minimum_coords.clone());
            let actual_value = gramacy_lee_2012(&x);

            let error = (actual_value - expected_value).abs();
            assert!(
                error <= tolerance,
                "Function value at global minimum {:?} should be {}, got {}, error: {}",
                minimum_coords,
                expected_value,
                actual_value,
                error
            );
        }

        // Test 3: Basic function properties
        if !meta.global_minima.is_empty() {
            let (first_minimum, _) = &meta.global_minima[0];
            let x = Array1::from_vec(first_minimum.clone());
            let result = gramacy_lee_2012(&x);

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
