//! Shared test functions and utilities for differential evolution optimization tests
//!
//! This module contains testfunctionsly used test functions and utilities for DE tests.

#![allow(unused)]

use ndarray::{Array1, Array2};

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

/// Griewank function - multimodal, challenging for large dimensions
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-600, 600]
pub fn griewank(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let product_cos: f64 = x.iter().enumerate()
        .map(|(i, &xi)| (xi / ((i + 1) as f64).sqrt()).cos())
        .product();
    1.0 + sum_squares / 4000.0 - product_cos
}

/// Griewank2 function - variant of Griewank with different scaling
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-600, 600]
pub fn griewank2(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let product_cos: f64 = x.iter().enumerate()
        .map(|(i, &xi)| (xi / ((i + 1) as f64).sqrt()).cos())
        .product();
    sum_squares / 4000.0 - product_cos + 1.0
}

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

/// Schwefel function - multimodal with many local minima
/// Global minimum: f(x) = 0 at x = (420.9687, 420.9687, ..., 420.9687)
/// Bounds: x_i in [-500, 500]
pub fn schwefel(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum: f64 = x.iter()
        .map(|&xi| xi * xi.abs().sqrt().sin())
        .sum();
    418.9829 * n - sum
}

/// Eggholder function - highly multimodal, very challenging
/// Global minimum: f(x) = -959.6407 at x = (512, 404.2319)
/// Bounds: x_i in [-512, 512]
pub fn eggholder(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -(x2 + 47.0) * (x2 + x1/2.0 + 47.0).abs().sqrt().sin() -
    x1 * (x1 - x2 - 47.0).abs().sqrt().sin()
}

/// Bukin N.6 function - highly multimodal with narrow global optimum
/// Global minimum: f(x) = 0 at x = (-10, 1)
/// Bounds: x1 in [-15, -5], x2 in [-3, 3]
pub fn bukin_n6(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    100.0 * (x2 - 0.01*x1.powi(2)).abs().sqrt() + 0.01 * (x1 + 10.0).abs()
}

/// Schaffer N.2 function - multimodal, 2D only
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-100, 100]
pub fn schaffer_n2(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    0.5 + ((x1.powi(2) + x2.powi(2)).sin().powi(2) - 0.5) /
        (1.0 + 0.001*(x1.powi(2) + x2.powi(2))).powi(2)
}

/// Schaffer N.4 function - multimodal, 2D only
/// Global minimum: f(x) = 0.292579 at x = (0, ±1.25313) or (±1.25313, 0)
/// Bounds: x_i in [-100, 100]
pub fn schaffer_n4(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    0.5 + ((x1.powi(2) - x2.powi(2)).sin().powi(2) - 0.5) /
        (1.0 + 0.001*(x1.powi(2) + x2.powi(2))).powi(2)
}

/// Easom function - multimodal with very narrow global basin
/// Global minimum: f(x) = -1 at x = (π, π)
/// Bounds: x_i in [-100, 100]
pub fn easom(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -x1.cos() * x2.cos() *
        (-(x1 - std::f64::consts::PI).powi(2) - (x2 - std::f64::consts::PI).powi(2)).exp()
}

/// Keane's bump function objective (for constrained optimization)
/// Subject to constraints: x1*x2*x3*x4 >= 0.75 and sum(x_i) <= 7.5*n
/// Bounds: x_i in [0, 10]
pub fn keanes_bump_objective(x: &Array1<f64>) -> f64 {
    let sum_cos4: f64 = x.iter().map(|&xi| xi.cos().powi(4)).sum();
    let prod_cos2: f64 = x.iter().map(|&xi| xi.cos().powi(2)).product();
    let sum_i_xi2: f64 = x.iter().enumerate()
        .map(|(i, &xi)| (i + 1) as f64 * xi.powi(2))
        .sum();

    -(sum_cos4 - 2.0 * prod_cos2).abs() / sum_i_xi2.sqrt()
}

