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

use ndarray::Array1;
use std::f64::consts::PI;
use std::fmt;

// Helper functions translated from the Python script.
/// Converts bandwidth in octaves to a Q factor.
pub fn bw2q(bw: f64) -> f64 {
    let two_pow_bw = 2.0_f64.powf(bw);
    two_pow_bw.sqrt() / (two_pow_bw - 1.0)
}

/// Converts a Q factor to bandwidth in octaves.
pub fn q2bw(q: f64) -> f64 {
    let q2 = (2.0 * q * q + 1.0) / (2.0 * q * q);
    (q2 + (q2 * q2 - 1.0).sqrt()).log(2.0)
}

// Constants
/// Default Q factor for high/low pass filters
pub const DEFAULT_Q_HIGH_LOW_PASS: f64 = 1.0 / std::f64::consts::SQRT_2;
/// Default Q factor for high/low shelf filters
pub const DEFAULT_Q_HIGH_LOW_SHELF: f64 = 1.0668676536332304; // Value of bw2q(0.9)

/// Sample rate constant (matching Python SRATE)
pub const SRATE: f64 = 48000.0;

/// Type alias for a Parametric EQ - a collection of weighted biquad filters
/// Each tuple contains (weight, biquad_filter)
pub type Peq = Vec<(f64, Biquad)>;

/// Filter types for biquad filters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiquadFilterType {
    /// Low-pass filter
    Lowpass,
    /// High-pass filter
    Highpass,
    /// High-pass filter
    HighpassVariableQ,
    /// Band-pass filter
    Bandpass,
    /// Peaking filter
    Peak,
    /// Notch filter
    Notch,
    /// Low-shelf filter
    Lowshelf,
    /// High-shelf filter
    Highshelf,
}

impl BiquadFilterType {
    /// Returns the short string representation of the filter type (e.g., "LP").
    pub fn short_name(&self) -> &'static str {
        match self {
            BiquadFilterType::Lowpass => "LP",
            BiquadFilterType::Highpass => "HP",
            BiquadFilterType::HighpassVariableQ => "HPQ",
            BiquadFilterType::Bandpass => "BP",
            BiquadFilterType::Peak => "PK",
            BiquadFilterType::Notch => "NO",
            BiquadFilterType::Lowshelf => "LS",
            BiquadFilterType::Highshelf => "HS",
        }
    }

    /// Returns the long string representation of the filter type (e.g., "Lowpass").
    pub fn long_name(&self) -> &'static str {
        match self {
            BiquadFilterType::Lowpass => "Lowpass",
            BiquadFilterType::Highpass => "Highpass",
            BiquadFilterType::HighpassVariableQ => "HighpassVariableQ",
            BiquadFilterType::Bandpass => "Bandpass",
            BiquadFilterType::Peak => "Peak",
            BiquadFilterType::Notch => "Notch",
            BiquadFilterType::Lowshelf => "Lowshelf",
            BiquadFilterType::Highshelf => "Highshelf",
        }
    }
}

/// Represents a single biquad IIR filter.
#[derive(Debug, Clone)]
pub struct Biquad {
    /// The type of filter
    pub filter_type: BiquadFilterType,
    /// Center frequency in Hz
    pub freq: f64,
    /// Sample rate in Hz
    pub srate: f64,
    /// Q factor (quality factor)
    pub q: f64,
    /// Gain in dB (for peaking and shelving filters)
    pub db_gain: f64,
    /// Filter coefficients
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    /// Filter state (for processing samples)
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
    /// Pre-computed coefficients for fast frequency response calculation
    r_up0: f64,
    r_up1: f64,
    r_up2: f64,
    r_dw0: f64,
    r_dw1: f64,
    r_dw2: f64,
}

