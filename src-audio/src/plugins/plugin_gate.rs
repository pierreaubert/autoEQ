// ============================================================================
// Gate Plugin
// ============================================================================
//
// Noise gate that silences audio below a specified threshold.
// Useful for removing background noise and mic bleed.
//
// Parameters:
// - threshold: Level below which the gate closes (dB)
// - ratio: Gate depth ratio (1.0 = no effect, inf = complete silence)
// - attack: Time to open the gate (ms)
// - hold: Time to keep gate open after signal drops (ms)
// - release: Time to close the gate (ms)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};

/// Noise gate with hold time
pub struct GatePlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_ratio: ParameterId,
    ratio: f32, // 1.0 = no effect, inf = complete silence

    param_attack: ParameterId,
    attack_ms: f32,

    param_hold: ParameterId,
    hold_ms: f32,

    param_release: ParameterId,
    release_ms: f32,

    // State per channel
    envelope: Vec<f32>,  // Current gate envelope per channel
    hold_counter: Vec<usize>, // Samples remaining in hold state
    attack_coeff: f32,
    release_coeff: f32,
}

impl GatePlugin {
    /// Create a new gate plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Threshold in dB (default: -40.0)
    /// * `ratio` - Gate depth ratio (default: 10.0, use large values for hard gate)
    /// * `attack_ms` - Attack time in milliseconds (default: 1.0)
    /// * `hold_ms` - Hold time in milliseconds (default: 10.0)
    /// * `release_ms` - Release time in milliseconds (default: 100.0)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
    ) -> Self {
        Self {
            channels,
            sample_rate: 44100, // Updated in initialize()

            param_threshold: ParameterId::from("threshold"),
            threshold_db,

            param_ratio: ParameterId::from("ratio"),
            ratio,

            param_attack: ParameterId::from("attack"),
            attack_ms,

            param_hold: ParameterId::from("hold"),
            hold_ms,

            param_release: ParameterId::from("release"),
            release_ms,

            envelope: vec![0.0; channels],
            hold_counter: vec![0; channels],
            attack_coeff: 0.0,
            release_coeff: 0.0,
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

    /// Calculate gate attenuation for a given input level
    fn calculate_gate_attenuation(&self, input_db: f32) -> f32 {
        if input_db >= self.threshold_db {
            // Above threshold - gate is open (no attenuation)
            0.0
        } else {
            // Below threshold - apply attenuation
            let below_threshold = self.threshold_db - input_db;
            below_threshold * (1.0 - 1.0 / self.ratio)
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);
    }

    /// Get hold time in samples
    fn hold_samples(&self) -> usize {
        (self.hold_ms * 0.001 * self.sample_rate as f32) as usize
    }
}

impl InPlacePlugin for GatePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Gate".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Noise gate with hold time for removing background noise".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("threshold", "Threshold", -40.0, -80.0, 0.0)
                .with_description("Level below which gate closes (dB)"),
            Parameter::new_float("ratio", "Ratio", 10.0, 1.0, 100.0)
                .with_description("Gate depth ratio (higher = more attenuation)"),
            Parameter::new_float("attack", "Attack", 1.0, 0.1, 50.0)
                .with_description("Time to open gate (ms)"),
            Parameter::new_float("hold", "Hold", 10.0, 0.0, 1000.0)
                .with_description("Time to keep gate open after signal drops (ms)"),
            Parameter::new_float("release", "Release", 100.0, 10.0, 2000.0)
                .with_description("Time to close gate (ms)"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_threshold {
            self.threshold_db = value.as_float().ok_or("Invalid threshold value")?;
        } else if id == self.param_ratio {
            self.ratio = value.as_float().ok_or("Invalid ratio value")?;
        } else if id == self.param_attack {
            self.attack_ms = value.as_float().ok_or("Invalid attack value")?;
            self.update_coefficients();
        } else if id == self.param_hold {
            self.hold_ms = value.as_float().ok_or("Invalid hold value")?;
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?;
            self.update_coefficients();
        } else {
            return Err(format!("Unknown parameter: {}", id));
        }
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_threshold {
            Some(ParameterValue::Float(self.threshold_db))
        } else if id == &self.param_ratio {
            Some(ParameterValue::Float(self.ratio))
        } else if id == &self.param_attack {
            Some(ParameterValue::Float(self.attack_ms))
        } else if id == &self.param_hold {
            Some(ParameterValue::Float(self.hold_ms))
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
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
        self.envelope.fill(0.0);
        self.hold_counter.fill(0);
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;
        let hold_samples = self.hold_samples();

        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;
                let input_sample = buffer[sample_idx];

                // Convert to dB
                let input_level = input_sample.abs().max(1e-10);
                let input_db = 20.0 * input_level.log10();

                // Calculate target attenuation
                let target_attenuation = self.calculate_gate_attenuation(input_db);

                // State machine for gate behavior
                let target_envelope = if input_db >= self.threshold_db {
                    // Signal above threshold - open gate (reset hold)
                    self.hold_counter[ch] = hold_samples;
                    0.0
                } else if self.hold_counter[ch] > 0 {
                    // In hold period - keep gate open
                    self.hold_counter[ch] -= 1;
                    0.0
                } else {
                    // Below threshold and hold expired - close gate
                    target_attenuation
                };

                // Smooth envelope follower
                let coeff = if target_envelope > self.envelope[ch] {
                    self.release_coeff // Closing gate (increasing attenuation)
                } else {
                    self.attack_coeff // Opening gate (decreasing attenuation)
                };

                self.envelope[ch] = target_envelope + coeff * (self.envelope[ch] - target_envelope);

                // Apply gate
                let gain_linear = 10.0_f32.powf(-self.envelope[ch] / 20.0);
                buffer[sample_idx] = input_sample * gain_linear;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_creation() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);
        assert_eq!(gate.channels(), 2);
        assert_eq!(gate.threshold_db, -40.0);
        assert_eq!(gate.ratio, 10.0);
    }

    #[test]
    fn test_gate_attenuation() {
        let gate = GatePlugin::new(2, -40.0, 10.0, 1.0, 10.0, 100.0);

        // Above threshold - no attenuation
        let atten = gate.calculate_gate_attenuation(-30.0);
        assert_eq!(atten, 0.0);

        // Below threshold - attenuate
        // 10 dB below threshold with 10:1 ratio
        // Attenuation = 10 * (1 - 1/10) = 9 dB
        let atten = gate.calculate_gate_attenuation(-50.0);
        assert!((atten - 9.0).abs() < 0.01);
    }
}