/// First constraint for Keane's bump function: x1*x2*x3*x4 >= 0.75
/// Returns violation amount (0 if satisfied, positive if violated)
pub fn keanes_bump_constraint1(x: &Array1<f64>) -> f64 {
    let product: f64 = x.iter().take(4).product();
    0.75 - product  // Constraint: product >= 0.75, so violation is 0.75 - product
}

/// Second constraint for Keane's bump function: sum(x_i) <= 7.5*n
/// Returns violation amount (0 if satisfied, positive if violated)
pub fn keanes_bump_constraint2(x: &Array1<f64>) -> f64 {
    let sum: f64 = x.iter().sum();
    let limit = 7.5 * x.len() as f64;
    sum - limit  // Constraint: sum <= limit, so violation is sum - limit
}

/// Branin function - multimodal, 2D only
/// Global minimum: f(x) = 0.397887 at x = (-π, 12.275), (π, 2.275), (9.42478, 2.475)
/// Bounds: x1 in [-5, 10], x2 in [0, 15]
pub fn branin(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let a = 1.0;
    let b = 5.1 / (4.0 * std::f64::consts::PI.powi(2));
    let c = 5.0 / std::f64::consts::PI;
    let r = 6.0;
    let s = 10.0;
    let t = 1.0 / (8.0 * std::f64::consts::PI);

    a * (x2 - b * x1.powi(2) + c * x1 - r).powi(2) + s * (1.0 - t) * x1.cos() + s
}

/// Rastrigin function - highly multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5.12, 5.12]
pub fn rastrigin(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum: f64 = x.iter()
        .map(|&xi| xi.powi(2) - 10.0 * (2.0 * std::f64::consts::PI * xi).cos())
        .sum();
    10.0 * n + sum
}

/// Cross-in-tray function - 2D multimodal function
/// Global minimum: f(x) = -2.06261 at x = (±1.34941, ±1.34941)
/// Bounds: x_i in [-10, 10]
pub fn cross_in_tray(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let exp_term = (100.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs();
    -0.0001 * ((x1 * x2).sin().abs() * exp_term.exp() + 1.0).powf(0.1)
}

/// Zakharov function - unimodal quadratic function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5, 10]
pub fn zakharov(x: &Array1<f64>) -> f64 {
    let sum1: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum2: f64 = x.iter().enumerate().map(|(i, &xi)| 0.5 * (i + 1) as f64 * xi).sum();
    sum1 + sum2.powi(2) + sum2.powi(4)
}

/// Zakharov function variant (2D specific)
pub fn zakharov2(x: &Array1<f64>) -> f64 {
    zakharov(x)
}

/// Three-hump camel function - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-5, 5]
pub fn three_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    2.0 * x1.powi(2) - 1.05 * x1.powi(4) + x1.powi(6) / 6.0 + x1 * x2 + x2.powi(2)
}

/// Schwefel function variant (different from the main schwefel)
pub fn schwefel2(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let sum: f64 = x.iter().enumerate()
        .map(|(i, &xi)| {
            let inner_sum: f64 = x.iter().take(i + 1).map(|&xj| xj).sum();
            inner_sum.powi(2)
        })
        .sum();
    sum
}

/// Bird function - 2D multimodal
/// Global minimum: f(x) = -106.764537 at x = (4.70104, 3.15294) and (-1.58214, -3.13024)
/// Bounds: x_i in [-2π, 2π]
pub fn bird(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.sin() * (x2 - 15.0).exp() + (x1 - x2.cos()).powi(2)
}

/// Holder table function - 2D multimodal
/// Global minimum: f(x) = -19.2085 at x = (±8.05502, ±9.66459)
/// Bounds: x_i in [-10, 10]
pub fn holder_table(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let exp_term = (1.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs();
    -(x1 * x2).sin().abs() * exp_term.exp()
}

/// McCormick function - 2D function
/// Global minimum: f(x) = -1.9133 at x = (-0.54719, -1.54719)
/// Bounds: x1 in [-1.5, 4], x2 in [-3, 4]
pub fn mccormick(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1 + x2).sin() + (x1 - x2).powi(2) - 1.5 * x1 + 2.5 * x2 + 1.0
}