impl Biquad {
    /// Creates and initializes a new Biquad filter.
    pub fn new(filter_type: BiquadFilterType, freq: f64, srate: f64, q: f64, db_gain: f64) -> Self {
        let mut biquad = Biquad {
            filter_type,
            freq,
            srate,
            q,
            db_gain,
            a1: 0.0,
            a2: 0.0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            r_up0: 0.0,
            r_up1: 0.0,
            r_up2: 0.0,
            r_dw0: 0.0,
            r_dw1: 0.0,
            r_dw2: 0.0,
        };

        // Adjust Q based on filter type, matching Python logic
        if biquad.filter_type == BiquadFilterType::Notch {
            biquad.q = 30.0;
        } else if biquad.q == 0.0 {
            match biquad.filter_type {
                BiquadFilterType::Bandpass
                | BiquadFilterType::Highpass
                | BiquadFilterType::Lowpass => {
                    biquad.q = DEFAULT_Q_HIGH_LOW_PASS;
                }
                BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => {
                    biquad.q = DEFAULT_Q_HIGH_LOW_SHELF;
                }
                _ => {}
            }
        }

        // Safety clamp: ensure strictly positive Q to avoid division by zero in alpha = sn/(2*q)
        if biquad.q <= 0.0 {
            biquad.q = 1.0e-2;
        }

        biquad.compute_coeffs();
        biquad
    }

    fn compute_coeffs(&mut self) {
        // Intermediate variables
        let a = 10.0_f64.powf(self.db_gain / 40.0);
        let omega = 2.0 * PI * self.freq / self.srate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * self.q);
        let beta = (a + a).sqrt();

        // Raw coefficients
        let (b0, b1, b2, a0, a1, a2);

        match self.filter_type {
            BiquadFilterType::Lowpass => {
                b0 = (1.0 - cs) / 2.0;
                b1 = 1.0 - cs;
                b2 = (1.0 - cs) / 2.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => {
                b0 = (1.0 + cs) / 2.0;
                b1 = -(1.0 + cs);
                b2 = (1.0 + cs) / 2.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Bandpass => {
                b0 = alpha;
                b1 = 0.0;
                b2 = -alpha;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Notch => {
                b0 = 1.0;
                b1 = -2.0 * cs;
                b2 = 1.0;
                a0 = 1.0 + alpha;
                a1 = -2.0 * cs;
                a2 = 1.0 - alpha;
            }
            BiquadFilterType::Peak => {
                b0 = 1.0 + (alpha * a);
                b1 = -2.0 * cs;
                b2 = 1.0 - (alpha * a);
                a0 = 1.0 + (alpha / a);
                a1 = -2.0 * cs;
                a2 = 1.0 - (alpha / a);
            }
            BiquadFilterType::Lowshelf => {
                b0 = a * ((a + 1.0) - (a - 1.0) * cs + beta * sn);
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cs);
                b2 = a * ((a + 1.0) - (a - 1.0) * cs - beta * sn);
                a0 = (a + 1.0) + (a - 1.0) * cs + beta * sn;
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cs);
                a2 = (a + 1.0) + (a - 1.0) * cs - beta * sn;
            }
            BiquadFilterType::Highshelf => {
                b0 = a * ((a + 1.0) + (a - 1.0) * cs + beta * sn);
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cs);
                b2 = a * ((a + 1.0) + (a - 1.0) * cs - beta * sn);
                a0 = (a + 1.0) - (a - 1.0) * cs + beta * sn;
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cs);
                a2 = (a + 1.0) - (a - 1.0) * cs - beta * sn;
            }
        }

        // Normalize coefficients
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;

        // Pre-compute for result()
        self.r_up0 = (self.b0 + self.b1 + self.b2).powi(2);
        self.r_up1 = -4.0 * (self.b0 * self.b1 + 4.0 * self.b0 * self.b2 + self.b1 * self.b2);
        self.r_up2 = 16.0 * self.b0 * self.b2;
        self.r_dw0 = (1.0 + self.a1 + self.a2).powi(2);
        self.r_dw1 = -4.0 * (self.a1 + 4.0 * self.a2 + self.a1 * self.a2);
        self.r_dw2 = 16.0 * self.a2;
    }

    /// Processes a single audio sample through the filter.
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;

        y
    }

    /// Calculates the filter's magnitude response at a single frequency `f`.
    pub fn result(&self, f: f64) -> f64 {
        let phi = (PI * f / self.srate).sin().powi(2);
        let phi2 = phi * phi;

        let numerator = self.r_up0 + self.r_up1 * phi + self.r_up2 * phi2;
        let denominator = self.r_dw0 + self.r_dw1 * phi + self.r_dw2 * phi2;

        let result = (numerator / denominator).max(0.0);
        result.sqrt()
    }

    /// Calculates the filter's response in dB at a single frequency `f`.
    pub fn log_result(&self, f: f64) -> f64 {
        let result = self.result(f);
        if result > 0.0 {
            20.0 * result.log10()
        } else {
            -200.0 // Return a large negative number for silence
        }
    }

    /// Vectorized version to compute the SPL response for a vector of frequencies.
    /// This is the fast equivalent of the `np_log_result` Python method.
    pub fn np_log_result(&self, freq: &Array1<f64>) -> Array1<f64> {
        let coeff = PI / self.srate;
        let phi = (freq * coeff).mapv(f64::sin).mapv(|x| x.powi(2));
        let phi2 = &phi * &phi;

        let r_up = self.r_up0 + self.r_up1 * &phi + self.r_up2 * &phi2;
        let r_dw = self.r_dw0 + self.r_dw1 * &phi + self.r_dw2 * &phi2;
        let r = r_up / r_dw;

        // Clip to a minimum value to avoid log(0), then calculate dB
        let min_val = 1.0e-20;

        r.mapv(|val| val.max(min_val))
            .mapv(f64::sqrt)
            .mapv(f64::log10)
            * 20.0
    }

    /// Returns the filter coefficients as a tuple.
    pub fn constants(&self) -> (f64, f64, f64, f64, f64) {
        (self.a1, self.a2, self.b0, self.b1, self.b2)
    }
}

