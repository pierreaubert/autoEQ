// ============================================================================
// Matrix Plugin - Channel mixer with configurable gain matrix
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

/// Matrix mixer plugin that routes N input channels to P output channels
///
/// Each output channel is computed as a weighted sum of all input channels,
/// where the weights are specified by an NxP gain matrix.
///
/// # Matrix Layout
/// The matrix is stored in row-major order where:
/// - matrix[out_ch * input_channels + in_ch] = gain from input in_ch to output out_ch
///
/// # Examples
///
/// Identity matrix (2x2): stereo pass-through
/// ```text
/// [1.0, 0.0,   // Out0 = 1.0*In0 + 0.0*In1
///  0.0, 1.0]   // Out1 = 0.0*In0 + 1.0*In1
/// ```
///
/// Swap channels (2x2):
/// ```text
/// [0.0, 1.0,   // Out0 = 0.0*In0 + 1.0*In1
///  1.0, 0.0]   // Out1 = 1.0*In0 + 0.0*In1
/// ```
///
/// Duplicate to 4 channels (2x4):
/// ```text
/// [1.0, 0.0,   // Out0 = In0
///  0.0, 1.0,   // Out1 = In1
///  1.0, 0.0,   // Out2 = In0
///  0.0, 1.0]   // Out3 = In1
/// ```
///
/// Mix stereo to mono (2x1):
/// ```text
/// [0.5, 0.5]   // Out0 = 0.5*In0 + 0.5*In1
/// ```
pub struct MatrixPlugin {
    /// Number of input channels
    input_channels: usize,
    /// Number of output channels
    output_channels: usize,
    /// Gain matrix in row-major order (output_channels x input_channels)
    /// matrix[out_ch * input_channels + in_ch] = gain from in_ch to out_ch
    matrix: Vec<f32>,
}

impl MatrixPlugin {
    /// Create a new matrix plugin with identity matrix
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `output_channels` - Number of output channels
    ///
    /// The matrix is initialized as identity (1.0 on diagonal, 0.0 elsewhere).
    /// For non-square matrices, only matching indices get 1.0.
    pub fn new(input_channels: usize, output_channels: usize) -> Self {
        let matrix = Self::create_identity_matrix(input_channels, output_channels);
        Self {
            input_channels,
            output_channels,
            matrix,
        }
    }

    /// Create a new matrix plugin with custom matrix
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `output_channels` - Number of output channels
    /// * `matrix` - Gain matrix in row-major order (must be output_channels * input_channels elements)
    ///
    /// # Returns
    /// `Ok(plugin)` if matrix size is correct, `Err(message)` otherwise
    pub fn with_matrix(
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>,
    ) -> Result<Self, String> {
        let expected_size = output_channels * input_channels;
        if matrix.len() != expected_size {
            return Err(format!(
                "Matrix size mismatch: expected {} elements ({}x{}), got {}",
                expected_size,
                output_channels,
                input_channels,
                matrix.len()
            ));
        }

        // Validate gains are in valid range [0.0, 1.0]
        for (idx, &gain) in matrix.iter().enumerate() {
            if !(0.0..=1.0).contains(&gain) {
                return Err(format!(
                    "Matrix element {} has invalid gain {:.3} (must be 0.0-1.0)",
                    idx, gain
                ));
            }
        }

        Ok(Self {
            input_channels,
            output_channels,
            matrix,
        })
    }

    /// Create an identity matrix
    fn create_identity_matrix(input_channels: usize, output_channels: usize) -> Vec<f32> {
        let mut matrix = vec![0.0; output_channels * input_channels];
        let min_channels = input_channels.min(output_channels);
        for i in 0..min_channels {
            matrix[i * input_channels + i] = 1.0;
        }
        matrix
    }

    /// Get the gain from input channel to output channel
    pub fn get_gain(&self, input_ch: usize, output_ch: usize) -> Option<f32> {
        if input_ch >= self.input_channels || output_ch >= self.output_channels {
            return None;
        }
        Some(self.matrix[output_ch * self.input_channels + input_ch])
    }

