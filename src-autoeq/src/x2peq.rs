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

use crate::cli::PeqModel;
use crate::iir::{Biquad, BiquadFilterType, Peq};
use ndarray::Array1;

/// Convert parameter vector to Peq structure
///
/// # Arguments
/// * `x` - Parameter vector with triplets [log10(freq), Q, gain] for each filter
/// * `srate` - Sample rate in Hz
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// A Peq structure containing the filters
pub fn x2peq(x: &[f64], srate: f64, peq_model: PeqModel) -> Peq {
    let num_filters = x.len() / 3;
    let mut peq = Vec::with_capacity(num_filters);

    for i in 0..num_filters {
        let freq = 10f64.powf(x[i * 3]);
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];

        let ftype = match peq_model {
            PeqModel::Pk => BiquadFilterType::Peak,
            PeqModel::HpPk => {
                if i == 0 {
                    BiquadFilterType::HighpassVariableQ
                } else {
                    BiquadFilterType::Peak
                }
            }
            PeqModel::HpPkLp => {
                if i == 0 {
                    BiquadFilterType::HighpassVariableQ
                } else if i == num_filters - 1 {
                    BiquadFilterType::Lowpass
                } else {
                    BiquadFilterType::Peak
                }
            }
            PeqModel::FreePkFree | PeqModel::Free => {
                // For now, treat these as peak filters
                // TODO: Implement free filter type selection
                BiquadFilterType::Peak
            }
        };

        let filter = Biquad::new(ftype, freq, srate, q, gain);
        peq.push((1.0, filter));
    }

    peq
}

/// Convert Peq structure back to parameter vector
///
/// # Arguments
/// * `peq` - Peq structure containing the filters
///
/// # Returns
/// Parameter vector with triplets [log10(freq), Q, gain] for each filter
pub fn peq2x(peq: &Peq) -> Vec<f64> {
    let mut x = Vec::with_capacity(peq.len() * 3);

    for (_weight, filter) in peq {
        x.push(filter.freq.log10());
        x.push(filter.q);
        x.push(filter.db_gain);
    }

    x
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
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// Frequency response in dB SPL at the specified frequency points
pub fn x2spl(freqs: &Array1<f64>, x: &[f64], srate: f64, peq_model: PeqModel) -> Array1<f64> {
    let peq = x2peq(x, srate, peq_model);
    crate::iir::compute_peq_response(freqs, &peq, srate)
}

/// Compute the combined PEQ response from parameter vector
///
/// This is an alias for x2spl, provided for compatibility
pub fn compute_peq_response_from_x(
    freqs: &Array1<f64>,
    x: &[f64],
    sample_rate: f64,
    peq_model: PeqModel,
) -> Array1<f64> {
    x2spl(freqs, x, sample_rate, peq_model)
}

/// Build a vector of sorted filter rows from optimization parameters
///
/// # Arguments
/// * `x` - Slice of optimization parameters laid out as [f0, Q, gain, f0, Q, gain, ...]
/// * `peq_model` - PEQ model that defines the filter structure
///
/// # Returns
/// * Vector of FilterRow structs sorted by frequency
///
/// # Details
/// Converts the flat parameter vector into a vector of FilterRow structs,
/// sorts them by frequency, and marks filters according to the PEQ model.
pub fn build_sorted_filters(x: &[f64], peq_model: PeqModel) -> Vec<crate::iir::FilterRow> {
    let mut rows: Vec<crate::iir::FilterRow> = Vec::with_capacity(x.len() / 3);
    for i in 0..(x.len() / 3) {
        let freq = 10f64.powf(x[i * 3]);
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];
        rows.push(crate::iir::FilterRow {
            freq,
            q,
            gain,
            kind: "PK",
        });
    }
    rows.sort_by(|a, b| {
        a.freq
            .partial_cmp(&b.freq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Mark filters according to the PEQ model for display purposes
    if !rows.is_empty() {
        let n = rows.len();
        match peq_model {
            PeqModel::HpPk => {
                rows[0].kind = "HPQ";
                rows[0].gain = 0.0;
            }
            PeqModel::HpPkLp => {
                rows[0].kind = "HPQ";
                rows[0].gain = 0.0;
                if n > 1 {
                    rows[n - 1].kind = "LP";
                    rows[n - 1].gain = 0.0;
                }
            }
            _ => {}
        }
    }
    rows
}

/// Print a formatted table of the parametric EQ filters.
///
/// The filters are printed with any non-Peak filters marked according to the PEQ model,
/// with all filters sorted by frequency.
pub fn peq_print_from_x(x: &[f64], peq_model: PeqModel) {
    let peq = x2peq(x, crate::iir::SRATE, peq_model);
    crate::iir::peq_print(&peq);
}
