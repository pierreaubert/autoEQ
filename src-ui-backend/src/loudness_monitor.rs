use ebur128::{EbuR128, Mode};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Real-time loudness measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoudnessInfo {
    /// Momentary loudness (M) - 400ms window, updated every 100ms
    /// Range: -inf to ~0 LUFS (typical: -40 to 0)
    pub momentary_lufs: f64,

    /// Short-term loudness (S) - 3 second window
    /// Range: -inf to ~0 LUFS (typical: -40 to 0)
    pub shortterm_lufs: f64,

    /// Current sample peak across all channels (0.0 to 1.0+)
    pub peak: f64,
}

impl Default for LoudnessInfo {
    fn default() -> Self {
        Self {
            momentary_lufs: f64::NEG_INFINITY,
            shortterm_lufs: f64::NEG_INFINITY,
            peak: 0.0,
        }
    }
}

/// Thread-safe loudness monitor for real-time audio analysis
pub struct LoudnessMonitor {
    /// EBU R128 analyzer
    ebur128: Arc<Mutex<EbuR128>>,
    /// Number of channels
    channels: u32,
    /// Sample rate
    sample_rate: u32,
    /// Current measurements
    current_info: Arc<Mutex<LoudnessInfo>>,
}

impl LoudnessMonitor {
    /// Create a new loudness monitor
    ///
    /// # Arguments
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(channels: u32, sample_rate: u32) -> Result<Self, String> {
        let ebur128 = EbuR128::new(channels, sample_rate, Mode::M | Mode::S | Mode::SAMPLE_PEAK)
            .map_err(|e| format!("Failed to create EBU R128 analyzer: {:?}", e))?;

        Ok(Self {
            ebur128: Arc::new(Mutex::new(ebur128)),
            channels,
            sample_rate,
            current_info: Arc::new(Mutex::new(LoudnessInfo::default())),
        })
    }

    /// Add audio frames to the analyzer
    ///
    /// # Arguments
    /// * `samples` - Interleaved f32 samples in range [-1.0, 1.0]
    ///
    /// # Returns
    /// Ok(()) if successful, Err if analysis fails
    pub fn add_frames(&self, samples: &[f32]) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Add frames to the analyzer
        ebur.add_frames_f32(samples)
            .map_err(|e| format!("Failed to add frames: {:?}", e))?;

        // Update measurements
        let momentary_lufs = ebur.loudness_momentary().unwrap_or(f64::NEG_INFINITY);

        let shortterm_lufs = ebur.loudness_shortterm().unwrap_or(f64::NEG_INFINITY);

        // Get peak across all channels
        let mut peak = 0.0f64;
        for ch in 0..self.channels {
            if let Ok(ch_peak) = ebur.sample_peak(ch) {
                peak = peak.max(ch_peak);
            }
        }

        // Update shared state
        {
            let mut info = self.current_info.lock().unwrap();
            info.momentary_lufs = momentary_lufs;
            info.shortterm_lufs = shortterm_lufs;
            info.peak = peak;
        }

        Ok(())
    }

    /// Get the current loudness measurements
    pub fn get_loudness(&self) -> LoudnessInfo {
        let info = self.current_info.lock().unwrap();
        info.clone()
    }

    /// Reset the monitor (clear all history)
    pub fn reset(&self) -> Result<(), String> {
        let mut ebur = self.ebur128.lock().unwrap();

        // Create a new EBU R128 instance to reset state
        let new_ebur = EbuR128::new(
            self.channels,
            self.sample_rate,
            Mode::M | Mode::S | Mode::SAMPLE_PEAK,
        )
        .map_err(|e| format!("Failed to reset analyzer: {:?}", e))?;

        *ebur = new_ebur;

        // Reset measurements
        {
            let mut info = self.current_info.lock().unwrap();
            *info = LoudnessInfo::default();
        }

        Ok(())
    }

    /// Get the sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels
    pub fn channels(&self) -> u32 {
        self.channels
    }
}

impl Clone for LoudnessMonitor {
    fn clone(&self) -> Self {
        Self {
            ebur128: Arc::clone(&self.ebur128),
            channels: self.channels,
            sample_rate: self.sample_rate,
            current_info: Arc::clone(&self.current_info),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loudness_monitor_creation() {
        let monitor = LoudnessMonitor::new(2, 48000).unwrap();
        assert_eq!(monitor.channels(), 2);
        assert_eq!(monitor.sample_rate(), 48000);

        let info = monitor.get_loudness();
        assert!(info.momentary_lufs.is_infinite() && info.momentary_lufs < 0.0);
        assert!(info.shortterm_lufs.is_infinite() && info.shortterm_lufs < 0.0);
        assert_eq!(info.peak, 0.0);
    }

    #[test]
    fn test_add_frames() {
        let monitor = LoudnessMonitor::new(2, 48000).unwrap();

        // Generate 1 second of 1kHz sine wave at -20dBFS
        let sample_rate = 48000;
        let frequency = 1000.0;
        let amplitude = 0.1; // -20dBFS
        let samples: Vec<f32> = (0..sample_rate)
            .flat_map(|i| {
                let t = i as f32 / sample_rate as f32;
                let value = amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin();
                vec![value, value] // Stereo
            })
            .collect();

        // Add frames
        monitor.add_frames(&samples).unwrap();

        // Check that we got valid measurements
        let info = monitor.get_loudness();
        assert!(info.momentary_lufs.is_finite());
        assert!(info.shortterm_lufs.is_finite());
        assert!(info.peak > 0.0 && info.peak <= 1.0);

        // Momentary and short-term should be roughly similar for continuous signal
        println!("Momentary: {} LUFS", info.momentary_lufs);
        println!("Short-term: {} LUFS", info.shortterm_lufs);
        println!("Peak: {}", info.peak);
    }

    #[test]
    fn test_reset() {
        let monitor = LoudnessMonitor::new(2, 48000).unwrap();

        // Add some audio
        let samples = vec![0.1f32; 48000 * 2]; // 1 second stereo
        monitor.add_frames(&samples).unwrap();

        // Verify we have measurements
        let info_before = monitor.get_loudness();
        assert!(info_before.momentary_lufs.is_finite());

        // Reset
        monitor.reset().unwrap();

        // Verify measurements are cleared
        let info_after = monitor.get_loudness();
        assert!(info_after.momentary_lufs.is_infinite() && info_after.momentary_lufs < 0.0);
        assert!(info_after.shortterm_lufs.is_infinite() && info_after.shortterm_lufs < 0.0);
    }
}
