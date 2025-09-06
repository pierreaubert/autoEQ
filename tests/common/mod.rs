//! Shared test functions and utilities for differential evolution optimization tests
//! 
//! This module contains all the benchmark functions used to test the DE optimizer,
//! organized by category and following the Wikipedia "Test functions for optimization" page.

use std::sync::Arc;
use ndarray::Array1;
use autoeq::optde::*;

// Basic 2D test functions from the original implementation
pub fn sphere(x: &Array1<f64>) -> f64 {
    x.iter().map(|&v| v * v).sum()
}

pub fn rosenbrock2(x: &Array1<f64>) -> f64 {
    let a = 1.0;
    let b = 100.0;
    let x1 = x[0];
    let x2 = x[1];
    (a - x1).powi(2) + b * (x2 - x1.powi(2)).powi(2)
}

pub fn rastrigin2(x: &Array1<f64>) -> f64 {
    let a = 10.0;
    let n = 2.0;
    a * n
        + x.iter()
            .map(|&xi| xi * xi - a * (2.0 * std::f64::consts::PI * xi).cos())
            .sum::<f64>()
}

pub fn ackley2(x: &Array1<f64>) -> f64 {
    let x0 = x[0];
    let x1 = x[1];
    let s = 0.5 * (x0 * x0 + x1 * x1);
    let c = 0.5
        * ((2.0 * std::f64::consts::PI * x0).cos() + (2.0 * std::f64::consts::PI * x1).cos());
    -20.0 * (-0.2 * s.sqrt()).exp() - c.exp() + 20.0 + std::f64::consts::E
}

pub fn booth(x: &Array1<f64>) -> f64 {
    (x[0] + 2.0 * x[1] - 7.0).powi(2) + (2.0 * x[0] + x[1] - 5.0).powi(2)
}

pub fn matyas(x: &Array1<f64>) -> f64 {
    0.26 * (x[0] * x[0] + x[1] * x[1]) - 0.48 * x[0] * x[1]
}

pub fn beale(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (1.5 - x1 + x1 * x2).powi(2)
        + (2.25 - x1 + x1 * x2 * x2).powi(2)
        + (2.625 - x1 + x1 * x2 * x2 * x2).powi(2)
}

pub fn himmelblau(x: &Array1<f64>) -> f64 {
    (x[0] * x[0] + x[1] - 11.0).powi(2) + (x[0] + x[1] * x[1] - 7.0).powi(2)
}

// Additional 2D functions
pub fn goldstein_price(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let a = 1.0
        + (x1 + x2 + 1.0).powi(2)
            * (19.0 - 14.0 * x1 + 3.0 * x1.powi(2) - 14.0 * x2
                + 6.0 * x1 * x2
                + 3.0 * x2.powi(2));
    let b = 30.0
        + (2.0 * x1 - 3.0 * x2).powi(2)
            * (18.0 - 32.0 * x1 + 12.0 * x1.powi(2) + 48.0 * x2 - 36.0 * x1 * x2
                + 27.0 * x2.powi(2));
    a * b
}

pub fn three_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    2.0 * x1 * x1 - 1.05 * x1.powi(4) + x1.powi(6) / 6.0 + x1 * x2 + x2 * x2
}

pub fn six_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (4.0 - 2.1 * x1 * x1 + x1.powi(4) / 3.0) * x1 * x1
        + x1 * x2
        + (-4.0 + 4.0 * x2 * x2) * x2 * x2
}

pub fn easom(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -((x1 - std::f64::consts::PI).cos() * (x2 - std::f64::consts::PI).cos())
        * (-((x1 - std::f64::consts::PI).powi(2) + (x2 - std::f64::consts::PI).powi(2))).exp()
}

pub fn mccormick(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1 + x2).sin() + (x1 - x2 * x2).powi(2) - 1.5 * x1 + 2.5 * x2 + 1.0
}

pub fn levi13(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (3.0 * std::f64::consts::PI * x1).sin().powi(2)
        + (x1 - 1.0).powi(2) * (1.0 + (3.0 * std::f64::consts::PI * x2).sin().powi(2))
        + (x2 - 1.0).powi(2) * (1.0 + (2.0 * std::f64::consts::PI * x2).sin().powi(2))
}

