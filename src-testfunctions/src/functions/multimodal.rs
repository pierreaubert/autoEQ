//! Multimodal test functions
//!
//! These functions have multiple local minima and are used to test the global
//! search capabilities and exploration of optimization algorithms.

use ndarray::Array1;
use std::collections::HashMap;

/// Step function - discontinuous, multimodal
/// Global minimum: f(x) = 0 at x = (0.5, 0.5, ..., 0.5)
/// Bounds: x_i in [-100, 100]
pub fn step(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| (xi + 0.5).floor().powi(2)).sum::<f64>()
}

/// Salomon function - multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn salomon(x: &Array1<f64>) -> f64 {
    let norm = x.iter().map(|&xi| xi.powi(2)).sum::<f64>().sqrt();
    1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm
}

/// Salomon function (corrected implementation)
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn salomon_corrected(x: &Array1<f64>) -> f64 {
    let norm = x.iter().map(|&xi| xi.powi(2)).sum::<f64>().sqrt();
    if norm == 0.0 {
        0.0
    } else {
        1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm
    }
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

/// Ackley function - N-dimensional multimodal
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-32.768, 32.768]
pub fn ackley(x: &Array1<f64>) -> f64 {
    let n = x.len() as f64;
    let sum_sq: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    let sum_cos: f64 = x.iter().map(|&xi| (2.0 * std::f64::consts::PI * xi).cos()).sum();

    -20.0 * (-0.2 * (sum_sq / n).sqrt()).exp() - (sum_cos / n).exp() + 20.0 + std::f64::consts::E
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

/// Alpine N.1 function - multimodal with many local minima
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn alpine_n1(x: &Array1<f64>) -> f64 {
    x.iter()
        .map(|&xi| (xi * xi.sin() + 0.1 * xi).abs())
        .sum()
}

/// Alpine N.2 function - multimodal with single global minimum
/// Global minimum: f(x) ≈ -2.808^N at x = (2.808, 2.808, ..., 2.808)
/// Bounds: x_i in [0, 10]
pub fn alpine_n2(x: &Array1<f64>) -> f64 {
    -x.iter()
        .map(|&xi| xi.sqrt() * xi.sin())
        .product::<f64>()
}

/// Hartman 3D function - 3D multimodal with 4 local minima
/// Global minimum: f(x) = -3.86278 at x = (0.114614, 0.555649, 0.852547)
/// Bounds: x_i in [0, 1]
pub fn hartman_3d(x: &Array1<f64>) -> f64 {
    let a = [
        [3.0, 10.0, 30.0],
        [0.1, 10.0, 35.0],
        [3.0, 10.0, 30.0],
        [0.1, 10.0, 35.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.3689, 0.1170, 0.2673],
        [0.4699, 0.4387, 0.7470],
        [0.1091, 0.8732, 0.5547],
        [0.03815, 0.5743, 0.8828],
    ];

    -c.iter().enumerate()
        .map(|(i, &ci)| {
            let inner_sum = a[i].iter().zip(p[i].iter()).enumerate()
                .map(|(j, (&aij, &pij))| aij * (x[j] - pij).powi(2))
                .sum::<f64>();
            ci * (-inner_sum).exp()
        })
        .sum::<f64>()
}

/// Hartman 4-D function - 4D multimodal with 4 local minima
/// Global minimum: f(x) ≈ -3.72983 at x ≈ [0.1873, 0.1936, 0.5576, 0.2647]
/// Bounds: x_i in [0, 1]
/// Reference: Hartman, J.K. (1973). Some experiments in global optimization
pub fn hartman_4d(x: &Array1<f64>) -> f64 {
    // Original Hartmann 4-D parameters from literature
    let a = [
        [10.0, 3.0, 17.0, 3.5],
        [0.05, 10.0, 17.0, 0.1],
        [3.0, 3.5, 1.7, 10.0],
        [17.0, 8.0, 0.05, 10.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.1312, 0.1696, 0.5569, 0.0124],
        [0.2329, 0.4135, 0.8307, 0.3736],
        [0.2348, 0.1451, 0.3522, 0.2883],
        [0.4047, 0.8828, 0.8732, 0.5743],
    ];

    -c.iter().enumerate()
        .map(|(i, &ci)| {
            let inner_sum = a[i].iter().zip(p[i].iter()).enumerate()
                .map(|(j, (&aij, &pij))| aij * (x[j] - pij).powi(2))
                .sum::<f64>();
            ci * (-inner_sum).exp()
        })
        .sum::<f64>()
}

/// Hartmann 6-D function - 6D multimodal with 4 local minima
/// Global minimum: f(x) = -3.32237 at complex optimum
/// Bounds: x_i in [0, 1]
pub fn hartman_6d(x: &Array1<f64>) -> f64 {
    let a = [
        [10.0, 3.0, 17.0, 3.5, 1.7, 8.0],
        [0.05, 10.0, 17.0, 0.1, 8.0, 14.0],
        [3.0, 3.5, 1.7, 10.0, 17.0, 8.0],
        [17.0, 8.0, 0.05, 10.0, 0.1, 14.0],
    ];
    let c = [1.0, 1.2, 3.0, 3.2];
    let p = [
        [0.1312, 0.1696, 0.5569, 0.0124, 0.8283, 0.5886],
        [0.2329, 0.4135, 0.8307, 0.3736, 0.1004, 0.9991],
        [0.2348, 0.1451, 0.3522, 0.2883, 0.3047, 0.6650],
        [0.4047, 0.8828, 0.8732, 0.5743, 0.1091, 0.0381],
    ];

    -c.iter().enumerate()
        .map(|(i, &ci)| {
            let inner_sum = a[i].iter().zip(p[i].iter()).enumerate()
                .map(|(j, (&aij, &pij))| aij * (x[j] - pij).powi(2))
                .sum::<f64>();
            ci * (-inner_sum).exp()
        })
        .sum::<f64>()
}

/// Shubert function - highly multimodal with many global minima
/// Global minimum: f(x) = -186.7309 (2D), multiple locations
/// Bounds: x_i in [-10, 10]
pub fn shubert(x: &Array1<f64>) -> f64 {
    x.iter()
        .map(|&xi| {
            (1..=5).map(|i| {
                let i_f64 = i as f64;
                i_f64 * ((i_f64 + 1.0) * xi + i_f64).cos()
            }).sum::<f64>()
        })
        .product()
}

/// Levy function - multimodal function (generalized version)
/// Global minimum: f(x) = 0 at x = (1, 1, ..., 1)
/// Bounds: x_i in [-10, 10]
pub fn levy(x: &Array1<f64>) -> f64 {
    use std::f64::consts::PI;
    
    let w: Vec<f64> = x.iter().map(|&xi| 1.0 + (xi - 1.0) / 4.0).collect();
    
    let first_term = (PI * w[0]).sin().powi(2);
    
    let middle_sum: f64 = w.iter().take(w.len() - 1).map(|&wi| {
        (wi - 1.0).powi(2) * (1.0 + 10.0 * (PI * wi + 1.0).sin().powi(2))
    }).sum();
    
    let last_term = {
        let wn = w[w.len() - 1];
        (wn - 1.0).powi(2) * (1.0 + (2.0 * PI * wn).sin().powi(2))
    };
    
    first_term + middle_sum + last_term
}

/// Periodic function - multimodal with periodic landscape
/// Global minimum: f(x) = 0.9 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn periodic(x: &Array1<f64>) -> f64 {
    let sum_sin_squared: f64 = x.iter().map(|&xi| xi.sin().powi(2)).sum();
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    
    1.0 + sum_sin_squared - 0.1 * (-sum_squares).exp()
}

