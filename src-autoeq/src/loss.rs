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
            panic!(
                "All CEA2034 curves must have the same length. on: {}, lw: {}, sp: {}, pir: {}",
                on.len(),
                lw.len(),
                sp.len(),
                pir.len()
            );
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
    metrics.pref_score
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
    if fmax <= fmin {
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

/// Calculate the standard deviation (SD) of the deviation error over the specified frequency range.
///
/// This function filters the input curve to include only frequencies within the specified range,
/// then calculates the standard deviation of the deviation values.
///
/// # Arguments
/// * `freq` - Frequency array in Hz
/// * `deviation` - Deviation values from Harman target curve in dB
/// * `fmin` - Minimum frequency in Hz (typically 50 Hz)
/// * `fmax` - Maximum frequency in Hz (typically 10000 Hz)
///
/// # Returns
/// * Standard deviation of the deviation in the specified frequency range
///
/// # Notes
/// Used as part of the Olive et al. headphone preference prediction model.
fn calculate_standard_deviation_in_range(
    freq: &Array1<f64>,
    deviation: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> f64 {
    assert_eq!(
        freq.len(),
        deviation.len(),
        "freq and deviation must have same length"
    );

    let mut values = Vec::new();

    // Collect deviation values in the specified frequency range
    for i in 0..freq.len() {
        let f = freq[i];
        if f >= fmin && f <= fmax {
            values.push(deviation[i]);
        }
    }

    if values.is_empty() {
        return 0.0;
    }

    // Calculate mean
    let mean = values.iter().sum::<f64>() / values.len() as f64;

    // Calculate variance
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

    // Return standard deviation
    variance.sqrt()
}

/// Calculate the absolute slope (AS) of the deviation using logarithmic regression over the specified frequency range.
///
/// This function performs linear regression of deviation against log2(frequency) to determine
/// the slope, then returns the absolute value.
///
/// # Arguments
/// * `freq` - Frequency array in Hz
/// * `deviation` - Deviation values from Harman target curve in dB
/// * `fmin` - Minimum frequency in Hz (typically 50 Hz)
/// * `fmax` - Maximum frequency in Hz (typically 10000 Hz)
///
/// # Returns
/// * Absolute value of the slope in dB per octave
///
/// # Notes
/// Used as part of the Olive et al. headphone preference prediction model.
fn calculate_absolute_slope_in_range(
    freq: &Array1<f64>,
    deviation: &Array1<f64>,
    fmin: f64,
    fmax: f64,
) -> f64 {
    match regression_slope_per_octave_in_range(freq, deviation, fmin, fmax) {
        Some(slope) => slope.abs(),
        None => 0.0,
    }
}

/// Compute headphone preference score based on frequency response deviations
///
/// This implements the headphone preference prediction model from:
/// Olive, S. E., Welti, T., & McMullin, E. (2013). "A Statistical Model that
/// Predicts Listeners' Preference Ratings of Around-Ear and On-Ear Headphones"
///
/// The model predicts preference using the equation:
/// **Predicted Preference Rating = 114.49 - (12.62 × SD) - (15.52 × AS)**
///
/// Where:
/// - **SD** = Standard deviation of the deviation error over 50 Hz to 10 kHz
/// - **AS** = Absolute value of slope of the deviation over 50 Hz to 10 kHz
///
/// # Arguments
/// * `curve` - Frequency response curve representing deviation from Harman AE/OE target
///
/// # Returns
/// * Predicted preference rating (higher values indicate better preference)
/// * For optimization purposes, return -preference_rating so minimizing improves preference
///
/// # Important Note
/// The input curve should represent deviation from the Harman Around-Ear (AE) or
/// On-Ear (OE) target curve, **not** deviation from flat or neutral response.
///
/// The frequency range for calculations is 50 Hz to 10 kHz as specified in the paper.
///
/// # References
/// Olive, S. E., Welti, T., & McMullin, E. (2013). "A Statistical Model that
/// Predicts Listeners' Preference Ratings of Around-Ear and On-Ear Headphones".
/// Presented at the 135th Convention of the Audio Engineering Society.
pub fn headphone_loss(curve: &Curve) -> f64 {
    let freq = &curve.freq;
    let deviation = &curve.spl;

    // Define frequency range for analysis (50 Hz to 10 kHz per paper)
    const FMIN: f64 = 50.0;
    const FMAX: f64 = 10000.0;

    // Calculate SD (Standard Deviation) of the deviation error
    let sd = calculate_standard_deviation_in_range(freq, deviation, FMIN, FMAX);

    // Calculate AS (Absolute Slope) of the deviation
    let as_value = calculate_absolute_slope_in_range(freq, deviation, FMIN, FMAX);

    // Apply the Olive et al. equation (Equation 4 from the paper)
    // Predicted Preference Rating = 114.49 - (12.62 × SD) - (15.52 × AS)
    let predicted_preference_rating = 114.49 - (12.62 * sd) - (15.52 * as_value);

    // Return negative preference rating for minimization during optimization
    // (minimizing the loss function maximizes the preference rating)
    predicted_preference_rating
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

    let input_curve = read::normalize_and_interpolate_response(&freqs, response);
    let target_curve = read::normalize_and_interpolate_response(&freqs, target);

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
    fn test_calculate_standard_deviation_in_range() {
        // Test SD calculation with known values
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]); // All in range

        let sd = calculate_standard_deviation_in_range(&freq, &deviation, 50.0, 10000.0);

        // Manual calculation: mean = (1+2+3+4+5)/5 = 3.0
        // variance = ((1-3)² + (2-3)² + (3-3)² + (4-3)² + (5-3)²)/5 = (4+1+0+1+4)/5 = 2.0
        // sd = sqrt(2.0) ≈ 1.414
        let expected_sd = 2.0_f64.sqrt();
        assert!(
            (sd - expected_sd).abs() < 1e-12,
            "SD calculation incorrect: got {}, expected {}",
            sd,
            expected_sd
        );
    }

    #[test]
    fn test_calculate_standard_deviation_filtered_range() {
        // Test SD calculation with frequency filtering
        let freq = Array1::from(vec![20.0, 100.0, 1000.0, 5000.0, 15000.0]); // Some out of range
        let deviation = Array1::from(vec![10.0, 2.0, 4.0, 6.0, 20.0]); // First and last should be filtered

        let sd = calculate_standard_deviation_in_range(&freq, &deviation, 50.0, 10000.0);

        // Only values at 100Hz, 1kHz, 5kHz should be included: [2.0, 4.0, 6.0]
        // mean = (2+4+6)/3 = 4.0
        // variance = ((2-4)² + (4-4)² + (6-4)²)/3 = (4+0+4)/3 = 8/3
        // sd = sqrt(8/3) ≈ 1.633
        let expected_sd = (8.0_f64 / 3.0_f64).sqrt();
        assert!(
            (sd - expected_sd).abs() < 1e-12,
            "SD calculation with filtering incorrect: got {}, expected {}",
            sd,
            expected_sd
        );
    }

    #[test]
    fn test_calculate_absolute_slope_in_range() {
        // Test AS calculation with linear slope
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a perfect 2 dB/octave slope: y = 2 * log2(f) + constant
        let deviation = freq.mapv(|f: f64| 2.0 * f.log2());

        let as_value = calculate_absolute_slope_in_range(&freq, &deviation, 50.0, 10000.0);

        // Should return absolute value of 2.0
        assert!(
            (as_value - 2.0).abs() < 1e-12,
            "AS calculation incorrect: got {}, expected 2.0",
            as_value
        );
    }

    #[test]
    fn test_calculate_absolute_slope_negative() {
        // Test AS calculation with negative slope
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a perfect -3 dB/octave slope
        let deviation = freq.mapv(|f: f64| -3.0 * f.log2());

        let as_value = calculate_absolute_slope_in_range(&freq, &deviation, 50.0, 10000.0);

        // Should return absolute value of -3.0 = 3.0
        assert!(
            (as_value - 3.0).abs() < 1e-12,
            "AS calculation with negative slope incorrect: got {}, expected 3.0",
            as_value
        );
    }

    #[test]
    fn test_headphone_loss_perfect_harman_deviation() {
        // Test with zero deviation from Harman target (perfect response)
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::zeros(5); // Perfect match to Harman target

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
        };
        let score = headphone_loss(&curve);

        // With zero deviation (SD=0, AS=0), predicted preference = 114.49
        // Function returns negative preference for minimization
        let expected_score = -114.49;
        assert!(
            (score - expected_score).abs() < 1e-12,
            "Perfect Harman score incorrect: got {}, expected {}",
            score,
            expected_score
        );
    }

    #[test]
    fn test_headphone_loss_with_deviation() {
        // Test with some deviation from Harman target
        let freq = Array1::from(vec![50.0, 100.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 1.0]); // 1dB constant deviation

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
        };
        let score = headphone_loss(&curve);

        // SD = 0 (constant deviation), AS = 0 (flat slope)
        // predicted preference = 114.49 - (12.62 * 0) - (15.52 * 0) = 114.49
        // But wait - SD should be 0 for constant values, but AS should also be 0
        let expected_preference = 114.49;
        let expected_score = -expected_preference;
        assert!(
            (score - expected_score).abs() < 1e-10,
            "Constant deviation score incorrect: got {}, expected {}",
            score,
            expected_score
        );
    }

    #[test]
    fn test_headphone_loss_with_slope() {
        // Test with a sloped deviation
        let freq = Array1::from(vec![
            50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 10000.0,
        ]);
        // Create a 1 dB/octave slope in the deviation
        let deviation = freq.mapv(|f: f64| 1.0 * f.log2());

        let curve = Curve {
            freq: freq.clone(),
            spl: deviation,
        };
        let score = headphone_loss(&curve);

        // AS = 1.0 (absolute slope)
        // SD will be non-zero due to the slope
        // predicted preference = 114.49 - (12.62 * SD) - (15.52 * 1.0)
        // Score should be worse (more negative) than perfect case (-114.49)
        assert!(
            score < -50.0,
            "Sloped deviation should have lower preference: got {}",
            score
        );
    }

    #[test]
    fn test_headphone_loss_with_target() {
        // Test the target curve variant with zero deviation
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

        // When response matches target, deviation is zero, so should get perfect score
        let expected_perfect_score = -114.49;
        assert!(
            (score - expected_perfect_score).abs() < 1e-10,
            "Perfect target match score incorrect: got {}, expected {}",
            score,
            expected_perfect_score
        );
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
