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
use std::process;

use crate::loss::{LossType, ScoreLossData, flat_loss, mixed_loss, score_loss};

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
    #[allow(dead_code)]
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

/// Data needed by the nonlinear ceiling constraint callback.
#[derive(Clone)]
struct CeilingConstraintData {
    freqs: Array1<f64>,
    srate: f64,
    max_db: f64,
    iir_hp_pk: bool,
}

/// Data needed by the nonlinear minimum gain constraint callback.
#[derive(Clone, Copy)]
struct MinGainConstraintData {
    min_db: f64,
    iir_hp_pk: bool,
}

/// Data needed by the nonlinear spacing constraint callback.
#[derive(Clone, Copy)]
struct SpacingConstraintData {
    min_spacing_oct: f64,
}

fn x2peq(freqs: &Array1<f64>, x: &[f64], srate: f64, iir_hp_pk: bool) -> Array1<f64> {
    let num_filters = x.len() / 3;
    let mut peq_spl = Array1::<f64>::zeros(freqs.len());
    for i in 0..num_filters {
        let freq = 10f64.powf(x[i * 3]);
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];
        let ftype = if iir_hp_pk && i == 0 {
            BiquadFilterType::HighpassVariableQ
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, freq, srate, q, gain);
        peq_spl += &filter.np_log_result(&freqs);
    }
    peq_spl
}

/// Inequality constraint: combined response must not exceed max_db when HP+PK is enabled.
/// Returns fc(x) = max_i (peq_spl[i] - max_db). Feasible when <= 0.
fn constraint_ceiling(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut CeilingConstraintData,
) -> f64 {
    let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);
    // return max excess (can be negative if no violation)
    let mut max_excess = f64::NEG_INFINITY;
    for &v in peq_spl.iter() {
        let excess = v - data.max_db;
        if excess > max_excess {
            max_excess = excess;
        }
    }
    if max_excess.is_finite() {
        max_excess
    } else {
        0.0
    }
}

/// Inequality constraint: spacing between any pair of center freqs must be at least min_spacing_oct.
/// Returns fc(x) = min_spacing_oct - min_pair_distance. Feasible when <= 0.
fn constraint_spacing(
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

/// Inequality constraint: for Peak filters, require |gain| >= min_db (skip HP in HP+PK mode).
/// Returns fc(x) = max_i (min_db - |g_i|) over applicable filters. Feasible when <= 0.
fn constraint_min_gain(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut MinGainConstraintData,
) -> f64 {
    if data.min_db <= 0.0 {
        return 0.0;
    }
    let n = x.len() / 3;
    if n == 0 {
        return 0.0;
    }
    let mut worst = f64::NEG_INFINITY;
    for i in 0..n {
        if data.iir_hp_pk && i == 0 {
            continue;
        }
        let g_abs = x[i * 3 + 2].abs();
        let short = data.min_db - g_abs; // can be negative when satisfied
        if short > worst {
            worst = short;
        }
    }
    if worst.is_finite() { worst } else { 0.0 }
}

fn parse_algorithm(name: &str) -> Algorithm {
    match name.to_lowercase().as_str() {
        // local
        "bobyqa" => Algorithm::Bobyqa,
        "cobyla" => Algorithm::Cobyla,
        "neldermead" => Algorithm::Neldermead,
        // global with inequality support
        "isres" => Algorithm::Isres,
        "ags" => Algorithm::Ags,
        "origdirect" => Algorithm::OrigDirect,
        // global without inequality support
        "crs2lm" => Algorithm::Crs2Lm,
        "direct" => Algorithm::Direct,
        "directl" => Algorithm::DirectL,
        "gmlsl" => Algorithm::GMlsl,
        "gmlsllds" => Algorithm::GMlslLds,
        "sbplx" => Algorithm::Sbplx,
        "slsqp" => Algorithm::Slsqp,
        "stogo" => Algorithm::StoGo,
        "stogorand" => Algorithm::StoGoRand,
        // default to
        _ => Algorithm::Isres,
    }
}

fn objective_function(x: &[f64], _gradient: Option<&mut [f64]>, data: &mut ObjectiveData) -> f64 {
    let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);

    // Compute base fit depending on loss type
    let fit = match data.loss_type {
        LossType::Flat => {
            // Error vs inverted target
            let error = &peq_spl - &data.target_error;
            let f = flat_loss(&data.freqs, &error);
            // println!("Flat fit: {}", f);
            f
        }
        LossType::Mixed => {
            if let Some(ref sd) = data.score_data {
                let s = mixed_loss(sd, &data.freqs, &peq_spl);
                // println!("Mixed: {:5.2} ", s);
                s
            } else {
                eprintln!("Error: mixed loss requested but score data is missing");
                process::exit(1);
            }
        }
        LossType::Score => {
            if let Some(ref sd) = data.score_data {
                let error = &peq_spl - &data.target_error;
                let s = score_loss(sd, &data.freqs, &peq_spl);
                let p = flat_loss(&data.freqs, &error) / 3.0;
                // println!("Score: {:5.2} Flatness: {:6.2}", -100.0 + s, p);
                s + p
            } else {
                eprintln!("Error: score loss requested but score data is missing");
                process::exit(1);
            }
        }
    };

    fit
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
    // Prepare constraint data BEFORE moving objective_data into NLopt
    let ceiling_data = CeilingConstraintData {
        freqs: objective_data.freqs.clone(),
        srate: objective_data.srate,
        max_db: objective_data.max_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };
    let spacing_data = SpacingConstraintData {
        min_spacing_oct: objective_data.min_spacing_oct,
    };
    let min_gain_data = MinGainConstraintData {
        min_db: objective_data.min_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };

    // Now create optimizer and move objective_data
    let mut optimizer = Nlopt::new(
        parse_algorithm(algo),
        num_params,
        objective_function,
        Target::Minimize,
        objective_data,
    );

    let _ = optimizer.set_lower_bounds(lower_bounds).unwrap();
    let _ = optimizer.set_upper_bounds(upper_bounds).unwrap();

    // Register inequality constraints supported by chosen algorithms.
    let _ = optimizer.add_inequality_constraint(constraint_ceiling, ceiling_data, 1e-6);
    // let _ = optimizer.add_inequality_constraint(constraint_spacing, spacing_data, 1e-9);
    let _ = optimizer.add_inequality_constraint(constraint_min_gain, min_gain_data, 1e-6);

    let _ = optimizer.set_population(population);
    let _ = optimizer.set_maxeval(maxeval as u32);
    let _ = optimizer.set_stopval(1e-4).unwrap();
    let _ = optimizer.set_ftol_rel(1e-6).unwrap();
    let _ = optimizer.set_xtol_rel(1e-4).unwrap();

    let result = optimizer.optimize(x);

    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}

/// Extract sorted center frequencies from parameter vector and compute adjacent spacings in octaves.
pub fn compute_sorted_freqs_and_adjacent_octave_spacings(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() / 3;
    let mut freqs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        freqs.push(10f64.powf(x[i * 3]));
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
        let x = [
            100f64.log10(),
            1.0,
            0.0,
            200f64.log10(),
            1.0,
            0.0,
            400f64.log10(),
            1.0,
            0.0,
        ];
        let (freqs, spacings) = compute_sorted_freqs_and_adjacent_octave_spacings(&x);
        assert!((spacings[0] - 1.0).abs() < 1e-12);
        assert!((spacings[1] - 1.0).abs() < 1e-12);
    }
}
