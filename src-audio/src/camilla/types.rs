// ============================================================================
// Audio State
// ============================================================================

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::filters::FilterParams;

/// Current state of the audio stream
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AudioState {
    #[default]
    Idle,
    Playing,
    Paused,
    Recording,
    Error,
}

/// Complete audio stream state including playback/recording info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStreamState {
    /// Current state (idle, playing, paused, recording, error)
    pub state: AudioState,
    /// Current playback position in seconds
    pub position_seconds: f64,
    /// Total duration in seconds (if known)
    pub duration_seconds: Option<f64>,
    /// Currently loaded file path
    pub current_file: Option<PathBuf>,
    /// Current output device name
    pub output_device: Option<String>,
    /// Current input device name (for recording)
    pub input_device: Option<String>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Active EQ filters
    pub filters: Vec<FilterParams>,
    /// Channel mapping mode
    pub channel_map_mode: ChannelMapMode,
    /// Playback device channel map (hardware channels)
    pub playback_channel_map: Option<Vec<u16>>,
    /// Capture device channel map (hardware channels)
    pub capture_channel_map: Option<Vec<u16>>,
    /// Recording output file path (for WAV conversion)
    pub recording_output_file: Option<PathBuf>,
    /// Last error message
    pub error_message: Option<String>,
}

impl Default for AudioStreamState {
    fn default() -> Self {
        Self {
            state: AudioState::Idle,
            position_seconds: 0.0,
            duration_seconds: None,
            current_file: None,
            output_device: None,
            input_device: None,
            sample_rate: 48000,
            channels: 2,
            filters: Vec::new(),
            channel_map_mode: ChannelMapMode::Normal,
            playback_channel_map: None,
            capture_channel_map: None,
            recording_output_file: None,
            error_message: None,
        }
    }
}

pub type SharedAudioStreamState = Arc<Mutex<AudioStreamState>>;

// ============================================================================
// CamillaDSP Configuration Structures
// ============================================================================

/// Top-level CamillaDSP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CamillaDSPConfig {
    pub devices: DeviceConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<serde_yaml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixers: Option<serde_yaml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<Vec<serde_yaml::Value>>,
}

/// Audio device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub samplerate: u32,
    pub chunksize: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_threshold: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_timeout: Option<f32>,
    /// Enable automatic sample rate adjustment (allows CamillaDSP to adapt to device rate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_rate_adjust: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<CaptureDevice>,
    pub playback: PlaybackDevice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resampler: Option<serde_yaml::Value>,
}

/// Capture device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureDevice {
    #[serde(rename = "type")]
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<ChannelsSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Playback device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackDevice {
    #[serde(rename = "type")]
    pub device_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<ChannelsSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wav_header: Option<bool>,
}

/// Pipeline step in the processing chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    #[serde(rename = "type")]
    pub step_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Channels can be specified either as a count (u16) or as explicit indices (Vec<u16>)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ChannelsSetting {
    Count(u16),
    Indices(Vec<u16>),
}

impl Default for CamillaDSPConfig {
    fn default() -> Self {
        Self {
            devices: DeviceConfig {
                samplerate: 48000,
                chunksize: 1024,
                silence_threshold: None,
                silence_timeout: None,
                enable_rate_adjust: None,
                capture: None,
                playback: PlaybackDevice {
                    device_type: "CoreAudio".to_string(),
                    device: None,
                    filename: None,
                    channels: Some(ChannelsSetting::Count(2)), // Restore individual device channels
                    format: None,
                    wav_header: None,
                },
                resampler: None,
            },
            filters: None,
            mixers: None,
            pipeline: None,
        }
    }
}

impl DeviceConfig {
    pub fn new(
        sample_rate: u32,
        capture: Option<CaptureDevice>,
        playback: PlaybackDevice,
        resampler_config: Option<serde_yaml::Value>,
    ) -> Self {
        let devices = DeviceConfig {
            // Use the requested playback sample rate in the config; add a resampler if needed
            samplerate: sample_rate,
            chunksize: 1024,
            silence_threshold: Some(-60),
            silence_timeout: Some(3.0),
            enable_rate_adjust: Some(true),
            capture,
            playback,
            resampler: resampler_config,
        };
        devices
    }
}

// ============================================================================
// Channel mapping mode
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChannelMapMode {
    Normal,
    Swap,
}
