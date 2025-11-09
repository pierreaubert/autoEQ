// ============================================================================
// Limiter Plugin
// ============================================================================
//
// Brickwall limiter that prevents audio from exceeding a specified threshold.
// Uses lookahead for transparent limiting with minimal distortion.
//
// Parameters:
// - threshold: Maximum output level (dB)
// - release: Time to return to unity gain (ms)
// - lookahead: Lookahead time for predictive limiting (ms)
// - soft: Enable soft limiting with saturation curve (more musical)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};
use std::collections::VecDeque;

/// Brickwall limiter with lookahead
pub struct LimiterPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_release: ParameterId,
    release_ms: f32,

    param_lookahead: ParameterId,
    lookahead_ms: f32,

    param_soft: ParameterId,
    soft: bool,

    // State
    envelope: f32,                   // Current gain reduction envelope
    release_coeff: f32,              // Release coefficient
    lookahead_buffer: VecDeque<f32>, // Circular buffer for lookahead (interleaved)
    lookahead_samples: usize,        // Lookahead buffer size in samples
}

impl LimiterPlugin {
    /// Create a new limiter plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Maximum output level in dB (default: -0.1)
    /// * `release_ms` - Release time in milliseconds (default: 50.0)
    /// * `lookahead_ms` - Lookahead time in milliseconds (default: 5.0)
    /// * `soft` - Enable soft limiting with saturation curve (default: false)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        release_ms: f32,
        lookahead_ms: f32,
        soft: bool,
    ) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()

            param_threshold: ParameterId::from("threshold"),
            threshold_db,

            param_release: ParameterId::from("release"),
            release_ms,

            param_lookahead: ParameterId::from("lookahead"),
            lookahead_ms,

            param_soft: ParameterId::from("soft"),
            soft,

            envelope: 0.0,
            release_coeff: 0.0,
            lookahead_buffer: VecDeque::new(),
            lookahead_samples: 0,
        }
    }

    /// Calculate time coefficient for envelope follower
    fn time_to_coeff(time_ms: f32, sample_rate: u32) -> f32 {
        if time_ms <= 0.0 {
            0.0
        } else {
            (-1.0 / (time_ms * 0.001 * sample_rate as f32)).exp()
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);

        // Update lookahead buffer size
        let new_lookahead_samples =
            ((self.lookahead_ms * 0.001 * self.sample_rate as f32) as usize).max(1) * self.channels;

        if new_lookahead_samples != self.lookahead_samples {
            self.lookahead_samples = new_lookahead_samples;
            self.lookahead_buffer.clear();
            // Pre-fill with zeros
            self.lookahead_buffer.resize(self.lookahead_samples, 0.0);
        }
    }

    /// Apply soft limiting using hyperbolic tangent saturation curve
    /// This provides a more musical limiting with smooth transition into saturation
    fn apply_soft_limit(&self, sample: f32, threshold_linear: f32) -> f32 {
        if self.soft {
            // Use tanh for smooth saturation curve
            // Normalize by threshold, apply tanh, scale back
            let normalized = sample / threshold_linear;
            threshold_linear * normalized.tanh()
        } else {
            // Hard limiting (clamp to threshold)
            sample.clamp(-threshold_linear, threshold_linear)
        }
    }
}

impl InPlacePlugin for LimiterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Limiter".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Brickwall limiter with lookahead for transparent peak control"
                .to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("threshold", "Threshold", -0.1, -20.0, 0.0)
                .with_description("Maximum output level (dB)"),
            Parameter::new_float("release", "Release", 50.0, 10.0, 1000.0)
                .with_description("Release time (ms)"),
            Parameter::new_float("lookahead", "Lookahead", 5.0, 0.0, 20.0)
                .with_description("Lookahead time for predictive limiting (ms)"),
            Parameter::new_bool("soft", "Soft", false)
                .with_description("Enable soft limiting with saturation curve (more musical)"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_threshold {
            self.threshold_db = value.as_float().ok_or("Invalid threshold value")?;
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?;
            self.update_coefficients();
        } else if id == self.param_lookahead {
            self.lookahead_ms = value.as_float().ok_or("Invalid lookahead value")?;
            self.update_coefficients();
        } else if id == self.param_soft {
            self.soft = value.as_bool().ok_or("Invalid soft value")?;
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_lookahead {
            Some(ParameterValue::Float(self.lookahead_ms))
        } else if id == &self.param_soft {
            Some(ParameterValue::Bool(self.soft))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        self.update_coefficients();
        Ok(())
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.lookahead_buffer.clear();
        self.lookahead_buffer.resize(self.lookahead_samples, 0.0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;
        let threshold_linear = 10.0_f32.powf(self.threshold_db / 20.0);

        for frame in 0..num_frames {
            // Process all channels for this frame
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;
                let input_sample = buffer[sample_idx];

                // Push to lookahead buffer
                self.lookahead_buffer.push_back(input_sample);
            }

            // Find peak in lookahead buffer to predict future peaks
            let mut lookahead_peak = 0.0_f32;
            for sample in self.lookahead_buffer.iter() {
                lookahead_peak = lookahead_peak.max(sample.abs());
            }

            // Calculate required gain reduction based on lookahead peak
            let target_gain = if lookahead_peak > threshold_linear {
                threshold_linear / lookahead_peak
            } else {
                1.0
            };

            // Convert to dB for envelope
            let target_gr_db = if target_gain < 1.0 {
                20.0 * (1.0 / target_gain).log10()
            } else {
                0.0
            };

            // Envelope follower (instant attack, smooth release)
            if target_gr_db > self.envelope {
                // Instant attack - use maximum to prevent overshoots
                self.envelope = target_gr_db.max(self.envelope);
            } else {
                // Smooth release
                self.envelope = target_gr_db + self.release_coeff * (self.envelope - target_gr_db);
            }

            // Calculate gain and ensure it doesn't exceed 1.0
            let gain = (10.0_f32.powf(-self.envelope / 20.0)).min(1.0);

            // Process all channels for this frame (apply gain to delayed samples)
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;

                // Get delayed sample from lookahead buffer
                if let Some(delayed_sample) = self.lookahead_buffer.pop_front() {
                    // Apply gain to delayed sample
                    let limited_sample = delayed_sample * gain;

                    // Apply soft limiting if enabled (provides additional saturation)
                    buffer[sample_idx] = self.apply_soft_limit(limited_sample, threshold_linear);
                } else {
                    buffer[sample_idx] = 0.0;
                }
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_samples / self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_creation() {
        let limiter = LimiterPlugin::new(2, -0.1, 50.0, 5.0, false);
        assert_eq!(limiter.channels(), 2);
        assert_eq!(limiter.threshold_db, -0.1);
    }

    #[test]
    fn test_limiter_prevents_clipping() {
        let mut limiter = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false); // No lookahead for simple test
        limiter.initialize(48000).unwrap();

        // Test with a signal that would clip
        let context = ProcessContext {
            num_frames: 10,
            sample_rate: 48000,
        };
        let mut buffer = vec![1.5; 10]; // Signal exceeds 1.0

        limiter.process_in_place(&mut buffer, &context).unwrap();

        // All samples should be <= 1.0
        for sample in &buffer {
            assert!(sample.abs() <= 1.0, "Sample {} exceeds 1.0", sample);
        }
    }
}
