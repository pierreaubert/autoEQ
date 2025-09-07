//! Constrained optimization test functions
//!
//! These functions include various constraints (equality, inequality) and are used
//! to test constrained optimization algorithms.

use ndarray::Array1;

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

/// Rosenbrock disk constraint: x^2 + y^2 <= 2
pub fn rosenbrock_disk_constraint(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + x[1].powi(2) - 2.0
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
