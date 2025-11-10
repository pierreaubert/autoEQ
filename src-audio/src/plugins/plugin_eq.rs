// ============================================================================
// Parametric EQ Plugin
// ============================================================================
//
// This plugin applies a chain of IIR biquad filters for parametric equalization.
// Supports multiple channels with the same EQ curve applied to each channel.

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use autoeq_iir::Biquad;
use serde::{Deserialize, Serialize};

// ============================================================================
// Configuration
// ============================================================================

/// Biquad filter configuration for JSON deserialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiquadFilterConfig {
    pub filter_type: String, // "peak", "lowshelf", "highshelf", "lowpass", "highpass", "notch", "bandpass"
    pub freq: f64,
    pub q: f64,
    #[serde(default)]
    pub db_gain: f64,
}

/// Configuration parameters for EqPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPluginParams {
    pub filters: Vec<BiquadFilterConfig>,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

/// Parametric EQ plugin using IIR biquad filters
pub struct EqPlugin {
    /// Number of input/output channels
    num_channels: usize,

    /// IIR filters (one chain per channel)
    /// filters[channel_idx][filter_idx]
    filters: Vec<Vec<Biquad>>,

    /// Sample rate
    sample_rate: u32,
}

impl EqPlugin {
    /// Create a new EQ plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to process
    /// * `filters` - List of biquad filters to apply (will be cloned for each channel)
    pub fn new(num_channels: usize, filters: Vec<Biquad>) -> Self {
        // Clone the filter chain for each channel
        let mut channel_filters = Vec::with_capacity(num_channels);
        for _ in 0..num_channels {
            channel_filters.push(filters.clone());
        }

        Self {
            num_channels,
            filters: channel_filters,
            sample_rate: 48000, // Will be updated in initialize()
        }
    }

    /// Create a new EQ plugin from configuration parameters
    pub fn from_params(
        num_channels: usize,
        sample_rate: u32,
        params: EqPluginParams,
    ) -> Result<Self, String> {
        use autoeq_iir::BiquadFilterType;

        let filters: Result<Vec<Biquad>, String> = params
            .filters
            .iter()
            .map(|f| {
                let filter_type = match f.filter_type.as_str() {
                    "peak" => BiquadFilterType::Peak,
                    "lowshelf" => BiquadFilterType::Lowshelf,
                    "highshelf" => BiquadFilterType::Highshelf,
                    "lowpass" => BiquadFilterType::Lowpass,
                    "highpass" => BiquadFilterType::Highpass,
                    "notch" => BiquadFilterType::Notch,
                    "bandpass" => BiquadFilterType::Bandpass,
                    other => return Err(format!("Unknown filter type: {}", other)),
                };

                Ok(Biquad::new(
                    filter_type,
                    f.freq,
                    sample_rate as f64,
                    f.q,
                    f.db_gain,
                ))
            })
            .collect();

        let filters = filters?;
        Ok(Self::new(num_channels, filters))
    }

    /// Update the sample rate for all filters
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;

        // Update sample rate for all filters
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                filter.srate = sample_rate as f64;
                // Recompute coefficients with new sample rate
                // Note: This requires making compute_coeffs public or adding a method
                // For now we'll recreate the filter
                *filter = Biquad::new(
                    filter.filter_type,
                    filter.freq,
                    sample_rate as f64,
                    filter.q,
                    filter.db_gain,
                );
            }
        }
    }

    /// Replace the filter chain
    pub fn set_filters(&mut self, filters: Vec<Biquad>) {
        // Clone the new filter chain for each channel
        self.filters.clear();
        for _ in 0..self.num_channels {
            self.filters.push(filters.clone());
        }
    }

    /// Get a reference to the filter chain
    pub fn filters(&self) -> &[Biquad] {
        if !self.filters.is_empty() {
            &self.filters[0]
        } else {
            &[]
        }
    }
}