    /// Set the gain from input channel to output channel
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(message)` if channels are out of range or gain is invalid
    pub fn set_gain(&mut self, input_ch: usize, output_ch: usize, gain: f32) -> Result<(), String> {
        if input_ch >= self.input_channels {
            return Err(format!(
                "Input channel {} out of range (max {})",
                input_ch,
                self.input_channels - 1
            ));
        }
        if output_ch >= self.output_channels {
            return Err(format!(
                "Output channel {} out of range (max {})",
                output_ch,
                self.output_channels - 1
            ));
        }
        if !(0.0..=1.0).contains(&gain) {
            return Err(format!(
                "Gain {:.3} out of range (must be 0.0-1.0)",
                gain
            ));
        }

        self.matrix[output_ch * self.input_channels + input_ch] = gain;
        Ok(())
    }

    /// Set the entire matrix
    ///
    /// # Arguments
    /// * `matrix` - New gain matrix in row-major order
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(message)` if matrix size is wrong or gains are invalid
    pub fn set_matrix(&mut self, matrix: Vec<f32>) -> Result<(), String> {
        let expected_size = self.output_channels * self.input_channels;
        if matrix.len() != expected_size {
            return Err(format!(
                "Matrix size mismatch: expected {} elements, got {}",
                expected_size,
                matrix.len()
            ));
        }

        // Validate gains
        for (idx, &gain) in matrix.iter().enumerate() {
            if !(0.0..=1.0).contains(&gain) {
                return Err(format!(
                    "Matrix element {} has invalid gain {:.3} (must be 0.0-1.0)",
                    idx, gain
                ));
            }
        }

        self.matrix = matrix;
        Ok(())
    }

    /// Get a reference to the matrix
    pub fn matrix(&self) -> &[f32] {
        &self.matrix
    }

    /// Reset to identity matrix
    pub fn reset_to_identity(&mut self) {
        self.matrix = Self::create_identity_matrix(self.input_channels, self.output_channels);
    }
}

