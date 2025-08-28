use crate::iir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use nlopt::{Algorithm, Nlopt, Target};

use crate::loss::{LossType, ScoreLossData, score_loss};

/// Data structure for holding objective function parameters
///
/// This struct contains all the data needed to compute the objective function
/// for filter optimization.
#[derive(Debug, Clone)]
pub struct ObjectiveData {
    /// Frequency points for evaluation
    pub freqs: Array1<f64>,
    /// Target error values
    pub target_error: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    /// Minimum spacing between filters in octaves
    pub min_spacing_oct: f64,
    /// Weight for spacing penalty term
    pub spacing_weight: f64,
    /// Maximum allowed dB level
    pub max_db: f64,
    /// Minimum absolute gain for filters
    pub min_db: f64,
    /// Whether to use highpass/peak filter configuration
    pub iir_hp_pk: bool,
    /// Type of loss function to use
    pub loss_type: LossType,
    /// Optional score data for Score loss type
    pub score_data: Option<ScoreLossData>,
}

fn parse_algorithm(name: &str) -> Algorithm {
    match name.to_lowercase().as_str() {
        "isres" => Algorithm::Isres,
        "cobyla" => Algorithm::Cobyla,
        "neldermead" | "nm" => Algorithm::Neldermead,
        "bobyqa" => Algorithm::Bobyqa,
        "sbplx" | "subplex" => Algorithm::Sbplx,
        "crs2lm" | "crs" => Algorithm::Crs2Lm,
        "slsqp" => Algorithm::Slsqp,
        _ => Algorithm::Isres,
    }
}

fn objective_function(x: &[f64], _gradient: Option<&mut [f64]>, data: &mut ObjectiveData) -> f64 {
    let num_filters = x.len() / 3;
    let mut combined_spl = Array1::zeros(data.target_error.len());

    // Each filter is defined by 3 parameters: freq, Q, and gain.
    // If enabled, determine which filter has the lowest frequency; make it Highpass, others Peak
    let mut hp_index = usize::MAX;
    if data.iir_hp_pk {
        hp_index = 0usize;
        if num_filters > 0 {
            let mut min_f = x[0];
            for i in 1..num_filters {
                let f = x[i * 3];
                if f < min_f {
                    min_f = f;
                    hp_index = i;
                }
            }
        }
    }

    for i in 0..num_filters {
        let freq = x[i * 3];
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];

        let ftype = if data.iir_hp_pk && i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, freq, data.srate, q, gain);
        let resp = filter.np_log_result(&data.freqs);
        combined_spl += &resp;
    }

    // Compute base fit depending on loss type
    let fit = match data.loss_type {
        LossType::Flat => {
            // Error vs inverted target
            let error = &combined_spl - &data.target_error;
            weighted_mse(&data.freqs, &error)
        }
        LossType::Score => {
            if let Some(ref sd) = data.score_data {
                // combined_spl is the PEQ response
                score_loss(sd, &data.freqs, &combined_spl)
            } else {
                // Fallback to flat if score data missing
                let error = &combined_spl - &data.target_error;
                weighted_mse(&data.freqs, &error)
            }
        }
    };

    // Add spacing penalty between center frequencies in octaves
    let spacing = spacing_penalty(x, data.min_spacing_oct);

    // Enforce minimum absolute gain for Peak EQs if min_db > 0
    let mut min_amp_penalty = 0.0;
    if data.min_db > 0.0 {
        let n = x.len() / 3;
        // Recompute hp_index only if mode uses Highpass
        let mut hp_index = usize::MAX;
        if data.iir_hp_pk {
            hp_index = 0usize;
            if n > 0 {
                let mut min_f = x[0];
                for i in 1..n {
                    let f = x[i * 3];
                    if f < min_f {
                        min_f = f;
                        hp_index = i;
                    }
                }
            }
        }
        for i in 0..n {
            if data.iir_hp_pk && i == hp_index {
                continue;
            }
            let g = x[i * 3 + 2].abs();
            let short = (data.min_db - g).max(0.0);
            if short > 0.0 {
                min_amp_penalty += short * short;
            }
        }
    }

    fit + data.spacing_weight * spacing + min_amp_penalty
}

