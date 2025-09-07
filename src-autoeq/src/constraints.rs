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

/// Convert parameter vector to parametric EQ frequency response
/// 
/// Computes the combined frequency response of all filters specified in the parameter vector.
/// Each filter is defined by 3 consecutive parameters: log10(frequency), Q, and gain.
/// 
/// # Arguments
/// * `freqs` - Frequency points for evaluation (Hz)
/// * `x` - Parameter vector with triplets [log10(freq), Q, gain] for each filter
/// * `srate` - Sample rate in Hz
/// * `iir_hp_pk` - If true, first filter is highpass; otherwise all are peak filters
/// 
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn x2peq(freqs: &Array1<f64>, x: &[f64], srate: f64, iir_hp_pk: bool) -> Array1<f64> {
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

/// Data needed by the nonlinear ceiling constraint callback.
#[derive(Clone)]
pub struct CeilingConstraintData {
    /// Frequency points for evaluation (Hz)
    pub freqs: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    /// Maximum allowed SPL in dB
    pub max_db: f64,
    /// Whether first filter is highpass (HP+PK mode)
    pub iir_hp_pk: bool,
}

/// Data needed by the nonlinear minimum gain constraint callback.
#[derive(Clone, Copy)]
pub struct MinGainConstraintData {
    /// Minimum required absolute gain in dB
    pub min_db: f64,
    /// Whether first filter is highpass (skip in constraint)
    pub iir_hp_pk: bool,
}

/// Data needed by the nonlinear spacing constraint callback.
#[derive(Clone, Copy)]
pub struct SpacingConstraintData {
    /// Minimum required spacing between filter centers in octaves
    pub min_spacing_oct: f64,
}

/// Inequality constraint: combined response must not exceed max_db when HP+PK is enabled.
/// Returns fc(x) = max_i (peq_spl[i] - max_db). Feasible when <= 0.
pub fn constraint_ceiling(
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

/// Inequality constraint: for Peak filters, require |gain| >= min_db (skip HP in HP+PK mode).
/// Returns fc(x) = max_i (min_db - |g_i|) over applicable filters. Feasible when <= 0.
pub fn constraint_min_gain(
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



// ---------------- Penalty helpers (shared) ----------------

/// Compute ceiling constraint violation from frequency response
/// 
/// Calculates the maximum excess over the allowed SPL ceiling. Only applies
/// in HP+PK mode where we need to limit the combined response.
/// 
/// # Arguments
/// * `peq_spl` - Frequency response in dB SPL
/// * `max_db` - Maximum allowed SPL ceiling
/// * `iir_hp_pk` - Whether HP+PK mode is enabled (constraint only active then)
/// 
/// # Returns
/// Maximum violation amount (0.0 if no violation or not HP+PK mode)
pub fn viol_ceiling_from_spl(peq_spl: &Array1<f64>, max_db: f64, iir_hp_pk: bool) -> f64 {
    if !iir_hp_pk {
        return 0.0;
    }
    let mut max_excess = 0.0_f64;
    for &v in peq_spl.iter() {
        let excess = (v - max_db).max(0.0);
        if excess > max_excess {
            max_excess = excess;
        }
    }
    max_excess
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

/// Compute minimum gain constraint violation from parameter vector
/// 
/// Calculates the worst violation of minimum absolute gain requirement.
/// Only applies to peak filters (skips highpass filter in HP+PK mode).
/// 
/// # Arguments
/// * `xs` - Parameter vector with [log10(freq), Q, gain] triplets
/// * `iir_hp_pk` - Whether HP+PK mode is enabled (skip first filter)
/// * `min_db` - Minimum required absolute gain in dB
/// 
/// # Returns
/// Worst gain deficiency (0.0 if no violation or disabled)
pub fn viol_min_gain_from_xs(xs: &[f64], iir_hp_pk: bool, min_db: f64) -> f64 {
    if min_db <= 0.0 {
        return 0.0;
    }
    let n = xs.len() / 3;
    if n == 0 {
        return 0.0;
    }
    let mut worst_short = 0.0_f64;
    for i in 0..n {
        if iir_hp_pk && i == 0 {
            continue;
        }
        let g_abs = xs[i * 3 + 2].abs();
        let short = (min_db - g_abs).max(0.0);
        if short > worst_short {
            worst_short = short;
        }
    }
    worst_short
}

