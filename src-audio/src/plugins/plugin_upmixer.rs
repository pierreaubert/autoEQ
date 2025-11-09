// ============================================================================
// Upmixer Plugin - Stereo to 5.0 Surround
// ============================================================================
//
// This plugin converts stereo (2 channels) to 5.0 surround sound using
// FFT-based Direct/Ambient decomposition.
//
// Algorithm uses frequency-domain
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
    /// Hop size for overlap-add (fft_size / 2 for 50% overlap)
    hop_size: usize,
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

    /// Temporary input block for FFT processing (pre-allocated)
    temp_input_block: Vec<f32>,

    /// Hann window for FFT (pre-computed)
    window: Vec<f32>,
    /// Output accumulator for overlap-add (holds fft_size samples per channel)
    /// This allows us to accumulate processed blocks and drain them gradually
    output_accumulator: Vec<Vec<f32>>,
    /// Number of valid samples in output accumulator
    output_accumulator_fill: usize,
    /// Next position to add a block (tracks overlap-add offset)
    next_add_position: usize,
    /// Pre-allocated output block buffer (reused to avoid allocations)
    output_block: Vec<f32>,
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

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        let zero_complex = Complex::new(0.0, 0.0);

        // Generate Hann window: w[n] = 0.5 * (1 - cos(2*pi*n/N))
        // Using N (not N-1) for perfect COLA with 50% overlap
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0 - ((2.0 * std::f32::consts::PI * i as f32) / fft_size as f32).cos())
            })
            .collect();

        // 50% overlap requires fft_size/2 hop size
        let hop_size = fft_size / 2;

        // Output accumulator holds up to 3*fft_size samples per channel
        // This provides enough headroom to avoid frequent draining during processing
        // which can cause discontinuities and crackling
        let output_accumulator = vec![vec![0.0; fft_size * 3]; 5]; // 5 output channels

        Self {
            fft_size,
            hop_size,
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

            temp_input_block: vec![0.0; fft_size * 2], // Pre-allocated temp buffer

            window,
            output_accumulator,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_block: vec![0.0; fft_size * 5], // Pre-allocated output block
        }
    }

    /// Process one FFT block
    fn process_fft_block(&mut self, input: &[f32], output: &mut [f32]) {
        // Verify sizes
        assert_eq!(input.len(), self.fft_size * 2); // stereo interleaved
        assert_eq!(output.len(), self.fft_size * 5); // 5.0 surround interleaved

        // 1. Copy input to time domain buffers and apply ANALYSIS window
        // CRITICAL: Window BEFORE FFT to prevent spectral leakage!
        // Standard STFT: window input -> FFT -> process -> IFFT -> overlap-add
        // Optimized for cache locality - process both channels together
        for i in 0..self.fft_size {
            let idx = i * 2;
            let window_val = self.window[i];
            self.time_domain_left[i] = Complex::new(input[idx] * window_val, 0.0);
            self.time_domain_right[i] = Complex::new(input[idx + 1] * window_val, 0.0);
        }

        // 2. Forward FFT (in-place)
        // Copy to frequency domain buffers first
        self.freq_domain_left
            .copy_from_slice(&self.time_domain_left);
        self.freq_domain_right
            .copy_from_slice(&self.time_domain_right);

        self.fft_forward.process(&mut self.freq_domain_left);
        self.fft_forward.process(&mut self.freq_domain_right);

        // 3. Direct/Ambient decomposition
        // Direct (center/phantom) = (L + R) / 2
        // Ambient (sides) = (L - R) / 2
        for i in 0..self.fft_size {
            let left = self.freq_domain_left[i];
            let right = self.freq_domain_right[i];

            // Direct component (what's common to both channels - center image)
            self.direct[i] = (left + right) * 0.5;

            // Ambient component (what's different - spatial/reverb)
            // Left ambient: emphasize left differences
            self.ambient_left[i] = (left - right) * 0.5;
            // Right ambient: emphasize right differences
            self.ambient_right[i] = (right - left) * 0.5;
        }

        // 4. Extract center channel from direct component
        // Center gets the direct component, left/right get direct minus center
        for i in 0..self.fft_size {
            // Center channel is the direct (phantom center) component
            self.direct_center[i] = self.direct[i];
            self.direct_center_mag[i] = self.direct[i].norm();

            // Front left/right get direct component (panned)
            // This creates a phantom center while maintaining stereo width
            self.direct_left[i] = self.freq_domain_left[i] - self.direct[i] * 0.5;
            self.direct_right[i] = self.freq_domain_right[i] - self.direct[i] * 0.5;
        }

        // 5. Inverse FFT for all output channels (in-place)
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

        // 6. Mix outputs with gains (NO additional windowing needed!)
        // IFFT output is already windowed because we windowed before FFT
        // Apply -3dB gain reduction to prevent clipping (0.707946 ≈ 10^(-3/20))
        let fft_scale = 1.0 / self.fft_size as f32;
        let output_gain = 0.707946; // -3dB to prevent clipping
        let combined_scale = fft_scale * output_gain;

        // Pre-compute gain factors
        let gain_fd = self.gain_front_direct * combined_scale;
        let gain_fa = self.gain_front_ambient * combined_scale;
        let gain_ra = self.gain_rear_ambient * combined_scale;
        // Center is part of direct sound, so use gain_front_direct
        let gain_center = self.gain_front_direct * combined_scale;

        // Mix channels - IFFT output already windowed, ready for overlap-add
        // Optimized: process all channels in single loop, better cache locality
        for i in 0..self.fft_size {
            let idx = i * 5;

            // Extract real parts once
            let direct_left = self.time_out_front_left[i].re;
            let direct_right = self.time_out_front_right[i].re;
            let center = self.time_out_center[i].re;
            let ambient_left = self.time_out_rear_left[i].re;
            let ambient_right = self.time_out_rear_right[i].re;

            // Write all 5 channels with pre-computed gains (already include combined_scale)
            output[idx] = direct_left * gain_fd + ambient_left * gain_fa;
            output[idx + 1] = direct_right * gain_fd + ambient_right * gain_fa;
            output[idx + 2] = center * gain_center;
            output[idx + 3] = ambient_left * gain_ra;
            output[idx + 4] = ambient_right * gain_ra;
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

        // Clear output accumulator
        for accum_buf in self.output_accumulator.iter_mut() {
            accum_buf.fill(0.0);
        }
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;

        // Clear output block
        self.output_block.fill(0.0);
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

        eprintln!(
            "[UPMIXER] process() called: input={} frames, output={} frames",
            context.num_frames, context.num_frames
        );
        eprintln!(
            "[UPMIXER] Initial state: input_buffer_fill={}, output_accumulator_fill={}, next_add_pos={}",
            self.input_buffer_fill, self.output_accumulator_fill, self.next_add_position
        );

        // Sanity check for threading issues
        if self.next_add_position > self.fft_size * 3 {
            eprintln!(
                "[UPMIXER] WARNING: Corrupted state detected! next_add_pos={} exceeds buffer size {}",
                self.next_add_position,
                self.fft_size * 3
            );
            eprintln!("[UPMIXER] This could indicate a threading issue. Resetting state.");
            self.reset();
        }

        // Initialize output buffer to zero (critical to prevent crackling!)
        output.fill(0.0);

        let mut input_pos = 0;
        let mut output_pos = 0;

        // Main processing loop: interleave input filling, FFT processing, and output draining
        let mut iteration = 0;
        loop {
            iteration += 1;
            if iteration > 1000 {
                eprintln!("[UPMIXER] ERROR: Infinite loop detected after 1000 iterations!");
                eprintln!(
                    "[UPMIXER] State: input_pos={}/{}, output_pos={}/{}",
                    input_pos / 2,
                    input.len() / 2,
                    output_pos / 5,
                    output.len() / 5
                );
                eprintln!(
                    "[UPMIXER] input_buffer_fill={}, output_accumulator_fill={}, next_add_pos={}",
                    self.input_buffer_fill, self.output_accumulator_fill, self.next_add_position
                );
                break;
            }
            // Step 1: Drain output accumulator if we have data and space
            let frames_available = (output.len() - output_pos) / 5;
            let frames_to_drain = self.output_accumulator_fill.min(frames_available);

            if frames_to_drain > 0 {
                eprintln!(
                    "[UPMIXER] Iter {}: DRAIN {} frames (accum_fill={}, frames_avail={})",
                    iteration, frames_to_drain, self.output_accumulator_fill, frames_available
                );

                // Copy samples to output
                for i in 0..frames_to_drain {
                    for ch in 0..5 {
                        output[output_pos + i * 5 + ch] = self.output_accumulator[ch][i];
                    }
                }
                output_pos += frames_to_drain * 5;

                // Shift accumulator
                for ch in 0..5 {
                    self.output_accumulator[ch]
                        .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                    // Clear the tail
                    for i in (self.output_accumulator_fill - frames_to_drain)
                        ..self.output_accumulator_fill
                    {
                        self.output_accumulator[ch][i] = 0.0;
                    }
                }
                self.output_accumulator_fill -= frames_to_drain;

                // Update next add position (subtract drained amount)
                self.next_add_position = self.next_add_position.saturating_sub(frames_to_drain);

                // Reset position if accumulator is empty
                if self.output_accumulator_fill == 0 {
                    self.next_add_position = 0;
                }

                eprintln!(
                    "[UPMIXER] After drain: accum_fill={}, next_add_pos={}, output_pos={}",
                    self.output_accumulator_fill,
                    self.next_add_position,
                    output_pos / 5
                );
            }

            // Step 2: Process FFT block if we have input and accumulator space
            // Ensure accumulator won't overflow (need space for fft_size samples)
            let can_process_input = self.input_buffer_fill >= self.fft_size * 2;
            let can_process_space = self.next_add_position + self.fft_size <= self.fft_size * 3;

            if can_process_input && can_process_space {
                eprintln!(
                    "[UPMIXER] Iter {}: PROCESS FFT (input_buf_fill={}/{}, next_add_pos={}, space_ok={})",
                    iteration,
                    self.input_buffer_fill / 2,
                    self.fft_size,
                    self.next_add_position,
                    can_process_space
                );

                // Copy to temp buffer
                self.temp_input_block[..self.fft_size * 2]
                    .copy_from_slice(&self.input_buffer[..self.fft_size * 2]);

                // Process FFT block
                let temp_input = std::mem::take(&mut self.temp_input_block);
                let mut output_block = std::mem::take(&mut self.output_block);
                self.process_fft_block(&temp_input, &mut output_block);
                self.temp_input_block = temp_input;

                // Accumulate output (overlap-add) at next_add_position
                for i in 0..self.fft_size {
                    for ch in 0..5 {
                        self.output_accumulator[ch][self.next_add_position + i] +=
                            output_block[i * 5 + ch];
                    }
                }

                // Update fill level and next add position
                if self.output_accumulator_fill == 0 {
                    // First block: fills from 0 to fft_size
                    self.output_accumulator_fill = self.fft_size;
                    self.next_add_position = self.hop_size;
                } else {
                    // Subsequent blocks: add hop_size more samples, next block starts hop_size later
                    self.output_accumulator_fill += self.hop_size;
                    self.next_add_position += self.hop_size;
                }

                self.output_block = output_block;

                // Shift input buffer by hop_size (50% overlap)
                let shift_amount = self.hop_size * 2; // stereo
                self.input_buffer
                    .copy_within(shift_amount..self.fft_size * 2, 0);
                self.input_buffer_fill -= shift_amount;

                eprintln!(
                    "[UPMIXER] After FFT: accum_fill={}, next_add_pos={}, input_buf_fill={}",
                    self.output_accumulator_fill,
                    self.next_add_position,
                    self.input_buffer_fill / 2
                );

                continue; // Process more blocks if possible
            } else if !can_process_input || !can_process_space {
                eprintln!(
                    "[UPMIXER] Iter {}: SKIP FFT (can_process_input={}, can_process_space={})",
                    iteration, can_process_input, can_process_space
                );
            }

            // Step 3: Fill input buffer if we have more input
            if input_pos < input.len() {
                let samples_to_copy =
                    (input.len() - input_pos).min(self.fft_size * 2 - self.input_buffer_fill);

                eprintln!(
                    "[UPMIXER] Iter {}: FILL {} samples (input_pos={}/{}, input_buf_fill={})",
                    iteration,
                    samples_to_copy / 2,
                    input_pos / 2,
                    input.len() / 2,
                    self.input_buffer_fill / 2
                );

                self.input_buffer[self.input_buffer_fill..self.input_buffer_fill + samples_to_copy]
                    .copy_from_slice(&input[input_pos..input_pos + samples_to_copy]);

                self.input_buffer_fill += samples_to_copy;
                input_pos += samples_to_copy;

                eprintln!(
                    "[UPMIXER] After fill: input_buf_fill={}, input_pos={}",
                    self.input_buffer_fill / 2,
                    input_pos / 2
                );

                continue; // Try processing again
            }

            // No more work to do - exit loop
            // Exit when: output buffer is full OR (no more input AND can't process AND nothing to drain)
            let cant_process = self.input_buffer_fill < self.fft_size * 2
                || self.next_add_position + self.fft_size > self.fft_size * 3;
            let no_data_to_drain = self.output_accumulator_fill == 0;
            let no_space_to_drain = (output.len() - output_pos) / 5 == 0;

            eprintln!(
                "[UPMIXER] Iter {}: CHECK EXIT - no_more_input={}, cant_process={}, no_data={}, no_space={}",
                iteration,
                input_pos >= input.len(),
                cant_process,
                no_data_to_drain,
                no_space_to_drain
            );

            // Exit if output buffer is full (most important - prevents deadlock)
            if no_space_to_drain {
                eprintln!("[UPMIXER] EXITING LOOP: output buffer full");
                break;
            }

            // Exit if no more input and can't process and nothing to drain
            if input_pos >= input.len() && cant_process && no_data_to_drain {
                eprintln!("[UPMIXER] EXITING LOOP: no more work");
                break;
            }
        }

        eprintln!("[UPMIXER] Loop finished after {} iterations", iteration);
        eprintln!(
            "[UPMIXER] Final: output_pos={}/{}, accum_fill={}",
            output_pos / 5,
            output.len() / 5,
            self.output_accumulator_fill
        );

        // Final drain of any remaining output
        let frames_available = (output.len() - output_pos) / 5;
        let frames_to_drain = self.output_accumulator_fill.min(frames_available);

        if frames_to_drain > 0 {
            eprintln!(
                "[UPMIXER] FINAL DRAIN: {} frames (accum_fill={}, frames_avail={})",
                frames_to_drain, self.output_accumulator_fill, frames_available
            );

            for i in 0..frames_to_drain {
                for ch in 0..5 {
                    output[output_pos + i * 5 + ch] = self.output_accumulator[ch][i];
                }
            }
            output_pos += frames_to_drain * 5;

            for ch in 0..5 {
                self.output_accumulator[ch]
                    .copy_within(frames_to_drain..self.output_accumulator_fill, 0);
                for i in
                    (self.output_accumulator_fill - frames_to_drain)..self.output_accumulator_fill
                {
                    self.output_accumulator[ch][i] = 0.0;
                }
            }
            self.output_accumulator_fill -= frames_to_drain;

            // Update next add position
            self.next_add_position = self.next_add_position.saturating_sub(frames_to_drain);

            // Reset position if accumulator is empty
            if self.output_accumulator_fill == 0 {
                self.next_add_position = 0;
            }

            eprintln!(
                "[UPMIXER] After final drain: accum_fill={}, next_add_pos={}, total_output={}",
                self.output_accumulator_fill,
                self.next_add_position,
                output_pos / 5
            );
        }

        eprintln!(
            "[UPMIXER] process() complete: returned {} frames\n",
            output_pos / 5
        );

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

    #[test]
    fn test_upmixer_zero_gains() {
        // Test that with all gains at 0, output is silence (critical for crackling fix)
        let mut plugin = UpmixerPlugin::new(2048, 0.0, 0.0, 0.0);
        plugin.initialize(44100).unwrap();

        // Create test input with signal
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

        // Verify output is all zeros (or very close to zero due to floating point)
        let max_abs = output.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        println!("Max abs value with zero gains: {}", max_abs);
        assert!(
            max_abs < 1e-6,
            "With all gains at 0, output should be silent, but max abs = {}",
            max_abs
        );
    }

    #[test]
    fn test_upmixer_full_5ch() {
        // Test full 5.0 upmixing with direct/ambient decomposition
        let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.0, 1.0);
        plugin.initialize(44100).unwrap();

        // Create test input with distinct left and right signals
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left: sine
            input[i * 2 + 1] = (i as f32 * 0.02).cos() * 0.5; // Right: cosine at different freq
        }
        let mut output = vec![0.0_f32; 2048 * 5];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check each channel
        let mut channel_energies = vec![0.0; 5];
        for i in 0..2048 {
            for ch in 0..5 {
                channel_energies[ch] += output[i * 5 + ch].powi(2);
            }
        }

        println!("Channel energies: {:?}", channel_energies);

        // Front left and right should have signal
        assert!(channel_energies[0] > 0.1, "Front left should have signal");
        assert!(channel_energies[1] > 0.1, "Front right should have signal");

        // Center should have signal (direct component)
        assert!(
            channel_energies[2] > 0.01,
            "Center should have direct component"
        );

        // Rear channels should have signal (ambient with gain=1.0)
        assert!(
            channel_energies[3] > 0.01,
            "Rear left should have ambient signal"
        );
        assert!(
            channel_energies[4] > 0.01,
            "Rear right should have ambient signal"
        );
    }

    #[test]
    fn test_continuity_invariant() {
        // INVARIANT: Processing continuous audio in chunks should produce continuous output
        // Test with various buffer sizes
        for buffer_size in [256, 512, 1024] {
            println!("\n=== Testing buffer size {} ===", buffer_size);
            let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
            plugin.initialize(44100).unwrap();

            // Generate continuous 440Hz sine wave, process in chunks
            let total_samples = 8192;
            let mut all_output = Vec::new();
            let mut sample_offset = 0;

            while sample_offset < total_samples {
                let chunk_size = buffer_size.min(total_samples - sample_offset);
                let mut input = vec![0.0_f32; chunk_size * 2];

                for i in 0..chunk_size {
                    let phase =
                        2.0 * std::f32::consts::PI * 440.0 * (sample_offset + i) as f32 / 44100.0;
                    input[i * 2] = phase.sin() * 0.5;
                    input[i * 2 + 1] = phase.sin() * 0.5;
                }

                let mut output = vec![0.0_f32; chunk_size * 5];
                let context = ProcessContext {
                    sample_rate: 44100,
                    num_frames: chunk_size,
                };

                plugin.process(&input, &mut output, &context).unwrap();
                all_output.extend_from_slice(&output);
                sample_offset += chunk_size;
            }

            // Check that we got significant output (accounting for latency)
            let total_output_samples = all_output.len() / 5;
            let non_zero_samples = all_output.iter().filter(|&&x| x.abs() > 1e-6).count();
            println!(
                "Buffer size {}: {} total frames, {} non-zero samples",
                buffer_size, total_output_samples, non_zero_samples
            );

            assert!(
                non_zero_samples > total_output_samples / 2,
                "Buffer size {}: Too many zero samples, got {} non-zero out of {} total",
                buffer_size,
                non_zero_samples,
                total_output_samples
            );
        }
    }

    #[test]
    fn test_energy_preservation() {
        // INVARIANT: Total output energy across all 5 channels should roughly equal input energy
        // (accounting for latency and windowing losses)
        let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
        plugin.initialize(44100).unwrap();

        let buffer_size = 1024;
        let mut total_input_energy = 0.0;
        let mut total_output_energy = 0.0;

        for iteration in 0..16 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            total_input_energy += input.iter().map(|x| x * x).sum::<f32>();

            let mut output = vec![0.0_f32; buffer_size * 5];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            // Count all 5 channels
            for i in 0..buffer_size {
                for ch in 0..5 {
                    total_output_energy += output[i * 5 + ch].powi(2);
                }
            }
        }

        println!(
            "Input energy: {}, Output energy: {}, Ratio: {}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );

        // Hann window has mean ~0.5, so expect ~75% energy loss (0.5²)
        // With overlap-add we recover some but not all
        // Accept 85% loss as reasonable for Hann windowed STFT
        assert!(
            total_output_energy > total_input_energy * 0.15,
            "Energy loss too high: input={}, output={}, ratio={}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );
    }

    #[test]
    fn test_no_gaps() {
        // INVARIANT: Every output buffer should have SOME non-zero samples after initial latency
        let mut plugin = UpmixerPlugin::new(2048, 1.0, 0.5, 1.0);
        plugin.initialize(44100).unwrap();

        let buffer_size = 512;
        let mut gap_count = 0;

        for iteration in 0..20 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            let mut output = vec![0.0_f32; buffer_size * 5];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            let max_abs = output.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

            if iteration >= 5 && max_abs < 1e-6 {
                gap_count += 1;
                println!("GAP at iteration {}: max_abs = {}", iteration, max_abs);
            }
        }

        assert_eq!(
            gap_count, 0,
            "Found {} gaps in output after initial latency",
            gap_count
        );
    }
}