/// Katsuura function - fractal-like multimodal function
/// Global minimum: f(x) = 1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [0, 100]
pub fn katsuura(x: &Array1<f64>) -> f64 {
    let d = x.len();
    let mut product = 1.0;
    
    for (i, &xi) in x.iter().enumerate() {
        let mut sum = 0.0;
        // Limit j to prevent overflow, 20 is sufficient for precision
        for j in 1..=20 {
            let power2j = (2.0_f64).powi(j as i32);
            let term = (power2j * xi).abs() - (power2j * xi).round().abs();
            sum += term / power2j;
        }
        product *= 1.0 + (i + 1) as f64 * sum;
    }
    
    let factor = 100.0 / (d as f64).powi(2);
    factor * product - factor
}

/// Epistatic Michalewicz function - modified version for GA testing
/// Global minimum: varies by dimension
/// Bounds: x_i in [0, π]
pub fn epistatic_michalewicz(x: &Array1<f64>) -> f64 {
    let m = 10.0; // Steepness parameter
    let n = x.len();
    
    // Add epistatic (interaction) terms
    let base_sum = -x.iter().enumerate().map(|(i, &xi)| {
        xi.sin() * ((i as f64 + 1.0) * xi.powi(2) / std::f64::consts::PI).sin().powf(2.0 * m)
    }).sum::<f64>();
    
    // Add epistatic interactions between adjacent variables
    let epistatic_sum: f64 = (0..n-1).map(|i| {
        let xi = x[i];
        let xi_plus_1 = x[i + 1];
        0.1 * (xi * xi_plus_1).sin().powi(2)
    }).sum();
    
    base_sum + epistatic_sum
}
