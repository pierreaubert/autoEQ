//! Shared test functions and utilities for differential evolution optimization tests
//! 
//! This module contains commonly used test functions and utilities for DE tests.

use ndarray::{Array1, Array2};
pub use autoeq::optim::auto_de;

/// Simple quadratic function for basic testing
/// f(x) = sum(x[i]^2)
/// Global minimum at (0, 0, ..., 0) with f = 0
pub fn quadratic(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| xi * xi).sum()
}

/// Create bounds matrix for optimization (2 x n matrix)
/// bounds[[0, i]] = lower bound, bounds[[1, i]] = upper bound
pub fn create_bounds(n: usize, lower: f64, upper: f64) -> Array2<f64> {
    Array2::from_shape_fn((2, n), |(i, _)| if i == 0 { lower } else { upper })
}

/// Simplified Lampinen test problem (unconstrained version)
/// f(x) = sum(5*x[i]) - sum(x[i]^2) for i in 0..4, - sum(x[j]) for j in 4..
pub fn lampinen_simplified(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    
    // First 4 variables: 5*x[i] - x[i]^2
    for i in 0..4.min(x.len()) {
        sum += 5.0 * x[i] - x[i] * x[i];
    }
    
    // Remaining variables: -x[j]
    for i in 4..x.len() {
        sum -= x[i];
    }
    
    -sum  // Minimize negative (i.e., maximize original)
}

/// Basic sphere function for testing
/// f(x) = sum(x[i]^2)
/// Same as quadratic, but kept separate for clarity in different test contexts
pub fn sphere(x: &Array1<f64>) -> f64 {
    x.iter().map(|&v| v * v).sum()
}

// Benchmark functions used in landscape tests

/// Trid function - unimodal, bowl-shaped
/// Global minimum for 2D: f(x) = -2 at x = (2, 2)
/// Bounds: x_i in [-d^2, d^2] where d is dimension
pub fn trid(x: &Array1<f64>) -> f64 {
    let sum1 = x.iter().map(|&xi| (xi - 1.0).powi(2)).sum::<f64>();
    let sum2 = x.windows(2).into_iter().map(|w| w[0] * w[1]).sum::<f64>();
    sum1 - sum2
}

/// Bent Cigar function - ill-conditioned, unimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn bent_cigar(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + 1e6 * x.iter().skip(1).map(|&xi| xi.powi(2)).sum::<f64>()
}

/// Sum of different powers function - unimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 1]
pub fn sum_of_different_powers(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| xi.abs().powf(i as f64 + 2.0))
        .sum::<f64>()
}

/// Step function - discontinuous, multimodal
/// Global minimum: f(x) = 0 at x = (0.5, 0.5, ..., 0.5) 
/// Bounds: x_i in [-100, 100]
pub fn step(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum::<f64>()
}

/// Quartic function with noise - unimodal with added random noise
/// Global minimum: f(x) ≈ 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1.28, 1.28]
pub fn quartic(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| (i as f64 + 1.0) * xi.powi(4))
        .sum::<f64>()
    // Note: Original includes random noise, but we omit it for deterministic testing
}

/// Salomon function - multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn salomon(x: &Array1<f64>) -> f64 {
    let norm = x.iter().map(|&xi| xi.powi(2)).sum::<f64>().sqrt();
    1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm
}

/// Cosine mixture function - multimodal
/// Global minimum depends on dimension
/// Bounds: x_i in [-1, 1]
pub fn cosine_mixture(x: &Array1<f64>) -> f64 {
    let sum_cos = x.iter().map(|&xi| (5.0 * std::f64::consts::PI * xi).cos()).sum::<f64>();
    let sum_sq = x.iter().map(|&xi| xi.powi(2)).sum::<f64>();
    -0.1 * sum_cos + sum_sq
}

/// Lévy function N.13 - multimodal function
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10, 10]
pub fn levy_n13(x: &Array1<f64>) -> f64 {
    let w1 = 1.0 + (x[0] - 1.0) / 4.0;
    let w2 = 1.0 + (x[1] - 1.0) / 4.0;
    
    (3.0 * std::f64::consts::PI * w1).sin().powi(2)
        + (w1 - 1.0).powi(2) * (1.0 + (3.0 * std::f64::consts::PI * w2).sin().powi(2))
        + (w2 - 1.0).powi(2) * (1.0 + (2.0 * std::f64::consts::PI * w2).sin().powi(2))
}

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

/// Rotated hyper-ellipsoid function - unimodal, non-separable
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-65.536, 65.536]
pub fn rotated_hyper_ellipsoid(x: &Array1<f64>) -> f64 {
    (0..x.len())
        .map(|i| x.iter().take(i + 1).map(|&xi| xi.powi(2)).sum::<f64>())
        .sum::<f64>()
}

/// Ackley N.2 function - challenging multimodal function
/// Global minimum: f(x*)=-200 at x=(0,0)
/// Bounds: x_i in [-32, 32]
pub fn ackley_n2(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -200.0 * (-0.02 * (x1.powi(2) + x2.powi(2)).sqrt()).exp()
        * (2.0 * std::f64::consts::PI * x1).cos()
        * (2.0 * std::f64::consts::PI * x2).cos()
}

/// Powell function - unimodal but ill-conditioned
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-4, 5]
pub fn powell(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum = 0.0;
    for i in (0..n).step_by(4) {
        if i + 3 < n {
            let x1 = x[i];
            let x2 = x[i + 1];
            let x3 = x[i + 2];
            let x4 = x[i + 3];
            sum += (x1 + 10.0 * x2).powi(2)
                + 5.0 * (x3 - x4).powi(2)
                + (x2 - 2.0 * x3).powi(4)
                + 10.0 * (x1 - x4).powi(4);
        }
    }
    sum
}

/// Dixon's Price function - unimodal, non-separable
/// Global minimum: f(x) = 0 at x = (1, 2^(-1/2), 2^(-2/2), ..., 2^(-(i-1)/2))
/// Bounds: x_i in [-10, 10]
pub fn dixons_price(x: &Array1<f64>) -> f64 {
    let first_term = (x[0] - 1.0).powi(2);
    let sum_term: f64 = x.iter().skip(1).enumerate()
        .map(|(i, &xi)| (i + 2) as f64 * (2.0 * xi.powi(2) - x[i]).powi(2))
        .sum();
    first_term + sum_term
}

/// Lévi N.13 function (alias for levy_n13 for compatibility)
pub fn levi13(x: &Array1<f64>) -> f64 {
    levy_n13(x)
}