/// Drop wave function - 2D multimodal
/// Global minimum: f(x) = -1.0 at x = (0, 0)
/// Bounds: x_i in [-5.12, 5.12]
pub fn drop_wave(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let numerator = 1.0 + (12.0 * (x1.powi(2) + x2.powi(2)).sqrt()).cos();
    let denominator = 0.5 * (x1.powi(2) + x2.powi(2)) + 2.0;
    -numerator / denominator
}

/// Styblinski-Tang function variant (2D specific)
/// Global minimum: f(x) = -78.332 for 2D at x = (-2.903534, -2.903534)
pub fn styblinski_tang2(x: &Array1<f64>) -> f64 {
    let sum: f64 = x.iter().map(|&xi| xi.powi(4) - 16.0 * xi.powi(2) + 5.0 * xi).sum();
    sum / 2.0
}

/// De Jong step function (variant)
pub fn de_jong_step2(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum()
}

/// De Jong F5 (Shekel's foxholes) function - 2D
pub fn dejong_f5_foxholes(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];

    // Shekel's foxholes a matrix (2x25)
    let a = [
        [-32, -16, 0, 16, 32, -32, -16, 0, 16, 32, -32, -16, 0, 16, 32, -32, -16, 0, 16, 32, -32, -16, 0, 16, 32],
        [-32, -32, -32, -32, -32, -16, -16, -16, -16, -16, 0, 0, 0, 0, 0, 16, 16, 16, 16, 16, 32, 32, 32, 32, 32]
    ];

    let mut sum = 0.0;
    for j in 0..25 {
        let mut inner_sum = 0.0;
        for i in 0..2 {
            let xi = if i == 0 { x1 } else { x2 };
            inner_sum += (xi - a[i][j] as f64).powi(6);
        }
        sum += 1.0 / (j as f64 + 1.0 + inner_sum);
    }
    1.0 / (0.002 + sum)
}

// Constrained optimization functions

/// Binh-Korn constraint 1: x1^2 + x2^2 <= 25
pub fn binh_korn_constraint1(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + x[1].powi(2) - 25.0
}

/// Binh-Korn constraint 2: (x1-8)^2 + (x2+3)^2 >= 7.7
pub fn binh_korn_constraint2(x: &Array1<f64>) -> f64 {
    7.7 - ((x[0] - 8.0).powi(2) + (x[1] + 3.0).powi(2))
}

/// Binh-Korn weighted objective function
pub fn binh_korn_weighted(x: &Array1<f64>) -> f64 {
    4.0 * x[0].powi(2) + 4.0 * x[1].powi(2)
}

/// Rosenbrock disk constraint: x^2 + y^2 <= 2
pub fn rosenbrock_disk_constraint(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + x[1].powi(2) - 2.0
}

/// Rosenbrock objective function (2D)
pub fn rosenbrock_objective(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    100.0 * (x2 - x1.powi(2)).powi(2) + (1.0 - x1).powi(2)
}

/// Mishra's Bird constraint: (x+5)^2 + (y+5)^2 < 25
pub fn mishras_bird_constraint(x: &Array1<f64>) -> f64 {
    (x[0] + 5.0).powi(2) + (x[1] + 5.0).powi(2) - 25.0
}

/// Mishra's Bird objective function
pub fn mishras_bird_objective(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let sin_term = ((x1 * x2).exp().cos() - (x1.powi(2) + x2.powi(2)).cos()).sin();
    sin_term.powi(2) + 0.01 * (x1 + x2)
}