/// Calculate spacing penalty between filter center frequencies
///
/// # Arguments
/// * `x` - Parameter vector with [freq, Q, gain] triplets
/// * `min_spacing_oct` - Minimum required spacing in octaves
///
/// # Returns
/// * Penalty value (sum of squared spacing violations)
///
/// # Details
/// Computes a penalty based on how much the spacing between adjacent filter
/// center frequencies falls short of the minimum required spacing.
fn spacing_penalty(x: &[f64], min_spacing_oct: f64) -> f64 {
    // Extract center frequencies (every 3rd element starting at 0)
    let n = x.len() / 3;
    let mut penalty = 0.0;
    if n <= 1 || min_spacing_oct <= 0.0 {
        return 0.0;
    }
    // Compare all pairs; use log2 ratio distance
    for i in 0..n {
        let fi = x[i * 3].max(1e-6);
        for j in (i + 1)..n {
            let fj = x[j * 3].max(1e-6);
            let d_oct = (fj / fi).log2().abs();
            let shortfall = (min_spacing_oct - d_oct).max(0.0);
            if shortfall > 0.0 {
                penalty += shortfall * shortfall;
            }
        }
    }
    penalty
}

/// Compute weighted mean squared error with frequency-dependent weighting
///
/// # Arguments
/// * `freqs` - Frequency points
/// * `error` - Error values at each frequency point
///
/// # Returns
/// * Weighted error value
///
/// # Details
/// Computes RMS error separately for frequencies below and above 3000 Hz,
/// with higher weight given to the lower frequency band.
fn weighted_mse(freqs: &Array1<f64>, error: &Array1<f64>) -> f64 {
    debug_assert_eq!(freqs.len(), error.len());
    let mut ss1 = 0.0; // sum of squares for f < 3000
    let mut n1: usize = 0;
    let mut ss2 = 0.0; // sum of squares for f >= 3000
    let mut n2: usize = 0;

    for i in 0..freqs.len() {
        let e = error[i];
        if freqs[i] < 3000.0 {
            ss1 += e * e;
            n1 += 1;
        } else {
            ss2 += e * e;
            n2 += 1;
        }
    }
    // RMS in each band: sqrt(mean of squares)
    let err1 = if n1 > 0 {
        (ss1 / n1 as f64).sqrt()
    } else {
        0.0
    };
    let err2 = if n2 > 0 {
        (ss2 / n2 as f64).sqrt()
    } else {
        0.0
    };
    err1 + err2 / 3.0
}

/// Optimize filter parameters using global optimization algorithms
///
/// # Arguments
/// * `x` - Initial parameter vector to optimize (modified in place)
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Data structure containing optimization parameters
/// * `algo` - Optimization algorithm name (e.g., "isres", "cobyla")
/// * `population` - Population size for population-based algorithms
/// * `maxeval` - Maximum number of function evaluations
///
/// # Returns
/// * Result containing (status, optimal value) or (error, value)
///
/// # Details
/// Uses the NLopt library to perform global optimization of filter parameters.
/// The parameter vector is organized as [freq, Q, gain] triplets for each filter.
pub fn optimize_filters(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    algo: &str,
    population: Option<usize>,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();
    let mut optimizer = Nlopt::new(
        parse_algorithm(algo),
        num_params,
        objective_function,
        Target::Minimize,
        objective_data.clone(),
    );

    optimizer.set_lower_bounds(lower_bounds).unwrap();
    optimizer.set_upper_bounds(upper_bounds).unwrap();

    // Enforce total response ceiling via nonlinear inequality constraint
    // Note: Constraint functionality removed due to nlopt API changes

    // Enforce |g_i| >= min_db as hard constraints when min_db > 0
    // Note: Constraint functionality removed due to nlopt API changes

    if let Some(pop) = population {
        let _ = optimizer.set_population(pop);
    }
    let _ = optimizer.set_maxeval(maxeval.try_into().unwrap());
    optimizer.set_stopval(1e-4).unwrap();
    optimizer.set_ftol_rel(1e-6).unwrap();
    optimizer.set_xtol_rel(1e-6).unwrap();

    let result = optimizer.optimize(x);
    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}

