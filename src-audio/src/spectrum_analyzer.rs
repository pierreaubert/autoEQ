use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Real-time spectrum measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumInfo {
    /// Frequency bin centers in Hz
    pub frequencies: Vec<f32>,
    /// Magnitude values in dB (relative to full scale)
    pub magnitudes: Vec<f32>,
    /// Peak magnitude across all bins
    pub peak_magnitude: f32,
}

impl Default for SpectrumInfo {
    fn default() -> Self {
        Self {
            frequencies: Vec::new(),
            magnitudes: Vec::new(),
            peak_magnitude: f32::NEG_INFINITY,
        }
    }
}

/// Configuration for spectrum analyzer
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    /// Number of frequency bins (default: 30)
    pub num_bins: usize,
    /// Minimum frequency in Hz (default: 20)
    pub min_freq: f32,
    /// Maximum frequency in Hz (default: 20000)
    pub max_freq: f32,
    /// Smoothing factor for exponential moving average (0.0 to 1.0)
    /// Higher values = more smoothing, lower values = more responsive
    pub smoothing: f32,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.7,
        }
    }
}

/// Real-time spectrum analyzer using FFT
pub struct SpectrumAnalyzer {
    /// Configuration
    config: SpectrumConfig,
    /// Sample rate in Hz
    sample_rate: u32,
    /// Number of channels
    channels: u32,
    /// FFT size (power of 2)
    fft_size: usize,
    /// Circular buffer for audio samples
    sample_buffer: Vec<f32>,
    /// Write position in circular buffer
    buffer_pos: usize,
    /// Frequency bin edges (for logarithmic binning)
    bin_edges: Vec<f32>,
    /// Frequency bin centers
    bin_centers: Vec<f32>,
    /// Current spectrum measurements (smoothed)
    current_spectrum: Arc<Mutex<SpectrumInfo>>,
    /// Previous spectrum values (for smoothing)
    prev_magnitudes: Vec<f32>,
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer
    pub fn new(channels: u32, sample_rate: u32, config: SpectrumConfig) -> Result<Self, String> {
        // eprintln!(
        //     "[Spectrum Analyzer] Creating new analyzer: {}ch, {}Hz",
        //     channels, sample_rate
        // );

        if config.num_bins < 2 {
            return Err("num_bins must be at least 2".to_string());
        }
        if config.min_freq <= 0.0 || config.max_freq <= config.min_freq {
            return Err("Invalid frequency range".to_string());
        }
        if !(0.0..=1.0).contains(&config.smoothing) {
            return Err("smoothing must be between 0.0 and 1.0".to_string());
        }

        // FFT size: use at least 2048 for good frequency resolution
        let fft_size = 2048;

        // Generate logarithmic frequency bins
        let (bin_edges, bin_centers) =
            Self::generate_log_bins(config.num_bins, config.min_freq, config.max_freq);

        // eprintln!(
        //     "[Spectrum Analyzer] Generated {} bins from {:.0}Hz to {:.0}Hz",
        //     config.num_bins, config.min_freq, config.max_freq
        // );

        let current_spectrum = Arc::new(Mutex::new(SpectrumInfo {
            frequencies: bin_centers.clone(),
            magnitudes: vec![f32::NEG_INFINITY; config.num_bins],
            peak_magnitude: f32::NEG_INFINITY,
        }));

        let num_bins = config.num_bins;
        Ok(Self {
            config,
            sample_rate,
            channels,
            fft_size,
            sample_buffer: vec![0.0; fft_size],
            buffer_pos: 0,
            bin_edges,
            bin_centers,
            current_spectrum,
            prev_magnitudes: vec![f32::NEG_INFINITY; num_bins],
        })
    }