pub fn styblinski_tang2(x: &Array1<f64>) -> f64 {
    x.iter()
        .map(|&xi| xi.powi(4) - 16.0 * xi * xi + 5.0 * xi)
        .sum::<f64>()
        / 2.0
}

pub fn griewank2(x: &Array1<f64>) -> f64 {
    let sum = x.iter().map(|&xi| xi * xi).sum::<f64>() / 4000.0;
    let prod = (x[0] / 1.0_f64.sqrt()).cos() * (x[1] / 2.0_f64.sqrt()).cos();
    sum - prod + 1.0
}

pub fn zakharov2(x: &Array1<f64>) -> f64 {
    let sum1 = x[0] * x[0] + x[1] * x[1];
    let sum2 = 0.5 * x[0] + 1.0 * x[1];
    sum1 + sum2 * sum2 + sum2.powi(4)
}

pub fn schwefel2(x: &Array1<f64>) -> f64 {
    let a = 418.9829 * 2.0;
    a - (x[0] * (x[0].abs().sqrt()).sin() + x[1] * (x[1].abs().sqrt()).sin())
}

pub fn de_jong_step2(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum::<f64>()
}

pub fn schaffer_n2(x: &Array1<f64>) -> f64 {
    let num = (x[0] * x[0] - x[1] * x[1]).sin().powi(2) - 0.5;
    let den = (1.0 + 0.001 * (x[0] * x[0] + x[1] * x[1])).powi(2);
    0.5 + num / den
}

pub fn schaffer_n4(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let num = ((x1 * x1 - x2 * x2).abs().sin().cos()).powi(2) - 0.5;
    let den = (1.0 + 0.001 * (x1 * x1 + x2 * x2)).powi(2);
    0.5 + num / den
}

pub fn bukin_n6(x: &Array1<f64>) -> f64 {
    let term1 = 100.0 * (x[1] - 0.01 * x[0] * x[0]).abs().sqrt();
    let term2 = (1.0 + 0.01 * (x[0] + 10.0)).abs();
    term1 + term2
}

pub fn eggholder(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -(x2 + 47.0) * (((x2 + x1 / 2.0 + 47.0).abs()).sqrt()).sin()
        - x1 * (((x1 - (x2 + 47.0)).abs()).sqrt()).sin()
}

pub fn branin(x: &Array1<f64>) -> f64 {
    let a = 1.0;
    let b = 5.1 / (4.0 * std::f64::consts::PI.powi(2));
    let c = 5.0 / std::f64::consts::PI;
    let r = 6.0;
    let s = 10.0;
    let t = 1.0 / (8.0 * std::f64::consts::PI);
    let x1 = x[0];
    let x2 = x[1];
    a * (x2 - b * x1 * x1 + c * x1 - r).powi(2) + s * (1.0 - t) * x1.cos() + s
}

pub fn bohachevsky1(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1 * x1 + 2.0 * x2 * x2
        - 0.3 * (3.0 * std::f64::consts::PI * x1).cos()
        - 0.4 * (4.0 * std::f64::consts::PI * x2).cos()
        + 0.7
}

pub fn bohachevsky2(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1 * x1 + 2.0 * x2 * x2
        - 0.3
            * (3.0 * std::f64::consts::PI * x1).cos()
            * (4.0 * std::f64::consts::PI * x2).cos()
        + 0.3
}

pub fn bohachevsky3(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1 * x1 + 2.0 * x2 * x2 - 0.3 * (3.0 * std::f64::consts::PI * x1).cos()
        + 0.4 * (4.0 * std::f64::consts::PI * x2).cos()
        - 0.7
}

pub fn dejong_f5_foxholes(x: &Array1<f64>) -> f64 {
    let a: [[f64; 5]; 2] = [
        [-32.0, -16.0, 0.0, 16.0, 32.0],
        [-32.0, -16.0, 0.0, 16.0, 32.0],
    ];
    let mut sum = 0.0;
    for i in 0..25 {
        let ii = i / 5;
        let jj = i % 5;
        let xi = a[0][jj];
        let yi = a[1][ii];
        let t = (i as f64 + 1.0) + (x[0] - xi).powi(6) + (x[1] - yi).powi(6);
        sum += 1.0 / t;
    }
    1.0 / (0.002 + sum)
}

