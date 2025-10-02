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

use crate::iir::Peq;
use ndarray::Array1;

/// Convert parameter vector to Peq structure
///
/// # Arguments
/// * `x` - Parameter vector with triplets [log10(freq), Q, gain] for each filter
/// * `srate` - Sample rate in Hz
/// * `iir_hp_pk` - If true, first filter is highpass; otherwise all are peak filters
///
/// # Returns
/// A Peq structure containing the filters
pub fn x2peq(x: &[f64], srate: f64, iir_hp_pk: bool) -> Peq {
    crate::iir::x2peq(x, srate, iir_hp_pk)
}

/// Convert Peq structure back to parameter vector
///
/// # Arguments
/// * `peq` - Peq structure containing the filters
///
/// # Returns
/// Parameter vector with triplets [log10(freq), Q, gain] for each filter
pub fn peq2x(peq: &Peq) -> Vec<f64> {
    crate::iir::peq2x(peq)
}

/// Convert parameter vector to parametric EQ frequency response
///
/// Computes the combined frequency response of all filters specified in the parameter vector.
/// This is a compatibility function that uses x2peq and compute_peq_response internally.
///
/// # Arguments
/// * `freqs` - Frequency points for evaluation (Hz)
/// * `x` - Parameter vector with triplets [log10(freq), Q, gain] for each filter
/// * `srate` - Sample rate in Hz
/// * `iir_hp_pk` - If true, first filter is highpass; otherwise all are peak filters
///
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn x2spl(freqs: &Array1<f64>, x: &[f64], srate: f64, iir_hp_pk: bool) -> Array1<f64> {
    let peq = x2peq(x, srate, iir_hp_pk);
    crate::iir::compute_peq_response(freqs, &peq, srate)
}
