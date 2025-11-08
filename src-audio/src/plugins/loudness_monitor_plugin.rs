// ============================================================================
// Loudness Monitor Analyzer Plugin
// ============================================================================
//
// Wraps the LoudnessMonitor as an AnalyzerPlugin.
// Provides real-time EBU R128 loudness measurements.

use super::analyzer::{AnalyzerPlugin, LoudnessData};
use super::plugin::{PluginInfo, PluginResult, ProcessContext};
use crate::loudness_monitor::{LoudnessInfo, LoudnessMonitor};
use std::any::Any;

/// Loudness monitor analyzer plugin
pub struct LoudnessMonitorPlugin {
    /// Underlying loudness monitor
    monitor: LoudnessMonitor,
    /// Number of channels
    num_channels: usize,
}

impl LoudnessMonitorPlugin {
    /// Create a new loudness monitor plugin
    ///
    /// # Arguments
    /// * `num_channels` - Number of audio channels to analyze
    pub fn new(num_channels: usize) -> Result<Self, String> {
        let monitor = LoudnessMonitor::new(num_channels as u32, 48000)?;

        Ok(Self {
            monitor,
            num_channels,
        })
    }

    /// Get current loudness measurements
    pub fn get_loudness(&self) -> LoudnessInfo {
        self.monitor.get_loudness()
    }

    /// Convert LoudnessInfo to LoudnessData
    fn to_loudness_data(info: &LoudnessInfo) -> LoudnessData {
        LoudnessData {
            momentary_lufs: info.momentary_lufs,
            shortterm_lufs: info.shortterm_lufs,
            peak: info.peak,
        }
    }
}

impl AnalyzerPlugin for LoudnessMonitorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Loudness Monitor".to_string(),
            version: "1.0.0".to_string(),
            author: "AutoEQ".to_string(),
            description: "Real-time EBU R128 loudness monitoring (LUFS, peaks)".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.num_channels
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        // Recreate the monitor with the new sample rate
        self.monitor = LoudnessMonitor::new(self.num_channels as u32, sample_rate)
            .map_err(|e| format!("Failed to initialize loudness monitor: {}", e))?;

        Ok(())
    }

    fn reset(&mut self) {
        self.monitor.reset().ok();
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

        // Add frames to the monitor
        self.monitor
            .add_frames(input)
            .map_err(|e| format!("Failed to add frames to loudness monitor: {}", e))?;

        Ok(())
    }

    fn get_data(&self) -> Box<dyn Any + Send> {
        let info = self.monitor.get_loudness();
        Box::new(Self::to_loudness_data(&info))
    }

    fn latency_samples(&self) -> usize {
        // EBU R128 has some latency due to the windowing, but it's minimal
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudness_monitor_plugin_creation() {
        let plugin = LoudnessMonitorPlugin::new(2).unwrap();
        assert_eq!(plugin.input_channels(), 2);
    }

    #[test]
    fn test_loudness_monitor_plugin_processing() {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Create test signal: 1kHz sine wave at -20dBFS
        let num_frames = 4800; // 100ms at 48kHz
        let mut input = vec![0.0_f32; num_frames * 2];
        for i in 0..num_frames {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0;
            let sample = phase.sin() * 0.1; // -20dBFS
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        // Process
        plugin.process(&input, &context).unwrap();

        // Get measurements
        let data = plugin.get_data();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        println!("Momentary LUFS: {:.1}", loudness_data.momentary_lufs);
        println!("Short-term LUFS: {:.1}", loudness_data.shortterm_lufs);
        println!("Peak: {:.3}", loudness_data.peak);

        // Peak should be around 0.1
        assert!(
            loudness_data.peak > 0.05 && loudness_data.peak < 0.15,
            "Peak should be around 0.1, got {}",
            loudness_data.peak
        );
    }

    #[test]
    fn test_loudness_monitor_plugin_reset() {
        let mut plugin = LoudnessMonitorPlugin::new(2).unwrap();
        plugin.initialize(48000).unwrap();

        // Process some audio
        let num_frames = 1024;
        let input = vec![0.5_f32; num_frames * 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        // Reset
        plugin.reset();

        // Measurements should be reset
        let data = plugin.get_data();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        // After reset, values should be back to default (negative infinity for LUFS)
        println!("After reset - Momentary: {:.1}, Peak: {:.3}",
            loudness_data.momentary_lufs, loudness_data.peak);
    }

    #[test]
    fn test_loudness_monitor_plugin_multichannel() {
        // Test with 5 channels (5.0 surround)
        let mut plugin = LoudnessMonitorPlugin::new(5).unwrap();
        plugin.initialize(48000).unwrap();

        let num_frames = 1024;
        let mut input = vec![0.0_f32; num_frames * 5];

        // Different signal on each channel
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            input[i * 5 + 0] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.1;
            input[i * 5 + 1] = (2.0 * std::f32::consts::PI * 550.0 * t).sin() * 0.1;
            input[i * 5 + 2] = (2.0 * std::f32::consts::PI * 660.0 * t).sin() * 0.1;
            input[i * 5 + 3] = (2.0 * std::f32::consts::PI * 770.0 * t).sin() * 0.1;
            input[i * 5 + 4] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.1;
        }

        let context = ProcessContext {
            sample_rate: 48000,
            num_frames,
        };

        plugin.process(&input, &context).unwrap();

        let data = plugin.get_data();
        let loudness_data = data.downcast_ref::<LoudnessData>().unwrap();

        println!("5-channel loudness: {:.1} LUFS, peak: {:.3}",
            loudness_data.momentary_lufs, loudness_data.peak);

        assert!(loudness_data.peak > 0.0, "Peak should be non-zero");
    }
}
