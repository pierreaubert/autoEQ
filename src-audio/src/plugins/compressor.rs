// ============================================================================
// Compressor Plugin
// ============================================================================
//
// Dynamic range compressor that reduces the volume of loud signals.
//
// Parameters:
// - threshold: Level above which compression starts (dB)
// - ratio: Compression ratio (1.0 = no compression, 10.0 = 10:1)
// - attack: Time to reach full compression (ms)
// - release: Time to return to no compression (ms)
// - knee: Soft knee width for smoother compression (dB)
// - makeup_gain: Output gain to compensate for volume reduction (dB)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{InPlacePlugin, PluginInfo, PluginResult, ProcessContext};

/// Dynamic range compressor
pub struct CompressorPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_threshold: ParameterId,
    threshold_db: f32,

    param_ratio: ParameterId,
    ratio: f32,

    param_attack: ParameterId,
    attack_ms: f32,

    param_release: ParameterId,
    release_ms: f32,

    param_knee: ParameterId,
    knee_db: f32,

    param_makeup_gain: ParameterId,
    makeup_gain_db: f32,

    // State per channel
    envelope: Vec<f32>, // Current gain reduction envelope per channel
    attack_coeff: f32,
    release_coeff: f32,
}

impl CompressorPlugin {
    /// Create a new compressor plugin
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `threshold_db` - Threshold in dB (default: -20.0)
    /// * `ratio` - Compression ratio (default: 4.0)
    /// * `attack_ms` - Attack time in milliseconds (default: 5.0)
    /// * `release_ms` - Release time in milliseconds (default: 50.0)
    /// * `knee_db` - Soft knee width in dB (default: 6.0)
    /// * `makeup_gain_db` - Makeup gain in dB (default: 0.0)
    pub fn new(
        channels: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        makeup_gain_db: f32,
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

            param_release: ParameterId::from("release"),
            release_ms,

            param_knee: ParameterId::from("knee"),
            knee_db,

            param_makeup_gain: ParameterId::from("makeup_gain"),
            makeup_gain_db,

            envelope: vec![0.0; channels],
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

    /// Calculate gain reduction for a given input level
    fn calculate_gain_reduction(&self, input_db: f32) -> f32 {
        let threshold = self.threshold_db;
        let knee = self.knee_db;

        // Handle hard knee (knee = 0) separately
        if knee < 0.1 {
            // Hard knee compression
            if input_db <= threshold {
                0.0
            } else {
                let overshoot = input_db - threshold;
                overshoot * (1.0 - 1.0 / self.ratio)
            }
        } else {
            // Soft knee compression
            if input_db < threshold - knee / 2.0 {
                // Below threshold - no compression
                0.0
            } else if input_db > threshold + knee / 2.0 {
                // Above threshold + knee - full compression
                let overshoot = input_db - threshold;
                overshoot * (1.0 - 1.0 / self.ratio)
            } else {
                // In the knee - smooth transition
                let overshoot = input_db - threshold + knee / 2.0;
                let knee_factor = overshoot / knee;
                knee_factor * knee_factor * knee / 2.0 * (1.0 - 1.0 / self.ratio)
            }
        }
    }

    /// Update coefficients when parameters change
    fn update_coefficients(&mut self) {
        self.attack_coeff = Self::time_to_coeff(self.attack_ms, self.sample_rate);
        self.release_coeff = Self::time_to_coeff(self.release_ms, self.sample_rate);
    }
}

impl InPlacePlugin for CompressorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Compressor".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Dynamic range compressor with soft knee".to_string(),
        }
    }

    fn channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("threshold", "Threshold", -20.0, -60.0, 0.0)
                .with_description("Level above which compression starts (dB)"),
            Parameter::new_float("ratio", "Ratio", 4.0, 1.0, 20.0)
                .with_description("Compression ratio (1:1 to 20:1)"),
            Parameter::new_float("attack", "Attack", 5.0, 0.1, 100.0)
                .with_description("Attack time (ms)"),
            Parameter::new_float("release", "Release", 50.0, 10.0, 1000.0)
                .with_description("Release time (ms)"),
            Parameter::new_float("knee", "Knee", 6.0, 0.0, 20.0)
                .with_description("Soft knee width (dB)"),
            Parameter::new_float("makeup_gain", "Makeup Gain", 0.0, 0.0, 24.0)
                .with_description("Output gain compensation (dB)"),
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
        } else if id == self.param_release {
            self.release_ms = value.as_float().ok_or("Invalid release value")?;
            self.update_coefficients();
        } else if id == self.param_knee {
            self.knee_db = value.as_float().ok_or("Invalid knee value")?;
        } else if id == self.param_makeup_gain {
            self.makeup_gain_db = value.as_float().ok_or("Invalid makeup gain value")?;
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
        } else if id == &self.param_release {
            Some(ParameterValue::Float(self.release_ms))
        } else if id == &self.param_knee {
            Some(ParameterValue::Float(self.knee_db))
        } else if id == &self.param_makeup_gain {
            Some(ParameterValue::Float(self.makeup_gain_db))
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
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;
        let makeup_gain_linear = 10.0_f32.powf(self.makeup_gain_db / 20.0);

        for frame in 0..num_frames {
            for ch in 0..self.channels {
                let sample_idx = frame * self.channels + ch;
                let input_sample = buffer[sample_idx];

                // Convert to dB
                let input_level = input_sample.abs().max(1e-10);
                let input_db = 20.0 * input_level.log10();

                // Calculate target gain reduction
                let target_gr = self.calculate_gain_reduction(input_db);

                // Smooth envelope follower
                let coeff = if target_gr > self.envelope[ch] {
                    self.attack_coeff
                } else {
                    self.release_coeff
                };

                self.envelope[ch] = target_gr + coeff * (self.envelope[ch] - target_gr);

                // Apply gain reduction and makeup gain
                let gain_linear = 10.0_f32.powf(-self.envelope[ch] / 20.0) * makeup_gain_linear;
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
    fn test_compressor_creation() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
        assert_eq!(compressor.channels(), 2);
        assert_eq!(compressor.threshold_db, -20.0);
        assert_eq!(compressor.ratio, 4.0);
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 0.0, 0.0); // No knee for simple test

        // Below threshold - no compression
        let gr = compressor.calculate_gain_reduction(-30.0);
        assert_eq!(gr, 0.0);

        // At threshold - no compression
        let gr = compressor.calculate_gain_reduction(-20.0);
        assert_eq!(gr, 0.0);

        // 12 dB above threshold with 4:1 ratio
        // Gain reduction = 12 * (1 - 1/4) = 9 dB
        let gr = compressor.calculate_gain_reduction(-8.0);
        assert!((gr - 9.0).abs() < 0.01);
    }
}
