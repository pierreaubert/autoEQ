//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::iir::{Biquad, BiquadFilterType};
use ndarray::Array1;
use nlopt::{Algorithm, Nlopt, Target};
use std::env;
use std::process;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

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
        "bobyqa" => Algorithm::Bobyqa,
        "cobyla" => Algorithm::Cobyla,
        "crs2lm" => Algorithm::Crs2Lm,
        "direct" => Algorithm::Direct,
        "directl" => Algorithm::DirectL,
        "gmlsl" => Algorithm::GMlsl,
        "gmlsllds" => Algorithm::GMlslLds,
        "isres" => Algorithm::Isres,
        "neldermead" => Algorithm::Neldermead,
        "sbplx" => Algorithm::Sbplx,
        "slsqp" => Algorithm::Slsqp,
        "stogo" => Algorithm::StoGo,
        "stogorand" => Algorithm::StoGoRand,
        _ => Algorithm::Isres,
    }
}

// Debug logging configuration for objective function (initialized once from env)
struct DebugCfg {
    enabled: bool,
    every: usize,
    max_logs: usize,
    printed: AtomicUsize,
}

static DEBUG_CFG: OnceLock<DebugCfg> = OnceLock::new();
static OBJ_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DebugParams {
    enabled: bool,
    every: usize,
    max_logs: usize,
}

fn parse_debug_env_with<F>(get: F) -> DebugParams
where
    F: Fn(&str) -> Option<String>,
{
    let enabled = get("AUTOEQ_DEBUG_OBJ")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let every = get("AUTOEQ_DEBUG_EVERY")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&v| v >= 1)
        .unwrap_or(100);
    let max_logs = get("AUTOEQ_DEBUG_MAX")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    DebugParams {
        enabled,
        every,
        max_logs,
    }
}

fn get_debug_cfg() -> &'static DebugCfg {
    DEBUG_CFG.get_or_init(|| {
        let params = parse_debug_env_with(|k| env::var(k).ok());
        DebugCfg {
            enabled: params.enabled,
            every: params.every,
            max_logs: params.max_logs,
            printed: AtomicUsize::new(0),
        }
    })
}

