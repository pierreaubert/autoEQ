use eqopt::{Biquad, BiquadFilterType};
use ndarray::Array1;
use nlopt::{Algorithm, Nlopt, Target};

#[derive(Debug, Clone)]
pub struct ObjectiveData {
    pub freqs: Array1<f64>,
    pub target_error: Array1<f64>,
    pub srate: f64,
    pub min_spacing_oct: f64,
    pub spacing_weight: f64,
    pub max_db: f64,
    pub min_db: f64,
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
    // Determine which filter has the lowest frequency; make it Highpass, others Peak
    let mut hp_index = 0usize;
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

    for i in 0..num_filters {
        let freq = x[i * 3];
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];

        let ftype = if i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, freq, data.srate, q, gain);
        let resp = filter.np_log_result(&data.freqs);
        combined_spl += &resp;
    }

    // The error is the difference between our filter chain and the target error.
    let error = combined_spl - &data.target_error;

    // Weighted objective: RMS below 3000 Hz + (RMS above 3000 Hz)/3
    let fit = weighted_mse(&data.freqs, &error);

    // Add spacing penalty between center frequencies in octaves
    let spacing = spacing_penalty(x, data.min_spacing_oct);

    // Enforce minimum absolute gain for Peak EQs if min_db > 0
    let mut min_amp_penalty = 0.0;
    if data.min_db > 0.0 {
        let n = x.len() / 3;
        //for i in 0..n {
            // Skip the Highpass (lowest frequency index)
            // Identify hp_index again to avoid passing it around
            // (small overhead, but keeps code simple)
            // Alternatively, we could have captured hp_index earlier.
        //}
        // Recompute hp_index
        let mut hp_index = 0usize;
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
        for i in 0..n {
            if i == hp_index {
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

// Nonlinear inequality constraint: ensure total combined response does not exceed max_db.
// Returns max_over_freq(combined_spl - max_db), which must be <= 0 to satisfy the constraint.
fn ceiling_constraint(x: &[f64], _gradient: Option<&mut [f64]>, data: &mut ObjectiveData) -> f64 {
    let num_filters = x.len() / 3;
    let mut combined_spl = Array1::zeros(data.freqs.len());

    // Determine lowest-frequency filter index for Highpass designation
    let mut hp_index = 0usize;
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

    for i in 0..num_filters {
        let freq = x[i * 3];
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];

        let ftype = if i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, freq, data.srate, q, gain);
        let resp = filter.np_log_result(&data.freqs);
        combined_spl += &resp;
    }

    // Compute maximum violation above max_db (<= 0 means no violation)
    let mut max_violation = f64::NEG_INFINITY;
    for &v in combined_spl.iter() {
        let excess = v - data.max_db;
        if excess > max_violation {
            max_violation = excess;
        }
    }
    max_violation
}

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
    optimizer
        .add_inequality_constraint(ceiling_constraint, objective_data, 1e-3)
        .unwrap();

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
    opt
        .add_inequality_constraint(ceiling_constraint, objective_data, 1e-4)
        .unwrap();
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
    fn ceiling_constraint_no_violation() {
        // Two filters, one HP at 100 Hz (lowest), one Peak at 1000 Hz with 0 dB gain
        let x = [100.0, 1.0, 0.0, 1000.0, 1.0, 0.0];
        let data = ObjectiveData {
            freqs: array![1000.0],
            target_error: array![0.0],
            srate: 48_000.0,
            min_spacing_oct: 0.25,
            spacing_weight: 1.0,
            max_db: 6.0,
            min_db: 0.0,
        };
        let v = ceiling_constraint(&x, None, &mut data.clone());
        assert!(v <= 1e-9, "constraint should be satisfied, got {}", v);
    }

    #[test]
    fn ceiling_constraint_detects_violation() {
        // Two filters, HP at 100 Hz and Peak at 1000 Hz with strong positive gain
        let x = [100.0, 1.0, 0.0, 1000.0, 1.0, 12.0];
        let data = ObjectiveData {
            freqs: array![1000.0],
            target_error: array![0.0],
            srate: 48_000.0,
            min_spacing_oct: 0.25,
            spacing_weight: 1.0,
            max_db: 0.1, // very low ceiling to trigger violation
            min_db: 0.0,
        };
        let v = ceiling_constraint(&x, None, &mut data.clone());
        assert!(v > 0.0, "expected violation > 0, got {}", v);
    }
}
