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
use crate::read;
use crate::Curve;
use clap::ValueEnum;
use ndarray::Array1;
use std::collections::HashMap;

/// The type of loss function to use during optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LossType {
    /// Flat loss function (minimize deviation from target curve)
    SpeakerFlat,
    /// Harmann/Olive Score-based loss function (maximize preference score)
    SpeakerScore,
    /// Flat loss function (minimize deviation from target curve)
    HeadphoneFlat,
    /// Harmann/Olive Score-based loss function (maximize preference score)
    HeadphoneScore,
}

/// Data required for computing speaker score-based loss
#[derive(Debug, Clone)]
pub struct SpeakerLossData {
    /// On-axis SPL measurements
    pub on: Array1<f64>,
    /// Listening window SPL measurements
    pub lw: Array1<f64>,
    /// Sound power SPL measurements
    pub sp: Array1<f64>,
    /// Predicted in-room SPL measurements
    pub pir: Array1<f64>,
}

impl SpeakerLossData {
    /// Create a new SpeakerLossData instance
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
            
        // Verify all arrays have the same length
        if on.len() != lw.len() || on.len() != sp.len() || on.len() != pir.len() {
            panic!("All CEA2034 curves must have the same length. on: {}, lw: {}, sp: {}, pir: {}", 
                   on.len(), lw.len(), sp.len(), pir.len());
        }
        
        Self { on, lw, sp, pir }
    }
}

/// Data required for computing headphone loss
#[derive(Debug, Clone)]
pub struct HeadphoneLossData {
    /// Enable smoothing (regularization) of the inverted target curve
    pub smooth: bool,
    /// Smoothing level as 1/N octave (N in [1..24])
    pub smooth_n: usize,
}

impl HeadphoneLossData {
    /// Create a new HeadphoneLossData instance
    ///
    /// # Arguments
    /// * `smooth` - Enable smoothing
    /// * `smooth_n` - Smoothing level as 1/N octave
    pub fn new(smooth: bool, smooth_n: usize) -> Self {
        Self { smooth, smooth_n }
    }
}

/// Compute the flat (current) loss
pub fn flat_loss(freqs: &Array1<f64>, error: &Array1<f64>) -> f64 {
    weighted_mse(freqs, error)
}

