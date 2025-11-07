// ============================================================================
// Upmixer Plugin - Stereo to 5.0 Surround
// ============================================================================
//
// This plugin converts stereo (2 channels) to 5.0 surround sound using
// FFT-based Direct/Ambient decomposition.
//
// Algorithm based on the JUCE upmixer plugin, which uses frequency-domain
// analysis to separate direct sound (which goes to front channels) from
// ambient sound (which goes to surround channels).
//
// Output channel mapping:
// 0: Front Left (FL)
// 1: Front Right (FR)
// 2: Center (C)
// 3: Rear Left (RL)
// 4: Rear Right (RR)

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// Stereo to 5.0 surround upmixer using FFT-based Direct/Ambient decomposition
pub struct UpmixerPlugin {
    /// FFT size (must be power of 2)
    fft_size: usize,
    /// Sample rate
    sample_rate: u32,

    /// Forward FFT planner
    fft_forward: Arc<dyn Fft<f32>>,
    /// Inverse FFT planner
    fft_inverse: Arc<dyn Fft<f32>>,

    // Gain parameters
    /// Front direct gain (gainFS)
    param_gain_front_direct: ParameterId,
    gain_front_direct: f32,

    /// Front ambient gain (gainFA)
    param_gain_front_ambient: ParameterId,
    gain_front_ambient: f32,

    /// Rear ambient gain (gainRA)
    param_gain_rear_ambient: ParameterId,
    gain_rear_ambient: f32,

    // Processing buffers (allocated once, reused)
    /// Time domain buffer for left channel
    time_domain_left: Vec<Complex<f32>>,
    /// Time domain buffer for right channel
    time_domain_right: Vec<Complex<f32>>,

    /// Frequency domain buffer for left channel
    freq_domain_left: Vec<Complex<f32>>,
    /// Frequency domain buffer for right channel
    freq_domain_right: Vec<Complex<f32>>,

    // Intermediate buffers for upmixing algorithm
    direct: Vec<Complex<f32>>,
    direct_left: Vec<Complex<f32>>,
    direct_right: Vec<Complex<f32>>,
    ambient_left: Vec<Complex<f32>>,
    ambient_right: Vec<Complex<f32>>,
    direct_center: Vec<Complex<f32>>,
    direct_center_mag: Vec<f32>,

    // Output time-domain buffers
    time_out_front_left: Vec<Complex<f32>>,
    time_out_front_right: Vec<Complex<f32>>,
    time_out_center: Vec<Complex<f32>>,
    time_out_rear_left: Vec<Complex<f32>>,
    time_out_rear_right: Vec<Complex<f32>>,

    /// Input buffer accumulator for block-based processing
    input_buffer: Vec<f32>,
    /// Number of samples currently in input buffer
    input_buffer_fill: usize,
}

impl UpmixerPlugin {
    /// Create a new upmixer plugin
    ///
    /// # Arguments
    /// * `fft_size` - FFT size (must be power of 2, recommended: 2048)
    /// * `gain_front_direct` - Gain for direct sound in front channels (default: 1.0)
    /// * `gain_front_ambient` - Gain for ambient sound in front channels (default: 0.5)
    /// * `gain_rear_ambient` - Gain for ambient sound in rear channels (default: 1.0)
    pub fn new(
        fft_size: usize,
        gain_front_direct: f32,
        gain_front_ambient: f32,
        gain_rear_ambient: f32,
    ) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be power of 2");

        let mut planner = FftPlanner::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        let zero_complex = Complex::new(0.0, 0.0);