// N-dimensional test functions
pub fn rastrigin(x: &Array1<f64>) -> f64 {
    let a = 10.0;
    let n = x.len() as f64;
    a * n + x.iter()
        .map(|&xi| xi * xi - a * (2.0 * std::f64::consts::PI * xi).cos())
        .sum::<f64>()
}

pub fn ackley(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum_sq = x.iter().map(|&xi| xi * xi).sum::<f64>() / n;
    let sum_cos = x.iter().map(|&xi| (2.0 * std::f64::consts::PI * xi).cos()).sum::<f64>() / n;
    -20.0 * (-0.2 * sum_sq.sqrt()).exp() - sum_cos.exp() + 20.0 + std::f64::consts::E
}

pub fn griewank(x: &Array1<f64>) -> f64 {
    let sum = x.iter().map(|&xi| xi * xi).sum::<f64>() / 4000.0;
    let prod: f64 = x.iter()
        .enumerate()
        .map(|(i, &xi)| (xi / ((i + 1) as f64).sqrt()).cos())
        .product();
    sum - prod + 1.0
}

pub fn schwefel(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let sum = x.iter().map(|&xi| xi * (xi.abs().sqrt()).sin()).sum::<f64>();
    418.9829 * n as f64 - sum
}

pub fn rosenbrock(x: &Array1<f64>) -> f64 {
    x.windows(2)
        .into_iter()
        .map(|w| {
            let a = 1.0;
            let b = 100.0;
            (a - w[0]).powi(2) + b * (w[1] - w[0].powi(2)).powi(2)
        })
        .sum::<f64>()
}

pub fn dixons_price(x: &Array1<f64>) -> f64 {
    let first_term = (x[0] - 1.0).powi(2);
    let sum_term: f64 = x.iter().skip(1).enumerate()
        .map(|(i, &xi)| (i + 2) as f64 * (2.0 * xi.powi(2) - x[i]).powi(2))
        .sum();
    first_term + sum_term
}

pub fn zakharov(x: &Array1<f64>) -> f64 {
    let sum1 = x.iter().map(|&xi| xi * xi).sum::<f64>();
    let sum2 = x.iter().enumerate().map(|(i, &xi)| 0.5 * (i + 1) as f64 * xi).sum::<f64>();
    sum1 + sum2.powi(2) + sum2.powi(4)
}

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

// Constrained optimization test functions - objective functions only
pub fn rosenbrock_objective(x: &Array1<f64>) -> f64 {
    (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0].powi(2)).powi(2)
}

// Constraint function for Rosenbrock constrained to disk: x^2 + y^2 <= 2
pub fn rosenbrock_disk_constraint(x: &Array1<f64>) -> f64 {
    x[0].powi(2) + x[1].powi(2) - 2.0  // <= 0 for feasibility
}

pub fn mishras_bird_objective(x: &Array1<f64>) -> f64 {
    (x[1]).sin() * ((1.0 - x[0].cos()).powi(2)).exp()
        + (x[0]).cos() * ((1.0 - x[1].sin()).powi(2)).exp()
        + (x[0] - x[1]).powi(2)
}

// Constraint function for Mishra's Bird: (x+5)^2 + (y+5)^2 < 25
pub fn mishras_bird_constraint(x: &Array1<f64>) -> f64 {
    (x[0] + 5.0).powi(2) + (x[1] + 5.0).powi(2) - 25.0  // <= 0 for feasibility
}

pub fn keanes_bump_objective(x: &Array1<f64>) -> f64 {
    let numerator = x.iter().map(|&xi| xi.cos().powi(4)).sum::<f64>()
        - 2.0 * x.iter().map(|&xi| xi.cos().powi(2)).product::<f64>();
    let denominator = x.iter().enumerate()
        .map(|(i, &xi)| (i + 1) as f64 * xi.powi(2))
        .sum::<f64>()
        .sqrt();
    
    -(numerator / denominator).abs()
}

