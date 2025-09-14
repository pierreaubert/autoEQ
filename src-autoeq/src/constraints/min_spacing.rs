/// Data needed by the nonlinear spacing constraint callback.
#[derive(Clone, Copy)]
pub struct SpacingConstraintData {
    /// Minimum required spacing between filter centers in octaves
    pub min_spacing_oct: f64,
}

/// Inequality constraint: spacing between any pair of center freqs must be at least min_spacing_oct.
/// Returns fc(x) = min_spacing_oct - min_pair_distance. Feasible when <= 0.
pub fn constraint_spacing(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut SpacingConstraintData,
) -> f64 {
    let n = x.len() / 3;
    if n <= 1 || data.min_spacing_oct <= 0.0 {
        return 0.0;
    }
    let mut min_dist = f64::INFINITY;
    for i in 0..n {
        let fi = 10f64.powf(x[i * 3]).max(1e-6);
        for j in (i + 1)..n {
            let fj = 10f64.powf(x[j * 3]).max(1e-6);
            let d_oct = (fj / fi).log10().abs();
            if d_oct < min_dist {
                min_dist = d_oct;
            }
        }
    }
    if min_dist.is_finite() {
        data.min_spacing_oct - min_dist
    } else {
        0.0
    }
}

/// Compute spacing constraint violation from parameter vector
///
/// Calculates how much the closest pair of filters violates the minimum
/// spacing requirement in octaves.
///
/// # Arguments
/// * `xs` - Parameter vector with [log10(freq), Q, gain] triplets
/// * `min_spacing_oct` - Minimum required spacing in octaves
///
/// # Returns
/// Spacing violation amount (0.0 if no violation or disabled)
pub fn viol_spacing_from_xs(xs: &[f64], min_spacing_oct: f64) -> f64 {
    let n = xs.len() / 3;
    if n <= 1 || min_spacing_oct <= 0.0 {
        return 0.0;
    }
    let mut min_dist = f64::INFINITY;
    for i in 0..n {
        let fi = 10f64.powf(xs[i * 3]).max(1e-9);
        for j in (i + 1)..n {
            let fj = 10f64.powf(xs[j * 3]).max(1e-9);
            let d_oct = (fj / fi).log2().abs();
            if d_oct < min_dist {
                min_dist = d_oct;
            }
        }
    }
    if !min_dist.is_finite() {
        0.0
    } else {
        (min_spacing_oct - min_dist).max(0.0)
    }
}

