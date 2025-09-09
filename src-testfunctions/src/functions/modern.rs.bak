//! Modern benchmark functions
//!
//! These functions are from recent optimization competitions (CEC, etc.) and
//! research papers, representing the current state-of-the-art in test functions.

use ndarray::Array1;

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

/// Gramacy & Lee (2012) Function - 1D test function with challenging properties  
/// f(x) = sin(10*pi*x) / (2*x) + (x-1)^4
/// Global minimum: f(x) ≈ -0.869011134989500 at x ≈ 0.548563444114526
/// Bounds: x in [0.5, 2.5]
pub fn gramacy_lee_2012(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let pi = std::f64::consts::PI;
    (10.0 * pi * x1).sin() / (2.0 * x1) + (x1 - 1.0).powi(4)
}

/// Gramacy & Lee (2012) Function - Alternative test function with noise
/// This is a variant used specifically for Gaussian process testing
/// Global minimum varies due to noise component
/// Bounds: x in [0, 1]
pub fn gramacy_lee_function(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    (x1 * (x1 - 0.5) * (x1 - 1.0)).exp() + x1.powi(2) / 10.0
}

/// Perm Function 0, d, β - bowl-shaped function
/// f(x) = ∑_{i=1}^d [∑_{j=1}^d (j+β)(x_j^i - (1/j)^i)]^2
/// Global minimum: f(x) = 0 at x = (1, 1/2, 1/3, ..., 1/d)
/// Bounds: x_i in [-1, 1]
pub fn perm_0_d_beta(x: &Array1<f64>) -> f64 {
    let d = x.len();
    let beta = 0.5; // Parameter β (smaller value for numerical stability)
    
    let mut outer_sum = 0.0;
    for i in 1..=d {
        let mut inner_sum = 0.0;
        for j in 1..=d {
            let xj = x[j - 1];
            let target = (1.0 / j as f64).powf(i as f64);
            inner_sum += (j as f64 + beta) * (xj.powf(i as f64) - target);
        }
        outer_sum += inner_sum.powi(2);
    }
    outer_sum
}

/// Power Sum Function - constrained optimization problem
/// Global minimum: complex, depends on parameters b
/// Bounds: x_i in [0, d] where d is dimension
pub fn power_sum(x: &Array1<f64>) -> f64 {
    let b = [8.0, 18.0, 44.0, 114.0]; // Parameters for up to 4D
    let d = x.len().min(4);
    
    let mut sum = 0.0;
    for i in 1..=d {
        let power_sum: f64 = x.iter().take(d).map(|&xj| xj.powf(i as f64)).sum();
        sum += (power_sum - b[i - 1]).powi(2);
    }
    sum
}

/// Forrester et al. (2008) Function - 1D function for metamodeling
/// Global minimum: f(x) ≈ -6.02074 at x ≈ 0.757249
/// Bounds: x in [0, 1]
pub fn forrester_2008(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    (6.0 * x1 - 2.0).powi(2) * (12.0 * x1 - 4.0).sin()
}

/// Perm Function d, β - another variant of the perm function
/// Global minimum: f(x) = 0 at x = (1/1, 1/2, 1/3, ..., 1/d)
/// Bounds: x_i in [-1, 1]
pub fn perm_d_beta(x: &Array1<f64>) -> f64 {
    let d = x.len();
    let beta = 0.5; // Parameter β
    
    let mut outer_sum = 0.0;
    for i in 1..=d {
        let mut inner_sum = 0.0;
        for j in 1..=d {
            let xj = x[j - 1];
            inner_sum += ((j as f64).powf(i as f64) + beta) * (xj.powf(i as f64) - (1.0 / j as f64).powf(i as f64));
        }
        outer_sum += inner_sum.powi(2);
    }
    outer_sum
}

/// Shekel Function - multimodal function with m local minima
/// Global minimum depends on m parameter
/// Bounds: x_i in [0, 10]
pub fn shekel(x: &Array1<f64>) -> f64 {
    let m = 10; // Number of local minima
    let a = [
        [4.0, 4.0, 4.0, 4.0],
        [1.0, 1.0, 1.0, 1.0], 
        [8.0, 8.0, 8.0, 8.0],
        [6.0, 6.0, 6.0, 6.0],
        [3.0, 7.0, 3.0, 7.0],
        [2.0, 9.0, 2.0, 9.0],
        [5.0, 5.0, 3.0, 3.0],
        [8.0, 1.0, 8.0, 1.0],
        [6.0, 2.0, 6.0, 2.0],
        [7.0, 3.6, 7.0, 3.6],
    ];
    let c = [0.1, 0.2, 0.2, 0.4, 0.4, 0.6, 0.3, 0.7, 0.5, 0.5];
    
    let mut sum = 0.0;
    for i in 0..m.min(10) {
        let mut inner_sum = 0.0;
        for j in 0..4.min(x.len()) {
            inner_sum += (x[j] - a[i][j]).powi(2);
        }
        sum += 1.0 / (inner_sum + c[i]);
    }
    -sum
}

/// Xin-She Yang N.1 function - newer benchmark function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5, 5]
pub fn xin_she_yang_n1(x: &Array1<f64>) -> f64 {
    let sum_abs: f64 = x.iter().map(|&xi| xi.abs()).sum();
    let sum_sin_sq: f64 = x.iter().map(|&xi| xi.powi(2).sin()).sum();
    sum_abs * (-sum_sin_sq).exp()
}

