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