fn objective_function(x: &[f64], _gradient: Option<&mut [f64]>, data: &mut ObjectiveData) -> f64 {
    let num_filters = x.len() / 3;
    let mut peq_spl = Array1::zeros(data.target_error.len());

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
            BiquadFilterType::HighpassVariableQ
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, freq, data.srate, q, gain);
        let resp = filter.np_log_result(&data.freqs);
        peq_spl += &resp;
    }

    // Compute base fit depending on loss type
    let fit = match data.loss_type {
        LossType::Flat => {
            // Error vs inverted target
            let error = &peq_spl - &data.target_error;
            weighted_mse(&data.freqs, &error)
        }
        LossType::Score => {
            if let Some(ref sd) = data.score_data {
                // peq_spl is the PEQ response
                score_loss(sd, &data.freqs, &peq_spl)
            } else {
                eprintln!("Error: score loss requested but score data is missing");
                process::exit(1);
                0.0
            }
        }
    };

    // Ceiling penalty: when using HP+PK mode, cap the total combined response at max_db
    // We penalize the positive excess above max_db. Using the maximum excess keeps
    // behavior close to the previous nonlinear inequality constraint.
    let mut ceiling_penalty = 0.0;
    if data.freqs.len() > 0 {
        let mut max_violation = 0.0_f64;
        for &v in peq_spl.iter() {
            let excess = v - data.max_db;
            if excess > max_violation {
                max_violation = excess;
            }
        }
        if max_violation > 0.0 {
            // Quadratic penalty on the maximum violation; scale moderately
            ceiling_penalty = max_violation * max_violation * 10.0;
        }
    }

    // Add spacing penalty between center frequencies in octaves
    let spacing = spacing_penalty(x, data.min_spacing_oct);

    // Enforce minimum absolute gain for Peak EQs if min_db > 0
    let mut min_amp_penalty = 0.0;
    if data.min_db > 0.0 {
        let n = x.len() / 3;
        // Recompute hp_index only if mode uses Highpass
        let mut hp_index2 = usize::MAX;
        if data.iir_hp_pk {
            hp_index2 = 0usize;
            if n > 0 {
                let mut min_f = x[0];
                for i in 1..n {
                    let f = x[i * 3];
                    if f < min_f {
                        min_f = f;
                        hp_index2 = i;
                    }
                }
            }
        }
        for i in 0..n {
            if data.iir_hp_pk && i == hp_index2 {
                continue;
            }
            let g = x[i * 3 + 2].abs();
            let short = (data.min_db - g).max(0.0);
            if short > 0.0 {
                // Strong barrier to emulate disjoint feasible set for gain:
                // gains in (-min_db, min_db) are heavily discouraged
                // Scale quadratic to keep differentiability for local methods
                const BARRIER_SCALE: f64 = 1e4;
                min_amp_penalty += BARRIER_SCALE * short * short;
            }
        }
    }

    let obj = fit*100.0 + data.spacing_weight * spacing + min_amp_penalty + ceiling_penalty;

    // Periodic debug logging of gain values to detect unintended quantization.
    // Controlled by env vars: AUTOEQ_DEBUG_OBJ (0/1), AUTOEQ_DEBUG_EVERY (default 100), AUTOEQ_DEBUG_MAX (default 50)
    let cfg = get_debug_cfg();
    if cfg.enabled {
        let call_idx = OBJ_CALLS.fetch_add(1, Ordering::Relaxed) + 1;
        if call_idx % cfg.every == 0 {
            let already = cfg.printed.fetch_add(1, Ordering::Relaxed);
            if already < cfg.max_logs {
                // Extract gains from x: indices 2, 5, 8, ...
                let mut gains: Vec<f64> = Vec::with_capacity(num_filters);
                for i in 0..num_filters {
                    gains.push(x[i * 3 + 2]);
                }
                let min_abs = gains.iter().fold(f64::INFINITY, |m, &g| m.min(g.abs()));
                let max_abs = gains.iter().fold(0.0_f64, |m, &g| m.max(g.abs()));
                let gains_str = gains
                    .iter()
                    .map(|g| format!("{:+.4}", g))
                    .collect::<Vec<_>>()
                    .join(", ");
                // Heuristic: check if all gains are ~multiples of 0.5 within 1e-6
                let half_step_like = gains
                    .iter()
                    .all(|&g| ((g / 0.5).round() * 0.5 - g).abs() < 1e-6);
                println!(
                    "[obj {:>6}] gains: [{}]  |min|={:.4} |max|={:.4}{}",
                    call_idx,
                    gains_str,
                    min_abs,
                    max_abs,
                    if half_step_like {
                        "  <- appears quantized to 0.5"
                    } else {
                        ""
                    }
                );
            }
        }
    }

    obj
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
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();
    let mut optimizer = Nlopt::new(
        parse_algorithm(algo),
        num_params,
        objective_function,
        Target::Minimize,
        objective_data,
    );

    optimizer.set_lower_bounds(lower_bounds).unwrap();
    optimizer.set_upper_bounds(upper_bounds).unwrap();

    // Enforce total response ceiling via nonlinear inequality constraint
    // Note: Constraint functionality removed due to nlopt API changes

    // Enforce |g_i| >= min_db as hard constraints when min_db > 0
    // Note: Constraint functionality removed due to nlopt API changes

    optimizer.set_population(population);
    optimizer.set_maxeval(maxeval as u32);
    optimizer.set_stopval(1e-6).unwrap();
    optimizer.set_ftol_rel(1e-5).unwrap();
    optimizer.set_xtol_rel(1e-5).unwrap();

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
        objective_data,
    );
    opt.set_lower_bounds(lower_bounds).unwrap();
    opt.set_upper_bounds(upper_bounds).unwrap();
    opt.set_maxeval(maxeval as u32);

    // Enforce total response ceiling during local refinement too
    // Note: Constraint functionality removed due to nlopt API changes

    // Enforce |g_i| >= min_db for local stage as well
    // Note: Constraint functionality removed due to nlopt API changes

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
    fn min_gain_barrier_penalizes_small_gains_peak_only() {
        // No fit term (empty freqs/target), isolate min gain barrier
        let mut data = ObjectiveData {
            freqs: Array1::<f64>::zeros(0),
            target_error: Array1::<f64>::zeros(0),
            srate: 48_000.0,
            min_spacing_oct: 0.0,
            spacing_weight: 0.0,
            max_db: 6.0,
            min_db: 1.0,
            iir_hp_pk: false,
            loss_type: LossType::Flat,
            score_data: None,
        };
        // One Peak filter with |gain| below min_db should be penalized heavily
        let x_bad = [1000.0, 1.0, 0.5];
        let obj_bad = objective_function(&x_bad, None, &mut data);
        // Same filter with |gain| == min_db should have zero penalty
        let x_ok = [1000.0, 1.0, 1.0];
        let obj_ok = objective_function(&x_ok, None, &mut data);
        assert!(
            obj_bad > obj_ok + 1.0,
            "barrier should strongly penalize small gains: {} vs {}",
            obj_bad,
            obj_ok
        );
    }

    #[test]
    fn min_gain_barrier_skips_highpass_in_hp_pk_mode() {
        // Two filters, first is lowest freq => Highpass, second Peak
        let mut data = ObjectiveData {
            freqs: Array1::<f64>::zeros(0),
            target_error: Array1::<f64>::zeros(0),
            srate: 48_000.0,
            min_spacing_oct: 0.0,
            spacing_weight: 0.0,
            max_db: 6.0,
            min_db: 1.0,
            iir_hp_pk: true,
            loss_type: LossType::Flat,
            score_data: None,
        };
        // x = [f1,q1,g1 (HP, ignored for barrier), f2,q2,g2 (Peak)]
        let x = [60.0, 1.0, 0.1, 1000.0, 1.0, 0.5];
        let obj_hp_small_gain = objective_function(&x, None, &mut data);
        // Increase Peak gain to meet min_db, objective should drop
        let x2 = [60.0, 1.0, 0.1, 1000.0, 1.0, 1.0];
        let obj_peak_ok = objective_function(&x2, None, &mut data);
        assert!(obj_peak_ok < obj_hp_small_gain);
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

    #[test]
    fn ceiling_penalty_applies_in_hp_pk_mode() {
        // Build data with one evaluation frequency to keep things simple
        let freqs = array![1000.0];
        let target_error = array![0.0];

        // Two filters: lowest freq becomes Highpass, second is Peak at 1 kHz with +12 dB
        // x = [f1,q1,g1, f2,q2,g2]
        let x = [60.0, 1.0, 0.0, 1000.0, 1.0, 12.0];

        // Baseline with a very high ceiling -> effectively no ceiling penalty
        let mut data_hi = ObjectiveData {
            freqs: freqs.clone(),
            target_error: target_error.clone(),
            srate: 48_000.0,
            min_spacing_oct: 0.0,
            spacing_weight: 0.0,
            max_db: 100.0,
            min_db: 0.0,
            iir_hp_pk: true,
            loss_type: LossType::Flat,
            score_data: None,
        };
        let obj_hi = objective_function(&x, None, &mut data_hi);

        // Tight ceiling -> should add positive ceiling penalty
        let mut data_lo = data_hi.clone();
        data_lo.max_db = 0.1;
        let obj_lo = objective_function(&x, None, &mut data_lo);

        assert!(
            obj_lo > obj_hi,
            "expected tighter ceiling to increase objective: {} vs {}",
            obj_lo,
            obj_hi
        );
    }

    #[test]
    fn no_ceiling_penalty_when_hp_pk_disabled() {
        let freqs = array![1000.0];
        let target_error = array![0.0];

        // Two Peaks (since hp/pk disabled), with boost at 1k
        let x = [100.0, 1.0, 0.0, 1000.0, 1.0, 12.0];

        let mut data_hi = ObjectiveData {
            freqs: freqs.clone(),
            target_error: target_error.clone(),
            srate: 48_000.0,
            min_spacing_oct: 0.0,
            spacing_weight: 0.0,
            max_db: 100.0,
            min_db: 0.0,
            iir_hp_pk: false,
            loss_type: LossType::Flat,
            score_data: None,
        };
        let obj_hi = objective_function(&x, None, &mut data_hi);

        let mut data_lo = data_hi.clone();
        data_lo.max_db = 0.1;
        let obj_lo = objective_function(&x, None, &mut data_lo);

        // With hp/pk disabled, our ceiling penalty is inactive; objectives should match
        assert!(
            (obj_lo - obj_hi).abs() < 1e-12,
            "objectives should be equal without ceiling penalty"
        );
    }

    #[test]
    fn debug_env_parsing_defaults_and_validation() {
        // No env present -> defaults
        let get = |_k: &str| -> Option<String> { None };
        let p = super::parse_debug_env_with(get);
        assert_eq!(p.enabled, false);
        assert_eq!(p.every, 100);
        assert_eq!(p.max_logs, 50);

        // Invalid values fall back to defaults or are clamped by filter
        let get_bad = |k: &str| -> Option<String> {
            match k {
                "AUTOEQ_DEBUG_OBJ" => Some("no".to_string()),
                "AUTOEQ_DEBUG_EVERY" => Some("0".to_string()), // invalid, should fallback to 100
                "AUTOEQ_DEBUG_MAX" => Some("notanumber".to_string()), // invalid, fallback to 50
                _ => None,
            }
        };
        let p2 = super::parse_debug_env_with(get_bad);
        assert_eq!(p2.enabled, false);
        assert_eq!(p2.every, 100);
        assert_eq!(p2.max_logs, 50);
    }

    #[test]
    fn debug_env_parsing_set_values() {
        let get = |k: &str| -> Option<String> {
            match k {
                "AUTOEQ_DEBUG_OBJ" => Some("true".to_string()),
                "AUTOEQ_DEBUG_EVERY" => Some("5".to_string()),
                "AUTOEQ_DEBUG_MAX" => Some("7".to_string()),
                _ => None,
            }
        };
        let p = super::parse_debug_env_with(get);
        assert_eq!(p.enabled, true);
        assert_eq!(p.every, 5);
        assert_eq!(p.max_logs, 7);
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
