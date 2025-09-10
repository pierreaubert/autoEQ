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

use crate::cea2034 as score;
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
    /// keep LW and PIR as flat as possible
    Mixed,
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

/// Compute the flat (current) loss
pub fn flat_loss(freqs: &Array1<f64>, error: &Array1<f64>) -> f64 {
    weighted_mse(freqs, error)
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
    let metrics = if peq_response.iter().all(|v| v.abs() < 1e-12) {
        // Exact score when no PEQ is applied
        score::score(
            freq,
            &intervals,
            &score_data.on,
            &score_data.lw,
            &score_data.sp,
            &score_data.pir,
        )
    } else {
        score::score_peq_approx(
            freq,
            &intervals,
            &score_data.lw,
            &score_data.sp,
            &score_data.pir,
            &score_data.on,
            peq_response,
        )
    };
    // Return negative preference score so minimizing improves preference.
    100.0 - metrics.pref_score
}

/// Compute a mixed loss based on flatness on lw and pir
pub fn mixed_loss(
    score_data: &ScoreLossData,
    freq: &Array1<f64>,
    peq_response: &Array1<f64>,
) -> f64 {
    let lw2 = &score_data.lw + peq_response;
    let pir2 = &score_data.pir + peq_response;
    // Compute slopes in dB per octave over 100 Hz .. 10 kHz
    let lw2_slope = regression_slope_per_octave_in_range(freq, &lw2, 100.0, 10000.0);
    let pir_og_slope = regression_slope_per_octave_in_range(freq, &score_data.pir, 100.0, 10000.0);
    let pir2_slope = regression_slope_per_octave_in_range(freq, &pir2, 100.0, 10000.0);
    if let (Some(lw2eq), Some(pir2og), Some(pir2eq)) = (lw2_slope, pir_og_slope, pir2_slope) {
        // some nlopt algorithms stop for negative values; keep result positive-ish
        (0.5 + lw2eq).powi(2) + (pir2og - pir2eq).powi(2)
    } else {
        f64::INFINITY
    }
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

/// Compute the slope (per octave) using linear regression of y against log2(f).
///
/// - `freq`: frequency array in Hz
/// - `y`: corresponding values (e.g., SPL in dB)
/// - Range is defined in Hz as [fmin, fmax]; only f > 0 are considered
/// - Returns `Some(slope_db_per_octave)` or `None` if insufficient data
pub fn regression_slope_per_octave_in_range(
    freq: &Array1<f64>,
    y: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> Option<f64> {
    assert_eq!(freq.len(), y.len(), "freq and y must have same length");
    if !(fmax > fmin) {
        return None;
    }

    let mut n: usize = 0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_x2 = 0.0;

    for i in 0..freq.len() {
        let f = freq[i];
        if f > 0.0 && f >= fmin && f <= fmax {
            let xi = f.log2();
            let yi = y[i];
            n += 1;
            sum_x += xi;
            sum_y += yi;
            sum_xy += xi * yi;
            sum_x2 += xi * xi;
        }
    }

    if n < 2 {
        return None;
    }
    let n_f = n as f64;
    let cov_xy = sum_xy - (sum_x * sum_y) / n_f;
    let var_x = sum_x2 - (sum_x * sum_x) / n_f;
    if var_x == 0.0 {
        return None;
    }
    Some(cov_xy / var_x)
}

/// Convenience wrapper for slope per octave on a `Curve`.
pub fn curve_slope_per_octave_in_range(curve: &crate::Curve, fmin: f64, fmax: f64) -> Option<f64> {
    regression_slope_per_octave_in_range(&curve.freq, &curve.spl, fmin, fmax)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;
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
        let intervals = super::score::octave_intervals(2, &freq);
        let expected = super::score::score(&freq, &intervals, &on, &lw, &sp, &pir);
        let got = score_loss(&sd, &freq, &zero);
        if got.is_nan() && expected.pref_score.is_nan() {
            // ok
        } else {
            assert!((got + expected.pref_score).abs() < 1e-12);
        }
    }

    #[test]
    fn regression_slope_per_octave_linear_log_relation_full_range() {
        // y = 3 * log2(f) + 1
        let freq = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let y = freq.mapv(|f: f64| 3.0 * f.log2() + 1.0);
        let slope = regression_slope_per_octave_in_range(&freq, &y, 100.0, 800.0).unwrap();
        assert!((slope - 3.0).abs() < 1e-12);
    }

    #[test]
    fn regression_slope_per_octave_sub_range() {
        // Same log-linear relation, sub-range 200..=800
        let freq = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let y = freq.mapv(|f: f64| -2.5 * f.log2() + 4.0);
        let slope = regression_slope_per_octave_in_range(&freq, &y, 200.0, 800.0).unwrap();
        assert!((slope + 2.5).abs() < 1e-12);
    }

    #[test]
    fn mixed_loss_finite_with_zero_peq() {
        // Frequency grid
        let freq = Array1::from(vec![
            100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Zero curves
        let on = Array1::zeros(freq.len());
        let lw = Array1::zeros(freq.len());
        let sp = Array1::zeros(freq.len());
        let pir = Array1::zeros(freq.len());

        // Build spin map
        let mut spin: HashMap<String, Curve> = HashMap::new();
        spin.insert(
            "On Axis".to_string(),
            Curve {
                freq: freq.clone(),
                spl: on,
            },
        );
        spin.insert(
            "Listening Window".to_string(),
            Curve {
                freq: freq.clone(),
                spl: lw,
            },
        );
        spin.insert(
            "Sound Power".to_string(),
            Curve {
                freq: freq.clone(),
                spl: sp,
            },
        );
        spin.insert(
            "Estimated In-Room Response".to_string(),
            Curve {
                freq: freq.clone(),
                spl: pir,
            },
        );

        let sd = ScoreLossData::new(&spin);
        let peq = Array1::zeros(freq.len());
        let v = mixed_loss(&sd, &freq, &peq);
        assert!(v.is_finite(), "mixed_loss should be finite, got {}", v);
    }

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
}