    /// Generate logarithmic frequency bins
    fn generate_log_bins(num_bins: usize, min_freq: f32, max_freq: f32) -> (Vec<f32>, Vec<f32>) {
        let log_min = min_freq.log10();
        let log_max = max_freq.log10();

        let mut edges = Vec::with_capacity(num_bins + 1);
        let mut centers = Vec::with_capacity(num_bins);

        for i in 0..=num_bins {
            let log_freq = log_min + (log_max - log_min) * (i as f32 / num_bins as f32);
            edges.push(10.0_f32.powf(log_freq));
        }

        for i in 0..num_bins {
            let center = (edges[i] * edges[i + 1]).sqrt(); // Geometric mean
            centers.push(center);
        }

        (edges, centers)
    }

    /// Add audio frames to the analyzer
    pub fn add_frames(&mut self, samples: &[f32]) -> Result<(), String> {
        // Mix all channels to mono for spectrum analysis
        let mono_samples = self.mix_to_mono(samples);

        // eprintln!(
        //     "[Spectrum Analyzer] add_frames: {} samples, buffer_pos={}",
        //     mono_samples.len(),
        //     self.buffer_pos
        // );

        // Add samples to circular buffer
        for sample in mono_samples {
            self.sample_buffer[self.buffer_pos] = sample;
            self.buffer_pos = (self.buffer_pos + 1) % self.fft_size;

            // When buffer is full, compute spectrum
            if self.buffer_pos == 0 {
                self.compute_spectrum()?;
            }
        }

        Ok(())
    }

    /// Mix all channels to mono
    fn mix_to_mono(&self, samples: &[f32]) -> Vec<f32> {
        let channels = self.channels as usize;
        let num_frames = samples.len() / channels;

        let mut mono = Vec::with_capacity(num_frames);
        for frame_idx in 0..num_frames {
            let mut sum = 0.0;
            for ch in 0..channels {
                sum += samples[frame_idx * channels + ch];
            }
            mono.push(sum / channels as f32);
        }

        mono
    }

    /// Compute spectrum using simple FFT approximation (Goertzel-like approach)
    /// For better accuracy, consider using rustfft crate
    fn compute_spectrum(&mut self) -> Result<(), String> {
        // Apply Hann window
        let windowed = self.apply_hann_window(&self.sample_buffer);

        // Compute magnitude spectrum at bin frequencies
        let mut magnitudes = vec![0.0; self.config.num_bins];

        for (bin_idx, &center_freq) in self.bin_centers.iter().enumerate() {
            let magnitude = self.compute_bin_magnitude(&windowed, center_freq);
            magnitudes[bin_idx] = magnitude;
        }

        // Apply smoothing (exponential moving average)
        for i in 0..self.config.num_bins {
            if self.prev_magnitudes[i].is_finite() {
                magnitudes[i] = self.config.smoothing * self.prev_magnitudes[i]
                    + (1.0 - self.config.smoothing) * magnitudes[i];
            }
        }

        self.prev_magnitudes = magnitudes.clone();

        // Find peak
        let peak_magnitude = magnitudes.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Update shared state
        {
            let mut spectrum = self.current_spectrum.lock().unwrap();
            spectrum.magnitudes = magnitudes;
            spectrum.peak_magnitude = peak_magnitude;
        }

        // eprintln!(
        //     "[Spectrum Analyzer] Computed spectrum: peak={:.1}dB",
        //     peak_magnitude
        // );

        Ok(())
    }