/// Rosenbrock function - N-dimensional
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-2.048, 2.048]
pub fn rosenbrock(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    for i in 0..x.len()-1 {
        let xi = x[i];
        let xi_plus_1 = x[i+1];
        sum += 100.0 * (xi_plus_1 - xi.powi(2)).powi(2) + (1.0 - xi).powi(2);
    }
    sum
}

/// Ackley function - N-dimensional multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-32.768, 32.768]
pub fn ackley(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum_sq: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum_cos: f64 = x.iter().map(|&xi| (2.0 * std::f64::consts::PI * xi).cos()).sum();

    -20.0 * (-0.2 * (sum_sq / n).sqrt()).exp() - (sum_cos / n).exp() + 20.0 + std::f64::consts::E
}

/// Six-hump camel function - 2D multimodal
/// Global minimum: f(x) = -1.0316 at x = (0.0898, -0.7126) and (-0.0898, 0.7126)
/// Bounds: x1 in [-3, 3], x2 in [-2, 2]
pub fn six_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (4.0 - 2.1 * x1.powi(2) + x1.powi(4) / 3.0) * x1.powi(2) + x1 * x2 + (-4.0 + 4.0 * x2.powi(2)) * x2.powi(2)
}

/// Booth function - 2D unimodal
/// Global minimum: f(x) = 0 at x = (1, 3)
/// Bounds: x_i in [-10, 10]
pub fn booth(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1 + 2.0 * x2 - 7.0).powi(2) + (2.0 * x1 + x2 - 5.0).powi(2)
}

/// Matyas function - 2D unimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-10, 10]
pub fn matyas(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    0.26 * (x1.powi(2) + x2.powi(2)) - 0.48 * x1 * x2
}

/// Beale function - 2D multimodal
/// Global minimum: f(x) = 0 at x = (3, 0.5)
/// Bounds: x_i in [-4.5, 4.5]
pub fn beale(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (1.5 - x1 + x1 * x2).powi(2) + (2.25 - x1 + x1 * x2.powi(2)).powi(2) + (2.625 - x1 + x1 * x2.powi(3)).powi(2)
}

/// Himmelblau function - 2D multimodal
/// Global minima: f(x) = 0 at x = (3, 2), (-2.805118, 3.131312), (-3.779310, -3.283186), (3.584428, -1.848126)
/// Bounds: x_i in [-5, 5]
pub fn himmelblau(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1.powi(2) + x2 - 11.0).powi(2) + (x1 + x2.powi(2) - 7.0).powi(2)
}

/// Bohachevsky function 1 - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-100, 100]
pub fn bohachevsky1(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.powi(2) + 2.0 * x2.powi(2) - 0.3 * (3.0 * std::f64::consts::PI * x1).cos() - 0.4 * (4.0 * std::f64::consts::PI * x2).cos() + 0.7
}

/// Bohachevsky function 2 - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-100, 100]
pub fn bohachevsky2(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.powi(2) + 2.0 * x2.powi(2) - 0.3 * (3.0 * std::f64::consts::PI * x1).cos() * (4.0 * std::f64::consts::PI * x2).cos() + 0.3
}

/// Bohachevsky function 3 - 2D multimodal
/// Global minimum: f(x) = 0 at x = (0, 0)
/// Bounds: x_i in [-100, 100]
pub fn bohachevsky3(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.powi(2) + 2.0 * x2.powi(2) - 0.3 * (3.0 * std::f64::consts::PI * x1 + 4.0 * std::f64::consts::PI * x2).cos() + 0.3
}

/// Michalewicz function - N-dimensional multimodal
/// Global minimum depends on dimension (e.g., -1.8013 for 2D, -9.66 for 10D)
/// Bounds: x_i in [0, π]
pub fn michalewicz(x: &Array1<f64>) -> f64 {
    let m = 10.0; // Steepness parameter
    -x.iter().enumerate().map(|(i, &xi)| {
        xi.sin() * ((i as f64 + 1.0) * xi.powi(2) / std::f64::consts::PI).sin().powf(2.0 * m)
    }).sum::<f64>()
}