        Self {
            fft_size,
            sample_rate: 44100, // Will be updated in initialize()

            fft_forward,
            fft_inverse,

            param_gain_front_direct: ParameterId::from("gain_front_direct"),
            gain_front_direct,

            param_gain_front_ambient: ParameterId::from("gain_front_ambient"),
            gain_front_ambient,

            param_gain_rear_ambient: ParameterId::from("gain_rear_ambient"),
            gain_rear_ambient,

            // Allocate all buffers
            time_domain_left: vec![zero_complex; fft_size],
            time_domain_right: vec![zero_complex; fft_size],
            freq_domain_left: vec![zero_complex; fft_size],
            freq_domain_right: vec![zero_complex; fft_size],
            direct: vec![zero_complex; fft_size],
            direct_left: vec![zero_complex; fft_size],
            direct_right: vec![zero_complex; fft_size],
            ambient_left: vec![zero_complex; fft_size],
            ambient_right: vec![zero_complex; fft_size],
            direct_center: vec![zero_complex; fft_size],
            direct_center_mag: vec![0.0; fft_size],
            time_out_front_left: vec![zero_complex; fft_size],
            time_out_front_right: vec![zero_complex; fft_size],
            time_out_center: vec![zero_complex; fft_size],
            time_out_rear_left: vec![zero_complex; fft_size],
            time_out_rear_right: vec![zero_complex; fft_size],

            input_buffer: vec![0.0; fft_size * 2], // stereo
            input_buffer_fill: 0,
        }
    }

    /// Process one FFT block
    fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes
        assert_eq!(input.len(), self.fft_size * 2); // stereo interleaved
        assert_eq!(output.len(), self.fft_size * 5); // 5.0 surround interleaved

        // 1. Copy input to time domain buffers and zero imaginary part
        for i in 0..self.fft_size {
            self.time_domain_left[i] = Complex::new(input[i * 2], 0.0);
            self.time_domain_right[i] = Complex::new(input[i * 2 + 1], 0.0);
        }

        // 2. Forward FFT (in-place)
        // Copy to frequency domain buffers first
        self.freq_domain_left
            .copy_from_slice(&self.time_domain_left);
        self.freq_domain_right
            .copy_from_slice(&self.time_domain_right);

        self.fft_forward.process(&mut self.freq_domain_left);
        self.fft_forward.process(&mut self.freq_domain_right);

        // 3. Direct/Ambient decomposition in frequency domain
        // exp(i*0.6*π) from the original C++ code
        // The C++ code uses: std::complex<float> tmpExp(-0.30901699437, -0.30901699437)
        let tmp_exp = Complex::new(-0.30901699437_f32, -0.30901699437_f32);

        const SQRT_2_INV: f32 = 0.70710678118_f32; // 1/sqrt(2)

        for i in 0..self.fft_size {
            let fl = self.freq_domain_left[i];
            let fr = self.freq_domain_right[i];

            // Calculate panning coefficients
            let fl_mag = fl.norm();
            let fr_mag = fr.norm();
            let denom = (fl_mag * fl_mag + fr_mag * fr_mag).sqrt();

            let (a_l, a_r) = if denom > 1e-10 {
                (fl_mag / denom, fr_mag / denom)
            } else {
                (0.5, 0.5) // Equal panning if denominator is zero
            };

            // Calculate Direct component
            // Direct[i] = (fl * tmpExp - fr) / (aL * tmpExp - aR)
            let numerator = fl * tmp_exp - fr;
            let denominator = a_l * tmp_exp - a_r;

            self.direct[i] = if denominator.norm() > 1e-10 {
                numerator / denominator
            } else {
                Complex::new(0.0, 0.0)
            };

            // Calculate Direct L&R and Ambient L&R
            self.direct_left[i] = self.direct[i] * a_l;
            self.direct_right[i] = self.direct[i] * a_r;
            self.ambient_left[i] = fl - self.direct_left[i];
            self.ambient_right[i] = fr - self.direct_right[i];

            // Upmix Direct Component to extract Center
            let dl = self.direct_left[i];
            let dr = self.direct_right[i];

            // Center channel magnitude
            // DC_mag = sqrt(0.5) * (|DL+DR| - |DL-DR|)
            let sum_mag = (dl + dr).norm();
            let diff_mag = (dl - dr).norm();
            self.direct_center_mag[i] = SQRT_2_INV * (sum_mag - diff_mag);

            // Center channel calculation
            let dc_mag = self.direct_center_mag[i];
            let sum_norm = (dl + dr).norm();
            self.direct_center[i] = if sum_norm > f32::MIN {
                (dl + dr) * dc_mag / sum_norm
            } else {
                Complex::new(0.0, 0.0)
            };

            // Adjust front left/right by removing center component
            self.direct_left[i] = dl - self.direct_center[i] * SQRT_2_INV;
            self.direct_right[i] = dr - self.direct_center[i] * SQRT_2_INV;
        }

        // 4. Inverse FFT for all output channels (in-place)
        // Copy to output buffers first, then process in-place
        self.time_out_front_left.copy_from_slice(&self.direct_left);
        self.fft_inverse.process(&mut self.time_out_front_left);

        self.time_out_front_right
            .copy_from_slice(&self.direct_right);
        self.fft_inverse.process(&mut self.time_out_front_right);

        self.time_out_center.copy_from_slice(&self.direct_center);
        self.fft_inverse.process(&mut self.time_out_center);

        self.time_out_rear_left.copy_from_slice(&self.ambient_left);
        self.fft_inverse.process(&mut self.time_out_rear_left);

        self.time_out_rear_right
            .copy_from_slice(&self.ambient_right);
        self.fft_inverse.process(&mut self.time_out_rear_right);

        // 5. Mix outputs with gains and normalize for inverse FFT
        // Apply -3dB gain reduction to prevent clipping (0.707946 ≈ 10^(-3/20))
        let fft_scale = 1.0 / self.fft_size as f32;
        let output_gain = 0.707946; // -3dB to prevent clipping
        let combined_scale = fft_scale * output_gain;

        for i in 0..self.fft_size {
            // Front Left = Direct Left * gain_front_direct + Ambient Left * gain_front_ambient
            output[i * 5] = (self.time_out_front_left[i].re * self.gain_front_direct
                + self.time_out_rear_left[i].re * self.gain_front_ambient)
                * combined_scale;

            // Front Right
            output[i * 5 + 1] = (self.time_out_front_right[i].re * self.gain_front_direct
                + self.time_out_rear_right[i].re * self.gain_front_ambient)
                * combined_scale;

            // Center
            output[i * 5 + 2] = self.time_out_center[i].re * combined_scale;

            // Rear Left = Ambient Left * gain_rear_ambient
            output[i * 5 + 3] =
                self.time_out_rear_left[i].re * self.gain_rear_ambient * combined_scale;

            // Rear Right
            output[i * 5 + 4] =
                self.time_out_rear_right[i].re * self.gain_rear_ambient * combined_scale;
        }
    }
}