impl Plugin for EqPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Parametric EQ".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "Parametric equalizer with {} IIR biquad filters",
                self.filters.first().map(|f| f.len()).unwrap_or(0)
            ),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn output_channels(&self) -> usize {
        self.num_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        // EQ parameters are managed externally via set_filters()
        // Could add per-filter gain controls here if needed
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Err("EQ plugin has no adjustable parameters (use set_filters() instead)".to_string())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.set_sample_rate(sample_rate);
        Ok(())
    }

    fn reset(&mut self) {
        // Reset filter state for all channels
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                // Reset filter state by recreating
                *filter = Biquad::new(
                    filter.filter_type,
                    filter.freq,
                    filter.srate,
                    filter.q,
                    filter.db_gain,
                );
            }
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify input size
        let input_samples = context.num_frames * self.num_channels;
        if input.len() != input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                input_samples,
                input.len()
            ));
        }

        let output_samples = context.num_frames * self.num_channels;
        if output.len() != output_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                output_samples,
                output.len()
            ));
        }

        // Process each frame
        for frame_idx in 0..context.num_frames {
            for ch in 0..self.num_channels {
                let sample_idx = frame_idx * self.num_channels + ch;
                let mut sample = input[sample_idx] as f64;

                // Apply all filters in the chain for this channel
                for filter in &mut self.filters[ch] {
                    sample = filter.process(sample);
                }

                output[sample_idx] = sample as f32;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        // IIR filters have minimal latency (essentially zero for practical purposes)
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autoeq_iir::{Biquad, BiquadFilterType};

    #[test]
    fn test_eq_creation() {
        let filters = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            48000.0,
            1.0,
            3.0,
        )];
        let plugin = EqPlugin::new(2, filters);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 2);
    }

    #[test]
    fn test_eq_passthrough() {
        // Empty filter chain should be passthrough
        let plugin = EqPlugin::new(2, vec![]);
        let mut plugin = plugin;
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            input[i * 2] = (i as f32 * 0.01).sin();
            input[i * 2 + 1] = (i as f32 * 0.01).cos();
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should be exact passthrough
        for i in 0..input.len() {
            assert_eq!(output[i], input[i]);
        }
    }

    #[test]
    fn test_eq_processing() {
        // Create a simple high-shelf filter (+6dB above 1kHz)
        let filters = vec![Biquad::new(
            BiquadFilterType::Highshelf,
            1000.0,
            48000.0,
            0.707,
            6.0,
        )];
        let mut plugin = EqPlugin::new(2, filters);
        plugin.initialize(48000).unwrap();

        // Test with a 1kHz sine wave
        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            input[i * 2] = phase.sin() * 0.5;
            input[i * 2 + 1] = phase.sin() * 0.5;
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Output should be amplified due to high-shelf filter
        let input_energy: f32 = input.iter().map(|x| x * x).sum();
        let output_energy: f32 = output.iter().map(|x| x * x).sum();

        println!(
            "Input energy: {}, Output energy: {}, Ratio: {}",
            input_energy,
            output_energy,
            output_energy / input_energy
        );

        // High-shelf at 1kHz with +6dB should amplify (ratio > 1.0)
        assert!(
            output_energy > input_energy * 1.5,
            "Expected amplification from high-shelf filter"
        );
    }

    #[test]
    fn test_eq_multiple_filters() {
        // Create a multi-band EQ: bass boost + mid cut + treble boost
        let filters = vec![
            Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.707, 3.0), // +3dB bass
            Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -3.0),     // -3dB mid cut
            Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.707, 3.0), // +3dB treble
        ];
        let mut plugin = EqPlugin::new(2, filters);
        plugin.initialize(48000).unwrap();

        let mut input = vec![0.0_f32; 1024 * 2];
        for i in 0..1024 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5;
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5;
        }
        let mut output = vec![0.0_f32; 1024 * 2];

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1024,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Should produce non-zero output
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Output should not be all zeros");
    }
}