/// Implement the Display trait for pretty-printing, similar to __str__.
impl fmt::Display for Biquad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Type:{},Freq:{:.1},Rate:{:.1},Q:{:.1},Gain:{:.1}",
            self.filter_type.short_name(),
            self.freq,
            self.srate,
            self.q,
            self.db_gain
        )
    }
}

/// Represents a single filter in a parametric equalizer.
#[derive(Debug, Clone, Default)]
///
/// Center frequency in Hz
/// Q factor (quality factor)
/// Gain in dB
/// Type of filter (e.g., "PK", "LP", "HP")
pub struct FilterRow {
    /// Center frequency in Hz
    pub freq: f64,
    /// Q factor (quality factor)
    pub q: f64,
    /// Gain in dB
    pub gain: f64,
    /// Type of filter (e.g., "PK", "LP", "HP")
    pub kind: &'static str,
}

/// Compute the combined PEQ response (in dB) on a given frequency grid for the current params.
///
/// The parameter vector `x` is laid out as triplets per filter: `[f0, Q, gain, f0, Q, gain, ...]`.
/// If `iir_hp_pk` is true, the lowest-frequency filter is treated as a Highpass, others as Peak.
pub fn compute_peq_response(
    freqs: &Array1<f64>,
    x: &[f64],
    sample_rate: f64,
    iir_hp_pk: bool,
) -> Array1<f64> {
    let n = x.len() / 3;
    if n == 0 {
        return Array1::zeros(freqs.len());
    }
    let mut peq = Array1::zeros(freqs.len());
    for i in 0..n {
        let f0 = 10f64.powf(x[i * 3]);
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];
        let ftype = if iir_hp_pk && i == 0 {
            BiquadFilterType::HighpassVariableQ
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, f0, sample_rate, q, gain);
        peq += &filter.np_log_result(freqs);
    }
    peq
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn test_bw_q_roundtrip() {
        let qs = [0.5, 1.0, 2.0, 5.0];
        for &q in &qs {
            let bw = q2bw(q);
            let q2 = bw2q(bw);
            assert!(
                approx_eq(q, q2, 1e-9),
                "roundtrip failed: q={} -> bw={} -> q2={}",
                q,
                bw,
                q2
            );
        }
    }

    #[test]
    fn test_biquad_np_log_result_is_finite() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 1.0, 6.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = bq.np_log_result(&freqs);
        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }

    #[test]
    fn peak_with_zero_q_is_safely_clamped() {
        // q==0 for Peak should be clamped internally to a small positive value
        let bq = Biquad::new(BiquadFilterType::Peak, 1_000.0, 48_000.0, 0.0, 3.0);
        let freqs = array![20.0, 100.0, 1_000.0, 10_000.0, 20_000.0];
        let resp = bq.np_log_result(&freqs);
        for (i, v) in resp.iter().enumerate() {
            assert!(v.is_finite(), "response at idx {} not finite: {}", i, v);
        }
    }
}
#[cfg(test)]
mod peq_response_tests {
    use super::compute_peq_response;
    use ndarray::array;

