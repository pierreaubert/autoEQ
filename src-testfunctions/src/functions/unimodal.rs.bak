//! Unimodal test functions
//!
//! These functions have a single global optimum and are typically used to test
//! convergence speed and precision of optimization algorithms.

use ndarray::Array1;

/// Simple quadratic function for basic testing
/// f(x) = sum(x[i]^2)
/// Global minimum at (0, 0, ..., 0) with f = 0
pub fn quadratic(x: &Array1<f64>) -> f64 {
    x.iter().map(|&xi| xi * xi).sum()
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

/// Quartic function with noise - unimodal with added random noise
/// Global minimum: f(x) ≈ 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1.28, 1.28]
pub fn quartic(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| (i as f64 + 1.0) * xi.powi(4))
        .sum::<f64>()
    // Note: Original includes random noise, but we omit it for deterministic testing
}

/// Rotated hyper-ellipsoid function - unimodal, non-separable
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-65.536, 65.536]
pub fn rotated_hyper_ellipsoid(x: &Array1<f64>) -> f64 {
    (0..x.len())
        .map(|i| x.iter().take(i + 1).map(|&xi| xi.powi(2)).sum::<f64>())
        .sum::<f64>()
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

/// McCormick function - 2D function
/// Global minimum: f(x) = -1.9133 at x = (-0.54719, -1.54719)
/// Bounds: x1 in [-1.5, 4], x2 in [-3, 4]
pub fn mccormick(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (x1 + x2).sin() + (x1 - x2).powi(2) - 1.5 * x1 + 2.5 * x2 + 1.0
}

/// Sum Squares Function - simple quadratic bowl-shaped function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-10, 10]
pub fn sum_squares(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| (i as f64 + 1.0) * xi.powi(2))
        .sum()
}

/// Different Powers function - unimodal with different scaling
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 1]
pub fn different_powers(x: &Array1<f64>) -> f64 {
    x.iter().enumerate()
        .map(|(i, &xi)| xi.abs().powf((i + 2) as f64))
        .sum()
}

/// Elliptic function - separable ill-conditioned function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn elliptic(x: &Array1<f64>) -> f64 {
    let n = x.len();
    if n == 0 {
        return 0.0;
    }
    x.iter().enumerate()
        .map(|(i, &xi)| {
            let condition_factor = 1e6_f64.powf(i as f64 / (n - 1).max(1) as f64);
            condition_factor * xi.powi(2)
        })
        .sum()
}

/// Cigar function - another ill-conditioned variant
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn cigar(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let first = x[0].powi(2);
    let rest: f64 = x.iter().skip(1).map(|&xi| 1e6 * xi.powi(2)).sum();
    first + rest
}

/// Tablet function - complementary to cigar function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn tablet(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let first = 1e6 * x[0].powi(2);
    let rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    first + rest
}

/// Discus function - ill-conditioned unimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn discus(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let first = 1e6 * x[0].powi(2);
    let rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    first + rest
}

/// Bent Cigar function (alternative implementation)
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn bent_cigar_alt(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let first = x[0].powi(2);
    let rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    first + 1e6 * rest
}

/// Ridge function - challenging unimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5, 5]
pub fn ridge(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let x1 = x[0];
    let sum_rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    x1 + 100.0 * sum_rest.sqrt()
}

/// Sharp Ridge function - even more challenging ridge
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-5, 5]
pub fn sharp_ridge(x: &Array1<f64>) -> f64 {
    if x.len() == 0 {
        return 0.0;
    }
    let x1 = x[0];
    let sum_rest: f64 = x.iter().skip(1).map(|&xi| xi.powi(2)).sum();
    x1.powi(2) + 100.0 * sum_rest.sqrt()
}

/// Brown function - ill-conditioned unimodal function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 4]
pub fn brown(x: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    for i in 0..x.len() - 1 {
        let xi = x[i];
        let xi_plus_1 = x[i + 1];
        sum += (xi.powi(2)).powf(xi_plus_1.powi(2) + 1.0);
        sum += (xi_plus_1.powi(2)).powf(xi.powi(2) + 1.0);
    }
    sum
}

/// Chung Reynolds function - unimodal quadratic function
/// Global minimum: f(x) = 0 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-100, 100]
pub fn chung_reynolds(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    sum_squares.powi(2)
}

/// Exponential function - unimodal function
/// Global minimum: f(x) = -1 at x = (0, 0, ..., 0)
/// Bounds: x_i in [-1, 1]
pub fn exponential(x: &Array1<f64>) -> f64 {
    let sum_squares: f64 = x.iter().map(|&xi| xi.powi(2)).sum();
    -(-0.5 * sum_squares).exp()
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

/// Rosenbrock objective function (2D)
pub fn rosenbrock_objective(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    100.0 * (x2 - x1.powi(2)).powi(2) + (1.0 - x1).powi(2)
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

/// Six-hump camel function - 2D multimodal
/// Global minimum: f(x) = -1.0316 at x = (0.0898, -0.7126) and (-0.0898, 0.7126)
/// Bounds: x1 in [-3, 3], x2 in [-2, 2]
pub fn six_hump_camel(x: &Array1<f64>) -> f64 {
    let x1 = x[0];
    let x2 = x[1];
    (4.0 - 2.1 * x1.powi(2) + x1.powi(4) / 3.0) * x1.powi(2) + x1 * x2 + (-4.0 + 4.0 * x2.powi(2)) * x2.powi(2)
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
