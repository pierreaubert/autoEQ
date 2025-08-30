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
use crate::Curve;
use clap::ValueEnum;
use ndarray::Array1;
use std::collections::HashMap;

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
}

impl ScoreLossData {
    /// Create a new ScoreLossData instance
    ///
    /// # Arguments
    /// * `spin` - Map of CEA2034 curves by name ("On Axis", "Listening Window", "Sound Power", "Estimated In-Room Response")
    pub fn new(spin: &HashMap<String, Curve>) -> Self {
        let on = spin
            .get("On Axis")
            .expect("Missing 'On Axis' in CEA2034 spin data")
            .spl
            .clone();
        let lw = spin
            .get("Listening Window")
            .expect("Missing 'Listening Window' in CEA2034 spin data")
            .spl
            .clone();
        let sp = spin
            .get("Sound Power")
            .expect("Missing 'Sound Power' in CEA2034 spin data")
            .spl
            .clone();
        let pir = spin
            .get("Estimated In-Room Response")
            .expect("Missing 'Estimated In-Room Response' in CEA2034 spin data")
            .spl
            .clone();
        Self { on, lw, sp, pir }
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
    // Compute 1/2-octave intervals on the fly using the provided frequency grid
    let intervals = score::octave_intervals(2, freq);
    let metrics = score::score_peq_approx(
        freq,
        &intervals,
        &score_data.lw,
        &score_data.sp,
        &score_data.pir,
        &score_data.on,
        peq_response,
    );
    -metrics.pref_score
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use std::collections::HashMap;

    #[test]
    fn score_loss_matches_score_when_peq_zero() {
        // Simple synthetic data
        let freq = Array1::from(vec![100.0, 1000.0]);
        let on = Array1::from(vec![80.0_f64, 85.0_f64]);
        let lw = Array1::from(vec![81.0_f64, 84.0_f64]);
        let sp = Array1::from(vec![78.0_f64, 82.0_f64]);
        let pir = Array1::from(vec![80.5_f64, 84.0_f64]);

        // Build spin map expected by constructor
        let mut spin: HashMap<String, Curve> = HashMap::new();
        spin.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on.clone(),
            },
        );
        spin.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw.clone(),
            },
        );
        spin.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp.clone(),
            },
        );
        spin.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir.clone(),
            },
        );

        let sd = ScoreLossData::new(&spin);
        let zero = Array1::zeros(freq.len());

        // Expected preference using score() with zero PEQ (i.e., base curves)
        let intervals = score::octave_intervals(2, &freq);
        let expected = score::score(&freq, &intervals, &on, &lw, &sp, &pir);
        let got = score_loss(&sd, &freq, &zero);
        assert!((got + expected.pref_score).abs() < 1e-12);
    }
}