    #[test]
    fn zero_filters_returns_zero() {
        let freqs = array![100.0, 1000.0, 10000.0];
        let peq = compute_peq_response(&freqs, &[], 48_000.0, false);
        for v in peq.iter() {
            assert!(v.abs() < 1e-12);
        }
    }

    #[test]
    fn one_peak_is_finite() {
        let freqs = array![100.0, 1000.0, 10000.0];
        let x = vec![1000.0_f64.log10(), 1.0, 6.0];
        let peq = compute_peq_response(&freqs, &x, 48_000.0, false);
        for v in peq.iter() {
            assert!(v.is_finite());
        }
    }
}

/// Build a vector of sorted filter rows from optimization parameters
///
/// # Arguments
/// * `x` - Slice of optimization parameters laid out as [f0, Q, gain, f0, Q, gain, ...]
/// * `iir_hp_pk` - If true, treat the lowest-frequency filter as a Highpass filter
///
/// # Returns
/// * Vector of FilterRow structs sorted by frequency
///
/// # Details
/// Converts the flat parameter vector into a vector of FilterRow structs,
/// sorts them by frequency, and optionally marks the lowest-frequency filter
/// as a Highpass filter for display purposes.
pub fn build_sorted_filters(x: &[f64], iir_hp_pk: bool) -> Vec<FilterRow> {
    let mut rows: Vec<FilterRow> = Vec::with_capacity(x.len() / 3);
    for i in 0..(x.len() / 3) {
        let freq = 10f64.powf(x[i * 3]);
        let q = x[i * 3 + 1];
        let gain = x[i * 3 + 2];
        rows.push(FilterRow {
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

    // If enabled, mark the lowest-frequency filter as Highpass for display purposes
    if iir_hp_pk && !rows.is_empty() {
        rows[0].kind = "HPQ";
        rows[0].gain = 0.0;
    }
    rows
}

/// Check if two PEQs are equal
/// 
/// Compares two PEQ vectors for equality, checking both weights and biquad parameters
pub fn peq_equal(left: &Peq, right: &Peq) -> bool {
    if left.len() != right.len() {
        return false;
    }
    
    left.iter().zip(right.iter()).all(|((w1, b1), (w2, b2))| {
        // Compare weights
        (w1 - w2).abs() < f64::EPSILON &&
        // Compare biquad parameters 
        b1.filter_type == b2.filter_type &&
        (b1.freq - b2.freq).abs() < f64::EPSILON &&
        (b1.srate - b2.srate).abs() < f64::EPSILON &&
        (b1.q - b2.q).abs() < f64::EPSILON &&
        (b1.db_gain - b2.db_gain).abs() < f64::EPSILON
    })
}

/// Compute SPL for each frequency given a PEQ
/// 
/// # Arguments
/// * `freq` - Array of frequencies to compute response for
/// * `peq` - PEQ vector containing weighted biquad filters
/// 
/// # Returns
/// * Array of SPL values in dB for each frequency
pub fn peq_spl(freq: &Array1<f64>, peq: &Peq) -> Array1<f64> {
    let mut current_filter = Array1::zeros(freq.len());
    
    for (weight, iir) in peq {
        current_filter += &(iir.np_log_result(freq) * *weight);
    }
    
    current_filter
}

/// Compute preamp gain for a PEQ: well adapted to computers
/// 
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
/// 
/// # Returns
/// * Preamp gain in dB (negative value to prevent clipping)
pub fn peq_preamp_gain(peq: &Peq) -> f64 {
    // Generate logarithmic frequency array from 20Hz to 20kHz with 1000 points
    let freq = Array1::logspace(10.0, (2.0f64 * 10.0).log10(), (2.0f64 * 10000.0).log10(), 1000);
    let spl = peq_spl(&freq, peq);
    
    // Find maximum positive gain and return its negative
    let overall = spl.iter().cloned().fold(0.0f64, |acc, x| acc.max(x.max(0.0)));
    -overall
}

/// Compute preamp gain for a PEQ and look at the worst case
/// 
/// Note that we add 0.2 dB to have a margin for clipping
/// 
/// # Arguments
/// * `peq` - PEQ vector containing weighted biquad filters
/// 
/// # Returns
/// * Preamp gain in dB (negative value to prevent clipping)
pub fn peq_preamp_gain_max(peq: &Peq) -> f64 {
    if peq.is_empty() {
        return 0.0;
    }
    
    // Generate logarithmic frequency array from 20Hz to 20kHz with 1000 points 
    let freq = Array1::logspace(10.0, (2.0f64 * 10.0).log10(), (2.0f64 * 10000.0).log10(), 1000);
    let spl = peq_spl(&freq, peq);
    
    // Find maximum individual filter contribution
    let mut individual: f64 = 0.0;
    for (_, iir) in peq {
        let single_peq = vec![(1.0, iir.clone())];
        let single_spl = peq_spl(&freq, &single_peq);
        let single_max = single_spl.iter().cloned().fold(0.0f64, |acc, x| acc.max(x));
        individual = individual.max(single_max);
    }
    
    // Find overall maximum positive gain
    let overall = spl.iter().cloned().fold(0.0f64, |acc, x| acc.max(x.max(0.0)));
    
    // Take worst case and add safety margin
    -(individual.max(overall) + 0.2)
}

/// Format PEQ as APO configuration string
/// 
/// # Arguments
/// * `comment` - Comment string to include at the top
/// * `peq` - PEQ vector containing weighted biquad filters
/// 
/// # Returns
/// * String formatted for EqualizerAPO
pub fn peq_format_apo(comment: &str, peq: &Peq) -> String {
    let mut res = Vec::new();
    res.push(comment.to_string());
    res.push(format!("Preamp: {:.1} dB", peq_preamp_gain(peq)));
    res.push(String::new());
    
    for (i, (_, iir)) in peq.iter().enumerate() {
        match iir.filter_type {
            BiquadFilterType::Peak | BiquadFilterType::Notch | BiquadFilterType::Bandpass => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:0.2}",
                    i + 1,
                    iir.filter_type.short_name(),
                    iir.freq as i32,
                    iir.db_gain,
                    iir.q
                ));
            }
            BiquadFilterType::Lowpass | BiquadFilterType::Highpass => {
                if (iir.q - DEFAULT_Q_HIGH_LOW_PASS).abs() < f64::EPSILON {
                    res.push(format!(
                        "Filter {:2}: ON {:2} Fc {:5} Hz",
                        i + 1,
                        iir.filter_type.short_name(),
                        iir.freq as i32
                    ));
                } else {
                    res.push(format!(
                        "Filter {:2}: ON {:2}Q Fc {:5} Hz Q {:0.2}",
                        i + 1,
                        iir.filter_type.short_name(),
                        iir.freq as i32,
                        iir.q
                    ));
                }
            }
            BiquadFilterType::Lowshelf | BiquadFilterType::Highshelf => {
                res.push(format!(
                    "Filter {:2}: ON {:2} Fc {:5} Hz Gain {:+0.2} dB Q {:.2}",
                    i + 1,
                    iir.filter_type.short_name(),
                    iir.freq as i32,
                    iir.db_gain,
                    iir.q
                ));
            }
            BiquadFilterType::HighpassVariableQ => {
                res.push(format!(
                    "Filter {:2}: ON HPQ Fc {:5} Hz Q {:0.2}",
                    i + 1,
                    iir.freq as i32,
                    iir.q
                ));
            }
        }
    }
    
    res.push(String::new());
    res.join("\n")
}

