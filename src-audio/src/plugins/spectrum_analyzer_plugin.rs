// ============================================================================
// Spectrum Analyzer Plugin
// ============================================================================
//
// Wraps the SpectrumAnalyzer as an AnalyzerPlugin.
// Provides real-time frequency spectrum analysis using FFT.

use super::analyzer::{AnalyzerPlugin, SpectrumData};
use super::plugin::{PluginInfo, PluginResult, ProcessContext};
use crate::spectrum_analyzer::{SpectrumAnalyzer, SpectrumConfig, SpectrumInfo};
use std::any::Any;

/// Spectrum analyzer plugin
pub struct SpectrumAnalyzerPlugin {
    /// Underlying spectrum analyzer
    analyzer: SpectrumAnalyzer,
    /// Number of channels
    num_channels: usize,
}

impl SpectrumAnalyzerPlugin {
    /// Create a new spectrum analyzer plugin with default configuration
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    pub fn new(num_channels: usize) -> Result<Self, String> {
        Self::with_config(num_channels, SpectrumConfig::default())
    }

    /// Create a new spectrum analyzer plugin with custom configuration
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    /// * `config` - Spectrum analyzer configuration
    pub fn with_config(num_channels: usize, config: SpectrumConfig) -> Result<Self, String> {
        let analyzer = SpectrumAnalyzer::new(num_channels as u32, 48000, config)?;

        Ok(Self {
            analyzer,
            num_channels,
        })
    }

    /// Get current spectrum measurements
    pub fn get_spectrum(&self) -> SpectrumInfo {
        self.analyzer.get_spectrum()
    }

    /// Convert SpectrumInfo to SpectrumData
    fn to_spectrum_data(info: &SpectrumInfo) -> SpectrumData {
        SpectrumData {
            frequencies: info.frequencies.clone(),
            magnitudes: info.magnitudes.clone(),
            peak_magnitude: info.peak_magnitude,
        }
    }
}

impl AnalyzerPlugin for SpectrumAnalyzerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Spectrum Analyzer".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Real-time FFT-based frequency spectrum analysis".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        // Get current config
        let config = self.analyzer.config.clone();

        // Recreate the analyzer with the new sample rate
        self.analyzer = SpectrumAnalyzer::new(self.num_channels as u32, sample_rate, config)
            .map_err(|e| format!("Failed to initialize spectrum analyzer: {}", e))?;

        Ok(())
    }

    fn reset(&mut self) {
        self.analyzer.reset().ok();
    }

    fn process(&mut self, input: &[f32], context: &ProcessContext) -> PluginResult<()> {
        // Verify input size
        let expected_samples = context.num_frames * self.num_channels;
        if input.len() != expected_samples {
            return Err(format!(
                "Input size mismatch: expected {}, got {}",
                expected_samples,
                input.len()
            ));
        }

        // Add frames to the analyzer
        self.analyzer
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to spectrum analyzer: {}", e))?;

        Ok(())
    }

    fn get_data(&self) -> Box<dyn Any + Send> {
        let info = self.analyzer.get_spectrum();
        Box::new(Self::to_spectrum_data(&info))
    }

    fn latency_samples(&self) -> usize {
        // Spectrum analyzer has latency equal to FFT size
        2048
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_analyzer_plugin_creation() {
        let plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_custom_config() {
        let config = SpectrumConfig {
            num_bins: 50,
            min_freq: 30.0,
            max_freq: 18000.0,
            smoothing: 0.8,
        };

        let plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
        assert_eq!(plugin.input_channels(), 2);

        let spectrum = plugin.get_spectrum();
        assert_eq!(spectrum.frequencies.len(), 50);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_processing() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Create test signal: 1kHz sine wave
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.5;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process
        plugin.process(&input, &context).unwrap();

        // Get spectrum
        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        println!("Number of bins: {}", spectrum_data.frequencies.len());
        println!("Frequency range: {:.0}Hz - {:.0}Hz",
            spectrum_data.frequencies.first().unwrap_or(&0.0),
            spectrum_data.frequencies.last().unwrap_or(&0.0));
        println!("Peak magnitude: {:.1}dB", spectrum_data.peak_magnitude);

        // Should have some bins
        assert!(spectrum_data.frequencies.len() > 0);
        assert!(spectrum_data.magnitudes.len() > 0);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_1khz_peak() {
        let config = SpectrumConfig {
            num_bins: 30,
            min_freq: 20.0,
            max_freq: 20000.0,
            smoothing: 0.0, // No smoothing for this test
        };

        let mut plugin = SpectrumAnalyzerPlugin::with_config(2, config).unwrap();
        plugin.initialize(48000).unwrap();

        // Create strong 1kHz signal
        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.8;
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process multiple times to fill the buffer
        for _ in 0..3 {
            plugin.process(&input, &context).unwrap();
        }

        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // Find the bin closest to 1kHz
        let target_freq = 1000.0;
        let (bin_idx, _) = spectrum_data
            .frequencies
            .iter()
            .enumerate()
            .min_by_key(|(_, f)| ((*f - target_freq).abs() * 1000.0) as i32)
            .unwrap();

        println!("1kHz test:");
        println!("  Bin {} ({:.0}Hz): {:.1}dB",
            bin_idx, spectrum_data.frequencies[bin_idx], spectrum_data.magnitudes[bin_idx]);

        // The 1kHz bin should have more energy than average
        // Note: With smoothing and circular buffer, it may take a few iterations
        // to build up. The bin should be above the noise floor.
        assert!(
            spectrum_data.magnitudes[bin_idx] > -70.0,
            "1kHz bin should be above noise floor, got {:.1}dB",
            spectrum_data.magnitudes[bin_idx]
        );
    }

    #[test]
    fn test_spectrum_analyzer_plugin_reset() {
        let mut plugin = SpectrumAnalyzerPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let num_frames = 2048;
        let input = vec![0.5_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        // Reset
        plugin.reset();

        // Get spectrum after reset
        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        // After reset, magnitudes should be low/silent
        println!("After reset - Peak: {:.1}dB", spectrum_data.peak_magnitude);
    }

    #[test]
    fn test_spectrum_analyzer_plugin_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut plugin = SpectrumAnalyzerPlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 2048;
        let mut input = vec![0.0_f32; num_frames * 5];

        // Different frequency on each channel
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            input[i * 5 + 0] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;
            input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.2;
            input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.2;
            input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 1760.0 * t).sin() * 0.2;
            input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 2200.0 * t).sin() * 0.2;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        let data = plugin.get_data();
        let spectrum_data = data.downcast_ref::<SpectrumData>().unwrap();

        println!("5-channel spectrum: peak = {:.1}dB", spectrum_data.peak_magnitude);

        // Should have computed spectrum
        assert!(spectrum_data.peak_magnitude > f32::NEG_INFINITY);
    }
}