impl Plugin for MatrixPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Matrix".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: format!(
                "Channel matrix mixer ({}x{} channels)",
                self.input_channels, self.output_channels
            ),
        }
    }

    fn input_channels(&self) -> usize {
        self.input_channels
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        // Create parameters for each matrix element
        let mut params = Vec::new();
        for out_ch in 0..self.output_channels {
            for in_ch in 0..self.input_channels {
                let param_id = format!("gain_{}_{}", in_ch, out_ch);
                let param_name = format!("In{} → Out{}", in_ch, out_ch);
                params.push(
                    Parameter::new_float(&param_id, &param_name, 0.0, 0.0, 1.0)
                        .with_description(&format!(
                            "Gain from input channel {} to output channel {}",
                            in_ch, out_ch
                        )),
                );
            }
        }
        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Parse parameter ID: "gain_IN_OUT"
        let id_str = id.0.as_str();
        if !id_str.starts_with("gain_") {
            return Err(format!("Unknown parameter: {}", id));
        }

        let parts: Vec<&str> = id_str.split('_').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid parameter format: {}", id));
        }

        let in_ch = parts[1]
            .parse::<usize>()
            .map_err(|_| format!("Invalid input channel: {}", parts[1]))?;
        let out_ch = parts[2]
            .parse::<usize>()
            .map_err(|_| format!("Invalid output channel: {}", parts[2]))?;

        let gain = value
            .as_float()
            .ok_or_else(|| "Parameter value must be a float".to_string())?;

        self.set_gain(in_ch, out_ch, gain)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Parse parameter ID: "gain_IN_OUT"
        let id_str = id.0.as_str();
        if !id_str.starts_with("gain_") {
            return None;
        }

        let parts: Vec<&str> = id_str.split('_').collect();
        if parts.len() != 3 {
            return None;
        }

        let in_ch = parts[1].parse::<usize>().ok()?;
        let out_ch = parts[2].parse::<usize>().ok()?;

        self.get_gain(in_ch, out_ch)
            .map(ParameterValue::Float)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;

        // Validate buffer sizes
        if input.len() != num_frames * self.input_channels {
            return Err(format!(
                "Input buffer size {} doesn't match expected {} (frames={}, channels={})",
                input.len(),
                num_frames * self.input_channels,
                num_frames,
                self.input_channels
            ));
        }
        if output.len() != num_frames * self.output_channels {
            return Err(format!(
                "Output buffer size {} doesn't match expected {} (frames={}, channels={})",
                output.len(),
                num_frames * self.output_channels,
                num_frames,
                self.output_channels
            ));
        }

        // Process frame by frame
        for frame in 0..num_frames {
            let in_frame_offset = frame * self.input_channels;
            let out_frame_offset = frame * self.output_channels;

            // Compute each output channel
            for out_ch in 0..self.output_channels {
                let mut sum = 0.0;

                // Sum contributions from all input channels
                for in_ch in 0..self.input_channels {
                    let gain = self.matrix[out_ch * self.input_channels + in_ch];
                    let input_sample = input[in_frame_offset + in_ch];
                    sum += gain * input_sample;
                }

                output[out_frame_offset + out_ch] = sum;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_matrix_2x2() {
        let plugin = MatrixPlugin::new(2, 2);
        assert_eq!(plugin.get_gain(0, 0), Some(1.0));
        assert_eq!(plugin.get_gain(1, 1), Some(1.0));
        assert_eq!(plugin.get_gain(0, 1), Some(0.0));
        assert_eq!(plugin.get_gain(1, 0), Some(0.0));
    }

    #[test]
    fn test_identity_matrix_nonsquare() {
        // 2 inputs, 4 outputs
        let plugin = MatrixPlugin::new(2, 4);
        assert_eq!(plugin.get_gain(0, 0), Some(1.0));
        assert_eq!(plugin.get_gain(1, 1), Some(1.0));
        assert_eq!(plugin.get_gain(0, 2), Some(0.0));
        assert_eq!(plugin.get_gain(1, 3), Some(0.0));
    }

    #[test]
    fn test_swap_channels() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_gain(0, 0, 0.0).unwrap();
        plugin.set_gain(1, 1, 0.0).unwrap();
        plugin.set_gain(1, 0, 1.0).unwrap();
        plugin.set_gain(0, 1, 1.0).unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels: [L=1,R=2], [L=3,R=4]
        let mut output = vec![0.0; 4];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 2,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Channels should be swapped
        assert_eq!(output[0], 2.0); // Frame 0, Out0 = In1
        assert_eq!(output[1], 1.0); // Frame 0, Out1 = In0
        assert_eq!(output[2], 4.0); // Frame 1, Out0 = In1
        assert_eq!(output[3], 3.0); // Frame 1, Out1 = In0
    }

    #[test]
    fn test_duplicate_to_4_channels() {
        let matrix = vec![
            1.0, 0.0, // Out0 = In0
            0.0, 1.0, // Out1 = In1
            1.0, 0.0, // Out2 = In0
            0.0, 1.0, // Out3 = In1
        ];
        let mut plugin = MatrixPlugin::with_matrix(2, 4, matrix).unwrap();

        let input = vec![1.0, 2.0]; // 1 frame, 2 channels
        let mut output = vec![0.0; 4]; // 1 frame, 4 channels
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(output[0], 1.0); // Out0 = In0
        assert_eq!(output[1], 2.0); // Out1 = In1
        assert_eq!(output[2], 1.0); // Out2 = In0
        assert_eq!(output[3], 2.0); // Out3 = In1
    }

    #[test]
    fn test_stereo_to_mono_mix() {
        let matrix = vec![0.5, 0.5]; // 2 inputs, 1 output: Out0 = 0.5*In0 + 0.5*In1
        let mut plugin = MatrixPlugin::with_matrix(2, 1, matrix).unwrap();

        let input = vec![2.0, 4.0]; // 1 frame, 2 channels
        let mut output = vec![0.0; 1]; // 1 frame, 1 channel
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(output[0], 3.0); // 0.5*2.0 + 0.5*4.0 = 3.0
    }

    #[test]
    fn test_parameter_set_get() {
        let mut plugin = MatrixPlugin::new(2, 2);

        // Set gain via parameter system
        plugin
            .set_parameter(ParameterId::from("gain_0_1"), ParameterValue::Float(0.7))
            .unwrap();

        assert_eq!(plugin.get_gain(0, 1), Some(0.7));

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("gain_0_1"));
        assert_eq!(value, Some(ParameterValue::Float(0.7)));
    }

    #[test]
    fn test_invalid_gain_range() {
        let mut plugin = MatrixPlugin::new(2, 2);
        assert!(plugin.set_gain(0, 0, 1.5).is_err()); // > 1.0
        assert!(plugin.set_gain(0, 0, -0.1).is_err()); // < 0.0
    }

    #[test]
    fn test_invalid_matrix_size() {
        let matrix = vec![1.0, 0.0, 0.0]; // Wrong size for 2x2
        let result = MatrixPlugin::with_matrix(2, 2, matrix);
        assert!(result.is_err());
    }
}