/// Compute Q values for Butterworth filters
/// 
/// # Arguments
/// * `order` - Filter order
/// 
/// # Returns
/// * Vector of Q values for each biquad section
pub fn peq_butterworth_q(order: usize) -> Vec<f64> {
    let odd = (order % 2) > 0;
    let mut q_values = Vec::new();
    
    for i in 0..order / 2 {
        let q = 2.0 * (PI / order as f64 * (i as f64 + 0.5)).sin();
        q_values.push(1.0 / q);
    }
    
    if odd {
        q_values.push(-1.0);
    }
    
    q_values
}

/// Create Butterworth lowpass filter
/// 
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
/// 
/// # Returns
/// * PEQ containing the Butterworth lowpass filter sections
pub fn peq_butterworth_lowpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_butterworth_q(order);
    q_values
        .into_iter()
        .map(|q| (1.0, Biquad::new(BiquadFilterType::Lowpass, freq, srate, q, 0.0)))
        .collect()
}

/// Create Butterworth highpass filter
/// 
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
/// 
/// # Returns
/// * PEQ containing the Butterworth highpass filter sections
pub fn peq_butterworth_highpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_butterworth_q(order);
    q_values
        .into_iter()
        .map(|q| (1.0, Biquad::new(BiquadFilterType::Highpass, freq, srate, q, 0.0)))
        .collect()
}

