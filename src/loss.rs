//! AutoEQ - A library for audio equalization and filter optimization
//! Loss functions and types for AutoEQ optimizer
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

use crate::score;
use clap::ValueEnum;
use ndarray::Array1;

/// The type of loss function to use during optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LossType {
    /// Flat loss function (minimize deviation from target curve)
    Flat,
    /// Score-based loss function (maximize preference score)
    Score,
}

/// Data required for computing score-based loss
#[derive(Debug, Clone)]
pub struct ScoreLossData {
    /// On-axis SPL measurements
    pub on: Array1<f64>,
    /// Listening window SPL measurements
    pub lw: Array1<f64>,
    /// Sound power SPL measurements
    pub sp: Array1<f64>,
    /// Predicted in-room SPL measurements
    pub pir: Array1<f64>,
    /// Octave band intervals for NBD calculations
    pub intervals: Vec<(usize, usize)>,
}

impl ScoreLossData {
    /// Create a new ScoreLossData instance
    ///
    /// # Arguments
    /// * `on` - On-axis SPL measurements
    /// * `lw` - Listening window SPL measurements
    /// * `sp` - Sound power SPL measurements
    /// * `pir` - Predicted in-room SPL measurements
    /// * `intervals` - Octave band intervals for NBD calculations
    pub fn new(
        on: Array1<f64>,
        lw: Array1<f64>,
        sp: Array1<f64>,
        pir: Array1<f64>,
        intervals: Vec<(usize, usize)>,
    ) -> Self {
        Self {
            on,
            lw,
            sp,
            pir,
            intervals,
        }
    }
}

/// Compute the flat (current) loss which is already implemented in optim::weighted_mse etc.
pub fn flat_loss(weighted_err: f64) -> f64 {
    weighted_err
}

/// Compute the score-based loss. Returns -pref_score so that minimizing it maximizes the preference score.
/// `peq_response` must be computed for the candidate parameters.
pub fn score_loss(
    score_data: &ScoreLossData,
    freq: &Array1<f64>,
    peq_response: &Array1<f64>,
) -> f64 {
    let metrics = score::score_peq_approx(
        freq,
        &score_data.intervals,
        &score_data.lw,
        &score_data.sp,
        &score_data.pir,
        &score_data.on,
        peq_response,
    );
    -metrics.pref_score
}