    /// Apply Hann window to samples
    fn apply_hann_window(&self, samples: &[f32]) -> Vec<f32> {
        let n = samples.len();
        samples
            .iter()
            .enumerate()
            .map(|(i, &sample)| {
                let window =
                    0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos());
                sample * window
            })
            .collect()
    }

    /// Compute magnitude at a specific frequency using Goertzel algorithm
    fn compute_bin_magnitude(&self, samples: &[f32], freq: f32) -> f32 {
        let normalized_freq = freq / self.sample_rate as f32;
        let w = 2.0 * std::f32::consts::PI * normalized_freq;

        let mut s0;
        let mut s1 = 0.0;
        let mut s2 = 0.0;

        let coeff = 2.0 * w.cos();

        for &sample in samples {
            s0 = sample + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }

        let real = s1 - s2 * w.cos();
        let imag = s2 * w.sin();

        let magnitude = (real * real + imag * imag).sqrt();

        // Normalize by window sum and number of samples for proper dB scaling
        // Window sum for Hann window is approximately N/2
        let window_sum = samples.len() as f32 / 2.0;
        let normalized_magnitude = magnitude / window_sum;

        // Convert to dB (20 * log10(magnitude / reference))
        // Reference is 1.0 for full scale
        if normalized_magnitude > 1e-10 {
            20.0 * normalized_magnitude.log10()
        } else {
            f32::NEG_INFINITY
        }
    }

    /// Get the current spectrum measurements
    pub fn get_spectrum(&self) -> SpectrumInfo {
        let spectrum = self.current_spectrum.lock().unwrap();
        spectrum.clone()
    }

    /// Reset the analyzer
    pub fn reset(&mut self) -> Result<(), String> {
        self.sample_buffer.fill(0.0);
        self.buffer_pos = 0;
        self.prev_magnitudes.fill(f32::NEG_INFINITY);

        {
            let mut spectrum = self.current_spectrum.lock().unwrap();
            spectrum.magnitudes.fill(f32::NEG_INFINITY);
            spectrum.peak_magnitude = f32::NEG_INFINITY;
        }

        Ok(())
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get number of channels
    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// Get configuration
    pub fn config(&self) -> &SpectrumConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_analyzer_creation() {
        let config = SpectrumConfig::default();
        let analyzer = SpectrumAnalyzer::new(2, 48000, config).unwrap();

        assert_eq!(analyzer.sample_rate(), 48000);
        assert_eq!(analyzer.channels(), 2);
        assert_eq!(analyzer.config().num_bins, 30);
    }

    #[test]
    fn test_log_bins_generation() {
        let (edges, centers) = SpectrumAnalyzer::generate_log_bins(30, 20.0, 20000.0);

        assert_eq!(edges.len(), 31);
        assert_eq!(centers.len(), 30);
        assert!(edges[0] >= 20.0);
        assert!(edges[30] <= 20000.0);

        // Check that bins are logarithmically spaced
        for i in 0..30 {
            assert!(centers[i] > edges[i]);
            assert!(centers[i] < edges[i + 1]);
        }
    }

    #[test]
    fn test_add_frames() {
        let config = SpectrumConfig::default();
        let mut analyzer = SpectrumAnalyzer::new(2, 48000, config).unwrap();

        // Generate 1 second of 1kHz sine wave
        let sample_rate = 48000;
        let frequency = 1000.0;
        let amplitude = 0.1;
        let samples: Vec<f32> = (0..sample_rate)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let value = amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin();
                vec![value, value] // Stereo
            })
            .collect();

        analyzer.add_frames(&samples).unwrap();

        let spectrum = analyzer.get_spectrum();
        assert_eq!(spectrum.frequencies.len(), 30);
        assert_eq!(spectrum.magnitudes.len(), 30);
        assert!(spectrum.peak_magnitude.is_finite());
    }

    #[test]
    fn test_reset() {
        let config = SpectrumConfig::default();
        let mut analyzer = SpectrumAnalyzer::new(2, 48000, config).unwrap();

        // Add some audio
        let samples = vec![0.1f32; 48000 * 2];
        analyzer.add_frames(&samples).unwrap();

        // Verify we have measurements
        let spectrum_before = analyzer.get_spectrum();
        assert!(spectrum_before.peak_magnitude.is_finite());

        // Reset
        analyzer.reset().unwrap();

        // Verify measurements are cleared
        let spectrum_after = analyzer.get_spectrum();
        assert!(spectrum_after.peak_magnitude.is_infinite());
    }
}