/// Compute Q values for Linkwitz-Riley filters
/// 
/// # Arguments
/// * `order` - Filter order
/// 
/// # Returns
/// * Vector of Q values for each biquad section
pub fn peq_linkwitzriley_q(order: usize) -> Vec<f64> {
    let q_bw = peq_butterworth_q(order / 2);
    let mut q_values = Vec::new();
    
    if order % 4 > 0 {
        // Odd number of pairs
        q_values.extend_from_slice(&q_bw[..q_bw.len() - 1]);
        q_values.extend_from_slice(&q_bw[..q_bw.len() - 1]);
        q_values.push(0.5);
    } else {
        // Even number of pairs
        q_values.extend_from_slice(&q_bw);
        q_values.extend_from_slice(&q_bw);
    }
    
    q_values
}

/// Create Linkwitz-Riley lowpass filter
/// 
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
/// 
/// # Returns
/// * PEQ containing the Linkwitz-Riley lowpass filter sections
pub fn peq_linkwitzriley_lowpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_linkwitzriley_q(order);
    q_values
        .into_iter()
        .map(|q| (1.0, Biquad::new(BiquadFilterType::Lowpass, freq, srate, q, 0.0)))
        .collect()
}

/// Create Linkwitz-Riley highpass filter
/// 
/// # Arguments
/// * `order` - Filter order
/// * `freq` - Cutoff frequency in Hz
/// * `srate` - Sample rate in Hz
/// 
/// # Returns
/// * PEQ containing the Linkwitz-Riley highpass filter sections
pub fn peq_linkwitzriley_highpass(order: usize, freq: f64, srate: f64) -> Peq {
    let q_values = peq_linkwitzriley_q(order);
    q_values
        .into_iter()
        .map(|q| (1.0, Biquad::new(BiquadFilterType::Highpass, freq, srate, q, 0.0)))
        .collect()
}

/// Print a formatted table of the parametric EQ filters.
///
/// The filters are printed with any non-Peak (i.e., Highpass when `iir_hp_pk` is true)
/// shown first, followed by Peak filters sorted by frequency.
pub fn peq_print(x: &[f64], iir_hp_pk: bool) {
    let rows = build_sorted_filters(x, iir_hp_pk);
    println!("+-# -|-Freq (Hz)--|-Q ---------|-Gain (dB)--|-Type-----+");
    for (i, r) in rows.iter().enumerate() {
        println!(
            "| {:<2} | {:<10.2} | {:<10.3} | {:<+10.3} | {:<8} |",
            i + 1,
            r.freq,
            r.q,
            r.gain,
            r.kind
        );
    }
    println!("+----|------------|------------|------------|----------+");
}

#[cfg(test)]
mod peq_print_tests {
    use super::{build_sorted_filters, peq_print};

    #[test]
    fn peq_print_does_not_panic() {
        let x = vec![
            1000.0_f64.log10(),
            1.0,
            2.0, // Peak
            80.0_f64.log10(),
            0.707,
            0.0, // Candidate HP when iir_hp_pk=true
            5000.0_f64.log10(),
            2.0,
            -3.0, // Peak
        ];
        // Ensure helper runs without panicking (output captured by test harness)
        peq_print(&x, true);
        peq_print(&x, false);
        // Also ensure build_sorted_filters remains consistent
        let rows = build_sorted_filters(&x, true);
        assert_eq!(rows.len(), 3);
    }
}

#[cfg(test)]
mod filter_tests {
    use super::build_sorted_filters;

    #[test]
    fn sorts_by_freq_and_sets_type() {
        let x = vec![
            1000.0_f64.log10(),
            1.0,
            0.0, // Peak at 1k
            100.0_f64.log10(),
            2.0,
            1.0, // Will become HPQ (lowest freq)
            500.0_f64.log10(),
            0.5,
            -1.0, // Peak at 500
        ];
        let rows = build_sorted_filters(&x, true);
        let freqs: Vec<f64> = rows.iter().map(|r| r.freq).collect();
        let expected = [100.0, 500.0, 1000.0];
        for (i, f) in freqs.iter().enumerate() {
            assert!(
                (f - expected[i]).abs() < 1e-9,
                "freq idx {}: got {}, expected {}",
                i,
                f,
                expected[i]
            );
        }
        assert!(rows[0].kind == "HPQ");
        assert!(rows.iter().skip(1).all(|r| r.kind == "PK"));
        // Lowest-frequency row becomes HPQ and its gain is forced to 0.0
        assert!((rows[0].q - 2.0).abs() < 1e-12 && (rows[0].gain - 0.0).abs() < 1e-12);
        assert!((rows[1].q - 0.5).abs() < 1e-12 && (rows[1].gain + 1.0).abs() < 1e-12);
        assert!((rows[2].q - 1.0).abs() < 1e-12 && (rows[2].gain - 0.0).abs() < 1e-12);
    }
}

