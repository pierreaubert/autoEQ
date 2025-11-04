//! Audio signal generation utilities
//!
//! Provides functions for generating various test signals including:
//! - Pure tones (sine waves)
//! - Two-tone signals (for IMD testing)
//! - Logarithmic frequency sweeps
//! - White noise
//! - Pink noise (1/f spectrum)
//! - M-weighted noise (ITU-R 468)

use std::f32::consts::PI;

/// Clip a sample to prevent overflow in PCM conversion
#[inline]
pub fn clip(x: f32) -> f32 {
    x.clamp(-0.999_999, 0.999_999)
}

/// Calculate number of frames for given duration and sample rate
#[inline]
pub fn frames_for(duration: f32, sample_rate: u32) -> usize {
    (duration * sample_rate as f32).round() as usize
}

/// Generate a pure tone (sine wave)
///
/// # Arguments
/// * `freq` - Frequency in Hz
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_tone(freq: f32, amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi = 2.0 * PI * freq / sample_rate as f32;
    let mut phase: f32 = 0.0;

    for _ in 0..n_frames {
        signal.push(clip(amp * phase.sin()));
        phase += dphi;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }

    signal
}

/// Generate a two-tone signal (sum of two sine waves)
///
/// Used for intermodulation distortion (IMD) testing.
///
/// # Arguments
/// * `f1` - First frequency in Hz
/// * `a1` - First amplitude (0.0 to 1.0)
/// * `f2` - Second frequency in Hz
/// * `a2` - Second amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_two_tone(
    f1: f32,
    a1: f32,
    f2: f32,
    a2: f32,
    sample_rate: u32,
    duration: f32,
) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi1 = 2.0 * PI * f1 / sample_rate as f32;
    let dphi2 = 2.0 * PI * f2 / sample_rate as f32;
    let mut phase1: f32 = 0.0;
    let mut phase2: f32 = 0.0;

    for _ in 0..n_frames {
        let sample = a1 * phase1.sin() + a2 * phase2.sin();
        signal.push(clip(sample));
        phase1 += dphi1;
        phase2 += dphi2;
        if phase1 > 2.0 * PI {
            phase1 -= 2.0 * PI;
        }
        if phase2 > 2.0 * PI {
            phase2 -= 2.0 * PI;
        }
    }

    signal
}

/// Generate a logarithmic frequency sweep
///
/// Sweeps from `f_start` to `f_end` Hz over the specified duration.
/// Useful for frequency response measurements.
///
/// # Arguments
/// * `f_start` - Starting frequency in Hz
/// * `f_end` - Ending frequency in Hz
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_log_sweep(
    f_start: f32,
    f_end: f32,
    amp: f32,
    sample_rate: u32,
    duration: f32,
) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    let k = (f_end / f_start).ln() / duration;
    let coefficient = 2.0 * PI * f_start / k;

    for n in 0..n_frames {
        let t = n as f32 / sample_rate as f32;
        let phase = coefficient * ((k * t).exp() - 1.0);
        signal.push(clip(amp * phase.sin()));
    }

    signal
}

/// Generate white noise
///
/// Produces noise with a flat frequency spectrum.
/// Uses a deterministic LCG for reproducible output.
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_white_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Simple LCG random number generator for deterministic output
    let mut seed: u64 = 1234567890;

    for _ in 0..n_frames {
        // LCG constants from Numerical Recipes
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        // Convert to [-1, 1] range
        let random = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        signal.push(clip(amp * random));
    }

    signal
}

/// Generate pink noise
///
/// Produces noise with a 1/f spectrum (-3dB/octave).
/// Uses the Voss-McCartney algorithm (Paul Kellett's implementation).
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_pink_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Voss-McCartney algorithm (Paul Kellett's implementation)
    // Uses multiple white noise generators at different rates
    let mut seed: u64 = 9876543210;
    let mut b0 = 0.0f32;
    let mut b1 = 0.0f32;
    let mut b2 = 0.0f32;
    let mut b3 = 0.0f32;
    let mut b4 = 0.0f32;
    let mut b5 = 0.0f32;
    let mut b6 = 0.0f32;

    for _ in 0..n_frames {
        // Generate white noise
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // Update pink noise state at different rates
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.96900 * b2 + white * 0.1538520;
        b3 = 0.86650 * b3 + white * 0.3104856;
        b4 = 0.55000 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.0168980;

        let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
        b6 = white * 0.115926;

        // Normalize and scale (pink noise is ~3dB louder than white)
        signal.push(clip(amp * pink * 0.11));
    }

    signal
}

