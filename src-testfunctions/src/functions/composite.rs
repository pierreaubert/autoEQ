//! Composite and hybrid test functions
//!
//! These functions combine characteristics of multiple functions or are
//! designed as challenging hybrid benchmarks.

use ndarray::Array1;

/// Freudenstein and Roth function - multimodal with ill-conditioning
/// Global minimum: f(x) = 0 at x = (5, 4)
/// Bounds: x_i in [-10, 10]
pub fn freudenstein_roth(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (-13.0 + x1 + ((5.0 - x2) * x2 - 2.0) * x2).powi(2)
        + (-29.0 + x1 + ((x2 + 1.0) * x2 - 14.0) * x2).powi(2)
}

/// Colville function - multimodal, non-separable
/// Global minimum: f(x) = 0 at x = (1, 1, 1, 1)
/// Bounds: x_i in [-10, 10]
pub fn colville(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let x3 = if x.len() > 2 { x[2] } else { 1.0 };
    let x4 = if x.len() > 3 { x[3] } else { 1.0 };

    100.0 * (x1.powi(2) - x2).powi(2)
        + (x1 - 1.0).powi(2)
        + (x3 - 1.0).powi(2)
        + 90.0 * (x3.powi(2) - x4).powi(2)
        + 10.1 * ((x2 - 1.0).powi(2) + (x4 - 1.0).powi(2))
        + 19.8 * (x2 - 1.0) * (x4 - 1.0)
}

/// Expanded Griewank plus Rosenbrock function (F8F2)
/// Combines characteristics of both functions
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-5, 5]
pub fn expanded_griewank_rosenbrock(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    let n = x.len();
    
    for i in 0..n {
        let xi = x[i];
        let xi_plus_1 = x[(i + 1) % n]; // Wrap around for the last element
        
        // Rosenbrock component
        let rosenbrock = 100.0 * (xi.powi(2) - xi_plus_1).powi(2) + (xi - 1.0).powi(2);
        
        // Griewank transformation of Rosenbrock
        let griewank_part = rosenbrock / 4000.0 - (rosenbrock).cos() + 1.0;
        
        sum += griewank_part;
    }
    
    sum
}