// Constraint functions for Keane's bump
pub fn keanes_bump_constraint1(x: &Array1<f64>) -> f64 {
    0.75 - x.iter().product::<f64>()  // product > 0.75 => constraint <= 0
}

pub fn keanes_bump_constraint2(x: &Array1<f64>) -> f64 {
    let m = x.len();
    x.iter().sum::<f64>() - 7.5 * m as f64  // sum < 7.5*m => constraint <= 0
}

// Binh and Korn function objectives
pub fn binh_korn_f1(x: &Array1<f64>) -> f64 {
    4.0 * x[0] * x[0] + 4.0 * x[1] * x[1]
}

pub fn binh_korn_f2(x: &Array1<f64>) -> f64 {
    (x[0] - 5.0).powi(2) + (x[1] - 5.0).powi(2)
}

// For single objective, use weighted sum
pub fn binh_korn_weighted(x: &Array1<f64>) -> f64 {
    0.5 * binh_korn_f1(x) + 0.5 * binh_korn_f2(x)
}

// Binh-Korn constraint functions: g1: (x-5)^2 + y^2 <= 25, g2: (x-8)^2 + (y+3)^2 >= 7.7
pub fn binh_korn_constraint1(x: &Array1<f64>) -> f64 {
    (x[0] - 5.0).powi(2) + x[1].powi(2) - 25.0  // <= 0
}

pub fn binh_korn_constraint2(x: &Array1<f64>) -> f64 {
    7.7 - ((x[0] - 8.0).powi(2) + (x[1] + 3.0).powi(2))  // >= 7.7 => 7.7 - value <= 0
}

// Additional functions from landscapes repository

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

/// Bird function - multimodal with multiple global minima
/// Global minimum: f(x) = -106.76453 at (4.70104, 3.15294) and (-1.58214, -3.13024)
/// Bounds: x_i in [-2*pi, 2*pi]
pub fn bird(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    x1.sin() * ((1.0 - x2.cos()).powi(2)).exp()
        + x2.cos() * ((1.0 - x1.sin()).powi(2)).exp()
        + (x1 - x2).powi(2)
}

/// Cross-in-tray function - multimodal with 4 global minima
/// Global minimum: f(x) = -2.06261 at (±1.34941, ±1.34941)
/// Bounds: x_i in [-10, 10]
pub fn cross_in_tray(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let term = (x1.sin() * x2.sin() * (100.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs().exp()).abs();
    -0.0001 * (term + 1.0).powf(0.1)
}

/// Drop-wave function - multimodal with single global minimum
/// Global minimum: f(x=0, y=0) = -1
/// Bounds: x_i in [-5.12, 5.12]
pub fn drop_wave(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    let norm_sq = x1.powi(2) + x2.powi(2);
    -(1.0 + (12.0 * norm_sq.sqrt()).cos()) / (0.5 * norm_sq + 2.0)
}

/// Holder table function - multimodal with 4 global minima
/// Global minimum: f(x) = -19.2085 at (±8.05502, ±9.66459)
/// Bounds: x_i in [-10, 10]
pub fn holder_table(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -(x1.sin() * x2.cos() * (1.0 - (x1.powi(2) + x2.powi(2)).sqrt() / std::f64::consts::PI).abs().exp()).abs()
}

/// Michalewicz function - multimodal with d! local minima
/// Global minimum for 2D: f(x)=-1.8013 at x*=(2.20,1.57)
/// Bounds: x_i in [0, pi]
pub fn michalewicz(x: &Array1<f64>) -> f64 {
    let m = 10.0; // steepness parameter
    -x.iter().enumerate()
        .map(|(i, &xi)| xi.sin() * ((i as f64 + 1.0) * xi.powi(2) / std::f64::consts::PI).sin().powf(2.0 * m))
        .sum::<f64>()
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

/// Rotated hyper-ellipsoid function - unimodal, non-separable
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-65.536, 65.536]
pub fn rotated_hyper_ellipsoid(x: &Array1<f64>) -> f64 {
    (0..x.len())
        .map(|i| x.iter().take(i + 1).map(|&xi| xi.powi(2)).sum::<f64>())
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