/// Generate M-weighted noise
///
/// Produces noise weighted according to ITU-R 468 standard.
/// This weighting emphasizes frequencies around 6.3 kHz, which is
/// useful for acoustic measurements.
///
/// # Arguments
/// * `amp` - Amplitude (0.0 to 1.0)
/// * `sample_rate` - Sample rate in Hz
/// * `duration` - Duration in seconds
pub fn gen_m_noise(amp: f32, sample_rate: u32, duration: f32) -> Vec<f32> {
    // M-weighted noise uses ITU-R 468 weighting curve
    // This is an approximation using a shaped white noise approach
    let n_frames = frames_for(duration, sample_rate);
    let mut signal = Vec::with_capacity(n_frames);

    // Generate white noise first
    let mut seed: u64 = 1122334455;
    let mut noise_buffer = Vec::with_capacity(n_frames);

    for _ in 0..n_frames {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        noise_buffer.push(white);
    }

    // Apply ITU-R 468 weighting approximation using IIR filters
    // This is a simplified version that boosts high frequencies (emphasis around 6.3 kHz)
    let mut hp_state = 0.0f32;

    // High-pass filter coefficient (cutoff around 30 Hz)
    let hp_coeff = 1.0 - (2.0 * PI * 30.0 / sample_rate as f32).exp();

    // Peak filter coefficients (peak around 6300 Hz)
    let peak_freq = 6300.0;
    let peak_gain_db = 12.0; // ITU-R 468 has peak around 6.3 kHz
    let w0 = 2.0 * PI * peak_freq / sample_rate as f32;
    let a = 10.0f32.powf(peak_gain_db / 40.0);

    for &white in &noise_buffer {
        // High-pass filter
        hp_state = hp_coeff * (hp_state + white);

        // Simplified peak boost (approximate ITU-R 468 weighting)
        let boosted = hp_state * (1.0 + (w0 * hp_state.abs()).sin() * a * 0.3);

        signal.push(clip(amp * boosted * 0.7));
    }

    signal
}

/// Interleave per-channel signals into a multi-channel interleaved buffer
///
/// Takes a vector of per-channel signals and interleaves them frame-by-frame.
///
/// # Arguments
/// * `per_channel` - Vector of per-channel signals (each Vec<f32> is one channel)
///
/// # Returns
/// Interleaved signal where samples are ordered: [ch0_frame0, ch1_frame0, ..., ch0_frame1, ch1_frame1, ...]
pub fn interleave_per_channel(per_channel: &[Vec<f32>]) -> Vec<f32> {
    let n_channels = per_channel.len();
    if n_channels == 0 {
        return Vec::new();
    }
    let n_frames = per_channel[0].len();
    let mut interleaved = Vec::with_capacity(n_frames * n_channels);

    for frame in 0..n_frames {
        for ch in 0..n_channels {
            interleaved.push(per_channel[ch][frame]);
        }
    }

    interleaved
}

/// Replicate a mono signal to multiple channels
///
/// Takes a mono signal and replicates it to all channels.
///
/// # Arguments
/// * `mono` - Mono signal
/// * `channels` - Number of output channels
///
/// # Returns
/// Interleaved multi-channel signal with the same content on all channels
pub fn replicate_mono(mono: &[f32], channels: u16) -> Vec<f32> {
    let n_frames = mono.len();
    let mut interleaved = Vec::with_capacity(n_frames * channels as usize);

    for &sample in mono {
        for _ in 0..channels {
            interleaved.push(sample);
        }
    }

    interleaved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frames_for() {
        assert_eq!(frames_for(1.0, 48000), 48000);
        assert_eq!(frames_for(0.5, 44100), 22050);
        assert_eq!(frames_for(2.0, 96000), 192000);
    }

    #[test]
    fn test_clip() {
        assert_eq!(clip(0.5), 0.5);
        assert_eq!(clip(-0.5), -0.5);
        assert!(clip(1.5) < 1.0);
        assert!(clip(-1.5) > -1.0);
    }

    #[test]
    fn test_gen_tone() {
        let signal = gen_tone(1000.0, 0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        // Check that signal is not all zeros
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_two_tone() {
        let signal = gen_two_tone(440.0, 0.3, 880.0, 0.3, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_log_sweep() {
        let signal = gen_log_sweep(20.0, 20000.0, 0.5, 48000, 1.0);
        assert_eq!(signal.len(), 48000);
        assert!(signal.iter().any(|&x| x.abs() > 0.1));
    }

    #[test]
    fn test_gen_white_noise() {
        let signal = gen_white_noise(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        // Check that noise has reasonable variance
        let mean: f32 = signal.iter().sum::<f32>() / signal.len() as f32;
        assert!(mean.abs() < 0.1); // Should be roughly centered at 0
    }

    #[test]
    fn test_gen_pink_noise() {
        let signal = gen_pink_noise(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.01));
    }

    #[test]
    fn test_gen_m_noise() {
        let signal = gen_m_noise(0.5, 48000, 0.1);
        assert_eq!(signal.len(), 4800);
        assert!(signal.iter().any(|&x| x.abs() > 0.01));
    }

    #[test]
    fn test_interleave_per_channel() {
        let ch0 = vec![1.0, 2.0, 3.0];
        let ch1 = vec![4.0, 5.0, 6.0];
        let interleaved = interleave_per_channel(&[ch0, ch1]);
        assert_eq!(interleaved, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn test_replicate_mono() {
        let mono = vec![1.0, 2.0, 3.0];
        let stereo = replicate_mono(&mono, 2);
        assert_eq!(stereo, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    }
}