/// Refine filter parameters using local optimization algorithms
///
/// # Arguments
/// * `x` - Initial parameter vector to optimize (modified in place)
/// * `lower_bounds` - Lower bounds for each parameter
/// * `upper_bounds` - Upper bounds for each parameter
/// * `objective_data` - Data structure containing optimization parameters
/// * `local_algo` - Local optimization algorithm name
/// * `maxeval` - Maximum number of function evaluations
///
/// # Returns
/// * Result containing (status, optimal value) or (error, value)
///
/// # Details
/// Uses the NLopt library to perform local optimization of filter parameters.
/// This function is typically called after global optimization to fine-tune results.
pub fn refine_local(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    local_algo: &str,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();
    let mut opt = Nlopt::new(
        parse_algorithm(local_algo),
        num_params,
        objective_function,
        Target::Minimize,
        objective_data.clone(),
    );
    opt.set_lower_bounds(lower_bounds).unwrap();
    opt.set_upper_bounds(upper_bounds).unwrap();
    // Enforce total response ceiling during local refinement too
    // Note: Constraint functionality removed due to nlopt API changes
    // Enforce |g_i| >= min_db for local stage as well
    // Note: Constraint functionality removed due to nlopt API changes
    let _ = opt.set_maxeval(maxeval.try_into().unwrap());
    opt.set_ftol_rel(1e-8).unwrap();
    opt.set_xtol_rel(1e-8).unwrap();

    let result = opt.optimize(x);
    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_weighted_mse_basic() {
        // Two points below 3k, two points above 3k with unit error
        let freqs = array![1000.0, 2000.0, 4000.0, 8000.0];
        let err = array![1.0, 1.0, 1.0, 1.0];
        let v = weighted_mse(&freqs, &err);
        // RMS below = 1, RMS above = 1 -> total = 1 + 1/3 = 1.333...
        assert!((v - (1.0 + 1.0 / 3.0)).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn test_weighted_mse_empty_upper_segment() {
        // All freqs below 3k -> upper RMS = 0
        let freqs = array![100.0, 200.0, 500.0];
        let err = array![2.0, 2.0, 2.0]; // squares: 4,4,4 -> mean=4 -> rms=2
        let v = weighted_mse(&freqs, &err);
        assert!((v - 2.0).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn test_weighted_mse_scaling() {
        // Different errors below and above to verify weighting
        let freqs = array![1000.0, 1500.0, 4000.0, 6000.0];
        let err = array![2.0, 2.0, 3.0, 3.0];
        // below RMS = sqrt((4+4)/2)=2, above RMS = sqrt((9+9)/2)=3
        let v = weighted_mse(&freqs, &err);
        let expected = 2.0 + 3.0 / 3.0; // 3.0
        assert!((v - expected).abs() < 1e-12, "got {}", v);
    }

    #[test]
    fn spacing_penalty_zero_when_spread() {
        // x: [f,q,g, f,q,g, f,q,g]
        let x = [100.0, 1.0, 0.0, 200.0, 1.0, 0.0, 400.0, 1.0, 0.0];
        // 100->200: 1 octave, 200->400: 1 octave, 100->400: 2 octaves
        let p = spacing_penalty(&x, 0.5);
        assert!(p == 0.0, "penalty should be zero, got {}", p);
    }

    #[test]
    fn spacing_penalty_positive_when_close() {
        let x = [1000.0, 1.0, 0.0, 1100.0, 1.0, 0.0];
        // log2(1100/1000) ~ 0.1375 octaves, require 0.25 -> shortfall ~ 0.1125
        let p = spacing_penalty(&x, 0.25);
        assert!(p > 0.0);
    }

    #[test]
    fn objective_spacing_penalty_zero_when_spread() {
        // Use empty freqs/target so the fit term is 0 and only spacing/min_amp contribute.
        // Set min_db=0 so min_amp term is disabled. Then objective == spacing_weight*spacing.
        let mut data = ObjectiveData {
            freqs: Array1::<f64>::zeros(0),
            target_error: Array1::<f64>::zeros(0),
            srate: 48_000.0,
            min_spacing_oct: 0.4,
            spacing_weight: 10.0,
            max_db: 6.0,
            min_db: 0.0,
            iir_hp_pk: false,
            loss_type: LossType::Flat,
            score_data: None,
        };
        // Two filters spaced by 1 octave -> spacing penalty should be 0 -> objective 0
        let x_spread = [1000.0, 1.0, 0.0, 2000.0, 1.0, 0.0];
        let obj = objective_function(&x_spread, None, &mut data);
        assert!(
            obj.abs() < 1e-12,
            "objective should be 0 when spacing OK, got {}",
            obj
        );
    }

    #[test]
    fn objective_spacing_penalty_positive_when_close() {
        // Same setup as above to isolate spacing term
        let mut data = ObjectiveData {
            freqs: Array1::<f64>::zeros(0),
            target_error: Array1::<f64>::zeros(0),
            srate: 48_000.0,
            min_spacing_oct: 0.4,
            spacing_weight: 10.0,
            max_db: 6.0,
            min_db: 0.0,
            iir_hp_pk: false,
            loss_type: LossType::Flat,
            score_data: None,
        };
        // Two filters too close -> objective should equal spacing_weight * spacing_penalty
        let x_close = [1000.0, 1.0, 0.0, 1100.0, 1.0, 0.0];
        let expected_spacing = spacing_penalty(&x_close, data.min_spacing_oct);
        let expected_obj = data.spacing_weight * expected_spacing;
        let obj = objective_function(&x_close, None, &mut data);
        assert!(
            obj > 0.0,
            "objective should be positive when spacing violated"
        );
        assert!(
            (obj - expected_obj).abs() < 1e-12,
            "objective {} != expected {}",
            obj,
            expected_obj
        );
    }
}

/// Extract sorted center frequencies from parameter vector and compute adjacent spacings in octaves.
pub fn compute_sorted_freqs_and_adjacent_octave_spacings(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() / 3;
    let mut freqs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        freqs.push(x[i * 3]);
    }
    freqs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let spacings: Vec<f64> = if freqs.len() < 2 {
        Vec::new()
    } else {
        freqs
            .windows(2)
            .map(|w| (w[1].max(1e-9) / w[0].max(1e-9)).log2().abs())
            .collect()
    };
    (freqs, spacings)
}

#[cfg(test)]
mod spacing_diag_tests {
    use super::compute_sorted_freqs_and_adjacent_octave_spacings;

    #[test]
    fn adjacent_octave_spacings_basic() {
        // x: [f,q,g, f,q,g, f,q,g]
        let x = [100.0, 1.0, 0.0, 200.0, 1.0, 0.0, 400.0, 1.0, 0.0];
        let (freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        assert_eq!(freqs, vec![100.0, 200.0, 400.0]);
        assert!((spacings[0] - 1.0).abs() < 1e-12);
        assert!((spacings[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn adjacent_octave_spacings_two_filters() {
        let x = [1000.0, 1.0, 0.0, 1100.0, 1.0, 0.0];
        let (_freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        // log2(1100/1000) ~ 0.1375 octaves
        assert!(spacings.len() == 1);
        assert!((spacings[0] - (1100.0_f64 / 1000.0).log2().abs()).abs() < 1e-12);
    }
}