/// Compute the score-based loss. Returns -pref_score so that minimizing it maximizes the preference score.
/// `peq_response` must be computed for the candidate parameters.
pub fn speaker_score_loss(
    score_data: &SpeakerLossData,
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
    score_data: &SpeakerLossData,
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

/// Compute headphone preference score based on frequency response deviations
///
/// This implements the headphone preference prediction model from:
/// "A Statistical Model that Predicts Listeners' Preference Ratings of
/// In-Ear Headphones" by Sean Olive et al.
///
/// The model predicts preference based on:
/// - Slope of the response (target: -1 dB/octave from 20Hz-10kHz)
/// - RMS deviation in different frequency bands
/// - Peak-to-peak variations
///
/// # Arguments
/// * `freq` - Frequency points in Hz
/// * `response` - Frequency response in dB (deviation from target)
///
/// # Returns
/// * Score value where lower is better (for minimization)
///
/// # References
/// Olive, S. E., Welti, T., & McMullin, E. (2013). "A Statistical Model that
/// Predicts Listeners' Preference Ratings of In-Ear Headphones: Part 2 –
/// Development and Validation of the Model"
pub fn headphone_loss(curve: &Curve) -> f64 {
    let freq = curve.freq.clone();
    let response = curve.spl.clone();

    // Define frequency bands for analysis
    const BAND_LIMITS: [(f64, f64); 10] = [
        (20.0, 60.0),       // Sub-bass
        (60.0, 200.0),      // Bass
        (200.0, 500.0),     // Lower midrange
        (500.0, 1000.0),    // Midrange
        (1000.0, 2000.0),   // Upper midrange
        (2000.0, 4000.0),   // Presence
        (4000.0, 8000.0),   // Brilliance
        (8000.0, 10000.0),  // Upper treble
        (10000.0, 12000.0), // Air
        (12000.0, 20000.0), // Ultra-high
    ];

    // Calculate slope (should be close to -1 dB/octave for headphones)
    let slope =
        regression_slope_per_octave_in_range(&freq, &response, 20.0, 10000.0).unwrap_or(0.0);

    // Target slope is -1 dB/octave (gentle downward tilt)
    let slope_deviation = (slope + 1.0).abs();

    // Calculate RMS and peak-to-peak in each band
    let mut band_rms = Vec::new();
    let mut band_peak_to_peak = Vec::new();

    for (f_low, f_high) in BAND_LIMITS.iter() {
        let mut values = Vec::new();

        // Collect values in this band
        for i in 0..freq.len() {
            if freq[i] >= *f_low && freq[i] <= *f_high {
                values.push(response[i]);
            }
        }

        if !values.is_empty() {
            // RMS of deviations
            let sum_sq: f64 = values.iter().map(|x| x * x).sum();
            let rms = (sum_sq / values.len() as f64).sqrt();
            band_rms.push(rms);

            // Peak-to-peak variation
            let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            band_peak_to_peak.push(max - min);
        }
    }

    // Weighted combination of metrics (weights from the paper)
    // Note: These weights are approximations based on the paper's findings
    let mut score = 0.0;

    // Slope term (high weight - critical for naturalness)
    score += 10.0 * slope_deviation;

    // RMS deviations with frequency-dependent weighting
    // Bass and midrange deviations are more critical
    if band_rms.len() >= 6 {
        score += 3.0 * band_rms[0]; // Sub-bass (20-60 Hz)
        score += 4.0 * band_rms[1]; // Bass (60-200 Hz)
        score += 5.0 * band_rms[2]; // Lower mid (200-500 Hz)
        score += 5.0 * band_rms[3]; // Midrange (500-1k Hz)
        score += 3.0 * band_rms[4]; // Upper mid (1k-2k Hz)
        score += 2.0 * band_rms[5]; // Presence (2k-4k Hz)

        // High frequency bands (less critical but still important)
        if band_rms.len() > 6 {
            for i in 6..band_rms.len() {
                score += 1.5 * band_rms[i];
            }
        }
    }

    // Peak-to-peak penalty (excessive variation is bad)
    for pp in &band_peak_to_peak {
        if *pp > 6.0 {
            // Penalize variations > 6dB
            score += 0.5 * (*pp - 6.0);
        }
    }

    // Return score (lower is better for optimization)
    score
}

/// Compute headphone preference score with additional target curve
///
/// # Arguments
/// * `data` - Headphone loss data containing smoothing parameters
/// * `response` - Measured frequency response in dB
/// * `target` - Target frequency response in dB
///
/// # Returns
/// * Score value where lower is better (for minimization)
pub fn headphone_loss_with_target(
    data: &HeadphoneLossData,
    response: &Curve,
    target: &Curve,
) -> f64 {
    // freqs on which we normalize every curve: 12 points per octave between 20 and 20kHz
    let freqs = read::create_log_frequency_grid(10 * 12, 20.0, 20000.0);

    let input_curve = read::normalize_and_interpolate_response(&freqs, &response);
    let target_curve = read::normalize_and_interpolate_response(&freqs, &target);

    // normalized and potentially smooth
    let deviation = Curve {
        freq: freqs.clone(),
        spl: &target_curve.spl - &input_curve.spl,
    };
    let smooth_deviation = if data.smooth {
        read::smooth_one_over_n_octave(&deviation, data.smooth_n)
    } else {
        deviation.clone()
    };

    headphone_loss(&smooth_deviation)
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

        let sd = SpeakerLossData::new(&spin);
        let zero = Array1::zeros(freq.len());

        // Expected preference using score() with zero PEQ (i.e., base curves)
        let intervals = super::score::octave_intervals(2, &freq);
        let expected = super::score::score(&freq, &intervals, &on, &lw, &sp, &pir);
        let got = speaker_score_loss(&sd, &freq, &zero);
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
    fn test_headphone_loss_flat_response() {
        // Test that a perfectly flat response (all zeros) gives a low score
        let freq = Array1::logspace(10.0, 1.301, 4.301, 100); // 20Hz to 20kHz
        let response = Array1::zeros(100); // Perfectly flat
        let curve = Curve {
            freq: freq.clone(),
            spl: response,
        };
        let score = headphone_loss(&curve);

        // With flat response, only slope deviation contributes
        // Target slope is -1 dB/octave, flat is 0, so deviation is 1
        // Score should be around 10.0 * 1.0 = 10.0
        assert!(
            score > 9.0 && score < 11.0,
            "Flat response score: {}",
            score
        );
    }

    #[test]
    fn test_headphone_loss_ideal_slope() {
        // Test response with ideal -1 dB/octave slope
        let freq = Array1::logspace(10.0, 1.301, 4.301, 100); // 20Hz to 20kHz
                                                              // Create response with -1 dB/octave slope
        let response = freq.mapv(|f: f64| -1.0 * f.log2() + 10.0);

        let curve = Curve {
            freq: freq.clone(),
            spl: response,
        };
        let score = headphone_loss(&curve);

        // With ideal slope, score should be lower than flat but still has RMS from the slope
        // The -1 dB/octave slope creates variation in each band
        assert!(score < 70.0, "Ideal slope score too high: {}", score);
    }

    #[test]
    fn test_headphone_loss_with_peaks() {
        // Test with a response that has a peak
        let freq = Array1::logspace(10.0, 1.301, 4.301, 100);
        let mut response = Array1::zeros(100);

        // Add a 5dB peak around 1kHz
        for i in 0..100 {
            if freq[i] > 800.0 && freq[i] < 1200.0 {
                response[i] = 5.0;
            }
        }

        let curve = Curve {
            freq: freq.clone(),
            spl: response,
        };
        let score_with_peak = headphone_loss(&curve);
        let curve = Curve {
            freq: freq.clone(),
            spl: Array1::zeros(100),
        };
        let score_flat = headphone_loss(&curve);

        // Score with peak should be significantly higher
        assert!(
            score_with_peak > score_flat + 20.0,
            "Peak not sufficiently penalized: {} vs {}",
            score_with_peak,
            score_flat
        );
    }

    #[test]
    fn test_headphone_loss_with_target() {
        // Test the target curve variant
        let freq = Array1::logspace(10.0, 1.301, 4.301, 100);
        let response = Array1::from_elem(100, 5.0); // Constant 5dB response
        let target = Array1::from_elem(100, 5.0); // Same as response

        let response_curve = Curve {
            freq: freq.clone(),
            spl: response,
        };
        let target_curve = Curve {
            freq: freq.clone(),
            spl: target,
        };
        let data = HeadphoneLossData::new(false, 2);
        let score = headphone_loss_with_target(&data, &response_curve, &target_curve);

        // When response matches target, deviation is zero (flat)
        // Score should be same as flat response test
        assert!(score > 9.0 && score < 11.0, "Target match score: {}", score);
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

        let sd = SpeakerLossData::new(&spin);
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