impl Plugin for UpmixerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Stereo to 5.0 Upmixer".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description:
                "Converts stereo to 5.0 surround using FFT-based Direct/Ambient decomposition"
                    .to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        2 // Stereo
    }

    fn output_channels(&self) -> usize {
        5 // 5.0 surround (FL, FR, C, RL, RR)
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("gain_front_direct", "Front Direct Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for direct sound in front channels"),
            Parameter::new_float("gain_front_ambient", "Front Ambient Gain", 0.5, 0.0, 2.0)
                .with_description("Gain for ambient sound in front channels"),
            Parameter::new_float("gain_rear_ambient", "Rear Ambient Gain", 1.0, 0.0, 2.0)
                .with_description("Gain for ambient sound in rear channels"),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_gain_front_direct {
            if let Some(gain) = value.as_float() {
                self.gain_front_direct = gain;
                return Ok(());
            }
        } else if id == self.param_gain_front_ambient {
            if let Some(gain) = value.as_float() {
                self.gain_front_ambient = gain;
                return Ok(());
            }
        } else if id == self.param_gain_rear_ambient {
            if let Some(gain) = value.as_float() {
                self.gain_rear_ambient = gain;
                return Ok(());
            }
        }
        Err(format!("Unknown parameter: {}", id))
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_gain_front_direct {
            Some(ParameterValue::Float(self.gain_front_direct))
        } else if id == &self.param_gain_front_ambient {
            Some(ParameterValue::Float(self.gain_front_ambient))
        } else if id == &self.param_gain_rear_ambient {
            Some(ParameterValue::Float(self.gain_rear_ambient))
        } else {
            None
        }
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        Ok(())
    }

    fn reset(&mut self) {
        // Clear buffers
        self.input_buffer_fill = 0;
        let zero = Complex::new(0.0, 0.0);
        for buf in [
            &mut self.time_domain_left,
            &mut self.time_domain_right,
            &mut self.freq_domain_left,
            &mut self.freq_domain_right,
            &mut self.direct,
            &mut self.direct_left,
            &mut self.direct_right,
            &mut self.ambient_left,
            &mut self.ambient_right,
            &mut self.direct_center,
        ]
        .iter_mut()
        {
            buf.fill(zero);
        }
        self.direct_center_mag.fill(0.0);
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        // Verify input size
        let input_samples = context.num_frames * 2; // stereo
        if input.len() != input_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                input_samples,
                input.len()
            ));
        }

        let output_samples = context.num_frames * 5; // 5.0 surround
        if output.len() != output_samples {
            return Err(format!(
                "Output size mismatch: expected {}, got {}",
                output_samples,
                output.len()
            ));
        }

        // For now, process in FFT-sized blocks
        // In a production implementation, you'd want overlap-add for better quality
        let mut input_pos = 0;
        let mut output_pos = 0;

        while input_pos < input.len() {
            let samples_to_copy =
                (input.len() - input_pos).min(self.fft_size * 2 - self.input_buffer_fill);

            // Copy to input buffer
            self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);

            self.input_buffer_fill += samples_to_copy;
            input_pos += samples_to_copy;

            // Process when buffer is full
            if self.input_buffer_fill == self.fft_size * 2 {
                let mut output_block = vec![0.0; self.fft_size * 5];

                // Make a copy of input_buffer to avoid borrow issues
                let input_copy = self.input_buffer.clone();
                self.process_fft_block(&input_copy, &mut output_block);

                // Copy to output
                let copy_len = (output.len() - output_pos).min(output_block.len());
                output[output_pos..output_pos + copy_len]
                    .copy_from_slice(&output_block[..copy_len]);
                output_pos += copy_len;

                self.input_buffer_fill = 0;
            }
        }

        Ok(())
    }

    fn latency_samples(&self) -> usize {
        self.fft_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upmixer_creation() {
        let plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 5);
        assert_eq!(plugin.fft_size, 2048);
    }

    #[test]
    fn test_upmixer_parameters() {
        let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);

        // Test setting parameters
        plugin
            .set_parameter(
                ParameterId::from("gain_front_direct"),
                ParameterValue::Float(0.8),
            )
            .unwrap();
        assert_eq!(plugin.gain_front_direct, 0.8);

        // Test getting parameters
        let value = plugin.get_parameter(&ParameterId::from("gain_rear_ambient"));
        assert_eq!(value, Some(ParameterValue::Float(1.0)));
    }

    #[test]
    fn test_upmixer_processing() {
        let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
        plugin.initialize(44100).unwrap();

        // Create test input: 2048 stereo samples (4096 samples total)
        // Use a simple sine wave pattern for more interesting input
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5; // Right
        }
        let mut output = vec![0.0_f32; 2048 * 5];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Verify output is not all zeros (some processing occurred)
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        println!("Output sum (abs): {}", sum);
        assert!(sum > 0.0, "Output should not be all zeros");

        // Check that we have output in multiple channels
        let mut channel_sums = vec![0.0; 5];
        for i in 0..2048 {
            for ch in 0..5 {
                channel_sums[ch] += output[i * 5 + ch].abs();
            }
        }
        println!("Channel sums: {:?}", channel_sums);
        // At least center and front channels should have content
        assert!(
            channel_sums[0] > 0.0 || channel_sums[1] > 0.0 || channel_sums[2] > 0.0,
            "At least one front channel should have content"
        );
    }
}