#[cfg(test)]
mod peq_tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn test_peq_equal() {
        let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq2 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let bq3 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 4.0);
        
        let peq1 = vec![(1.0, bq1.clone()), (0.5, bq2.clone())];
        let peq2 = vec![(1.0, bq1), (0.5, bq2)];
        let peq3 = vec![(1.0, bq3)];
        
        assert!(peq_equal(&peq1, &peq2));
        assert!(!peq_equal(&peq1, &peq3));
        assert!(!peq_equal(&peq1, &vec![]));
    }
    
    #[test]
    fn test_peq_spl() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];
        let freq = array![100.0, 1000.0, 10000.0];
        
        let spl = peq_spl(&freq, &peq);
        
        // Should have gain close to 6 dB at 1kHz
        assert!(spl[1] > 5.0 && spl[1] < 7.0);
        // Should have less gain at other frequencies
        assert!(spl[0].abs() < 1.0);
        assert!(spl[2].abs() < 1.0);
    }
    
    #[test] 
    fn test_peq_preamp_gain() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
        let peq = vec![(1.0, bq)];
        
        let gain = peq_preamp_gain(&peq);
        
        // Should be negative to prevent clipping
        assert!(gain < 0.0);
        // Should be around -6 dB to compensate for the +6 dB boost
        assert!(gain > -7.0 && gain < -5.0);
    }
    
    #[test]
    fn test_peq_format_apo() {
        let bq = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
        let peq = vec![(1.0, bq)];
        
        let apo_str = peq_format_apo("Test EQ", &peq);
        
        assert!(apo_str.contains("Test EQ"));
        assert!(apo_str.contains("Preamp:"));
        assert!(apo_str.contains("Filter  1:"));
        assert!(apo_str.contains("PK"));
        assert!(apo_str.contains("1000 Hz"));
        assert!(apo_str.contains("+3.00 dB"));
    }
    
    #[test]
    fn test_butterworth_q() {
        let q_values = peq_butterworth_q(4);
        assert_eq!(q_values.len(), 2);
        
        // For 4th order Butterworth, should have specific Q values
        assert!((q_values[0] - 1.3065630).abs() < 1e-6);
        assert!((q_values[1] - 0.5411961).abs() < 1e-6);
    }
    
    #[test] 
    fn test_butterworth_filters() {
        let lp = peq_butterworth_lowpass(4, 1000.0, 48000.0);
        let hp = peq_butterworth_highpass(4, 1000.0, 48000.0);
        
        assert_eq!(lp.len(), 2);
        assert_eq!(hp.len(), 2);
        
        // All filters should have weight 1.0 and correct type
        for (weight, bq) in &lp {
            assert_eq!(*weight, 1.0);
            assert_eq!(bq.filter_type, BiquadFilterType::Lowpass);
            assert_eq!(bq.freq, 1000.0);
        }
        
        for (weight, bq) in &hp {
            assert_eq!(*weight, 1.0);
            assert_eq!(bq.filter_type, BiquadFilterType::Highpass);
            assert_eq!(bq.freq, 1000.0);
        }
    }
    
    #[test]
    fn test_linkwitzriley_filters() {
        let lp = peq_linkwitzriley_lowpass(4, 1000.0, 48000.0);
        let hp = peq_linkwitzriley_highpass(4, 1000.0, 48000.0);
        
        // 4th order LR should have 2 sections (each with Q = 0.7071...)
        assert_eq!(lp.len(), 2);
        assert_eq!(hp.len(), 2);
        
        // All should be unit weight
        for (weight, _) in &lp {
            assert_eq!(*weight, 1.0);
        }
        for (weight, _) in &hp {
            assert_eq!(*weight, 1.0);
        }
    }
}