/// Xin-She Yang N.2 function - newer benchmark function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-2π, 2π]
pub fn xin_she_yang_n2(x: &Array1<f64>) -> f64 {
    use std::f64::consts::PI;
    let sum_abs: f64 = x.iter().map(|&xi| xi.abs()).sum();
    let exp_sum_sin_sq: f64 = (-x.iter().map(|&xi| xi.powi(2).sin()).sum::<f64>()).exp();
    sum_abs * exp_sum_sin_sq
}

/// Xin-She Yang N.3 function - multimodal with parameter m
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-20, 20]
pub fn xin_she_yang_n3(x: &Array1<f64>) -> f64 {
    let m = 5.0; // Parameter
    let beta = 15.0; // Parameter
    let sum_pow: f64 = x.iter().map(|&xi| xi.abs().powf(m)).sum();
    let prod_cos_sq: f64 = x.iter().map(|&xi| (beta * xi).cos().powi(2)).product();
    -(-sum_pow).exp() * prod_cos_sq
}

/// Xin-She Yang N.4 function - challenging multimodal
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn xin_she_yang_n4(x: &Array1<f64>) -> f64 {
    let sum_sin_sq: f64 = x.iter().map(|&xi| xi.powi(2).sin()).sum();
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    (sum_sin_sq - (-sum_squares).exp()) * (-sum_squares.sin().powi(2)).exp()
}

/// Langermann function - complex multimodal with parameters
/// Global minimum: f(x) ≈ -5.1621 at complex optimum
/// Bounds: x_i in [0, 10]
pub fn langermann(x: &Array1<f64>) -> f64 {
    // Langermann function parameters (for 2D)
    let a = [[3.0, 5.0], [5.0, 2.0], [2.0, 1.0], [1.0, 4.0], [7.0, 9.0]];
    let c = [1.0, 2.0, 5.0, 2.0, 3.0];
    
    let mut sum = 0.0;
    for i in 0..5 {
        let mut inner_sum = 0.0;
        for j in 0..2.min(x.len()) {
            inner_sum += (x[j] - a[i][j]).powi(2);
        }
        sum += c[i] * (-inner_sum / std::f64::consts::PI).exp() * (std::f64::consts::PI * inner_sum).cos();
    }
    sum
}

/// Qing function - separable multimodal function
/// Global minimum: f(x) = 0 at x = (±√i, ±√2, ..., ±√n)
/// Bounds: x_i in [-500, 500]
pub fn qing(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| (xi.powi(2) - (i + 1) as f64).powi(2))
        .sum()
}

/// Whitley function - challenging multimodal function
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10.24, 10.24]
pub fn whitley(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum = 0.0;
    
    for i in 0..n {
        for j in 0..n {
            let xi = x[i];
            let xj = x[j];
            let term = 100.0 * (xi.powi(2) - xj).powi(2) + (1.0 - xj).powi(2);
            sum += term.powi(2) / 4000.0 - term.cos() + 1.0;
        }
    }
    sum
}

/// HappyCat function - recent CEC benchmark function
/// Global minimum: f(x) = 0 at x = (-1, -1, ..., -1)
/// Bounds: x_i in [-2, 2]
pub fn happycat(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let norm_sq: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum_x: f64 = x.iter().sum();
    
    ((norm_sq - n).abs()).powf(0.25) + (0.5 * norm_sq + sum_x) / n + 0.5
}

/// Happy Cat function - recent benchmark with interesting landscape
/// Global minimum: f(x) = 0 at x = (±1, ±1, ..., ±1)
/// Bounds: x_i in [-2, 2]
pub fn happy_cat(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum_x: f64 = x.iter().sum();
    
    ((sum_squares - n).powi(2)).powf(0.25) + (0.5 * sum_squares + sum_x) / n + 0.5
}

/// Pinter function - challenging multimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn pinter(x: &Array1<f64>) -> f64 {
    let n = x.len();
    let mut sum1 = 0.0;
    let mut sum2 = 0.0;
    let mut sum3 = 0.0;
    
    for i in 0..n {
        let ii = (i + 1) as f64;
        let xi = x[i];
        let x_prev = if i == 0 { x[n - 1] } else { x[i - 1] };
        let x_next = if i == n - 1 { x[0] } else { x[i + 1] };
        
        let ai = x_prev * xi.sin() + (x_next - xi).sin();
        let bi = x_prev.powi(2) - 2.0 * xi + 3.0 * x_next - (1.0 + xi).cos() + 1.0;
        
        sum1 += ii * xi.powi(2);
        sum2 += 20.0 * ii * ai.powi(2).sin();
        sum3 += ii * (1.0 + ii).ln() * bi.powi(2);
    }
    
    sum1 + sum2 + sum3
}

/// Vincent function - high-dimensional multimodal
/// Global minimum: f(x) = -N at x = (7.70628, 7.70628, ..., 7.70628)
/// Bounds: x_i in [0.25, 10]
pub fn vincent(x: &Array1<f64>) -> f64 {
    -x.iter().map(|&xi| (10.0 * xi).sin()).sum::<f64>()
}

/// Ackley N.3 function - variant of Ackley function
/// Global minimum: f(x) ≈ -195.6 at complex optimum
/// Bounds: x_i in [-32, 32]
pub fn ackley_n3(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    -200.0 * (-0.02 * (x1.powi(2) + x2.powi(2)).sqrt()).exp() 
        * (2.0 * std::f64::consts::PI * x1).cos()
        * (2.0 * std::f64::consts::PI * x2).cos()
        + 5.0 * (3.0 * (x1 + x2)).exp()
}
