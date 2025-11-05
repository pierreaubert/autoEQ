// ============================================================================
// Audio Manager - High-Level API
// ============================================================================

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;

use super::errors::{CamillaError, CamillaResult};
use super::process::CamillaDSPProcess;
use super::types::{AudioState, AudioStreamState, SharedAudioStreamState};
use super::websocket::CamillaWebSocketClient;
use super::config::{
    generate_streaming_config, generate_playback_config, generate_recording_config,
    generate_recording_config_with_output_type, RecordingOutputType,
    write_config_to_temp, convert_raw_to_wav, fix_rf64_wav, validate_config
};
use crate::filters::FilterParams;
use crate::loudness_compensation::LoudnessCompensation;
use crate::decoder::decoder::AudioSpec;

/// High-level audio manager that coordinates CamillaDSP subprocess,
/// WebSocket communication, and state management
pub struct AudioManager {
    process: Arc<Mutex<CamillaDSPProcess>>,
    state: SharedAudioStreamState,
    temp_config_file: Arc<Mutex<Option<NamedTempFile>>>,
}

impl AudioManager {
    /// Create a new AudioManager
    pub fn new(binary_path: PathBuf) -> Self {
        let process = CamillaDSPProcess::new(binary_path);
        Self {
            process: Arc::new(Mutex::new(process)),
            state: Arc::new(Mutex::new(AudioStreamState::default())),
            temp_config_file: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the current state
    pub fn get_state(&self) -> CamillaResult<AudioStreamState> {
        let state = self.state.lock().map_err(|e| {
            CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
        })?;
        Ok(state.clone())
    }

    /// Get shared state handle for external access
    pub fn shared_state(&self) -> SharedAudioStreamState {
        Arc::clone(&self.state)
    }

    /// Take the stdin handle from the CamillaDSP process for writing audio data
    /// This transfers ownership of the stdin handle to the caller
    pub fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        let mut process = self.process.lock().ok()?;
        process.stdin.take()
    }

    /// Start streaming playback from decoded audio (FLAC, MP3, etc.)
    #[allow(clippy::too_many_arguments)]
    pub async fn start_streaming_playback(
        &self,
        audio_spec: AudioSpec,
        output_device: Option<String>,
        filters: Vec<FilterParams>,
        channel_map_mode: super::types::ChannelMapMode,
        output_map: Option<Vec<u16>>,
        loudness: Option<LoudnessCompensation>,
    ) -> CamillaResult<()> {
        println!(
            "[AudioManager] Starting streaming playback: {}Hz, {}ch, {} filters",
            audio_spec.sample_rate,
            audio_spec.channels,
            filters.len()
        );

        // Update state to reflect we're starting
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Idle;
            state.current_file = None; // No file for streaming
            state.output_device = output_device.clone();
            state.sample_rate = audio_spec.sample_rate;
            state.channels = audio_spec.channels;
            state.filters = filters.clone();
            state.channel_map_mode = channel_map_mode;
            state.playback_channel_map = output_map.clone();
            state.error_message = None;
        }

        // Generate config for streaming (stdin input)
        let config = generate_streaming_config(
            output_device.as_deref(),
            audio_spec.sample_rate,
            audio_spec.channels,
            &filters,
            channel_map_mode,
            output_map.as_deref(),
            loudness.as_ref(),
        )?;

        // Print the configuration for debugging
        println!("[CamillaDSP] Generated configuration:");
        println!("{}", serde_yaml::to_string(&config).unwrap());

        // Write config to temp file
        let temp_file = write_config_to_temp(&config)?;
        let config_path = temp_file.path().to_path_buf();

        // Store temp file to keep it alive
        {
            let mut temp_config = self.temp_config_file.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!(
                    "Failed to lock temp config: {}",
                    e
                ))
            })?;
            *temp_config = Some(temp_file);
        }

        // Start the CamillaDSP process
        {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.start(config_path)?;
        }

        // Wait for WebSocket to be ready and verify connection
        let ws_url = {
            let process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        // Use shorter retry for faster startup
        client
            .connect_with_retry(3, std::time::Duration::from_millis(300))
            .await?;

        // Update state to playing
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Playing;
            state.position_seconds = 0.0;
        }

        println!("[AudioManager] Streaming playback started successfully");
        Ok(())
    }

    /// Start playback with the given audio file and filters
    #[allow(clippy::too_many_arguments)]
    pub async fn start_playback(
        &self,
        audio_file: PathBuf,
        output_device: Option<String>,
        sample_rate: u32,
        channels: u16,
        filters: Vec<FilterParams>,
        channel_map_mode: super::types::ChannelMapMode,
        output_map: Option<Vec<u16>>,
        loudness: Option<LoudnessCompensation>,
    ) -> CamillaResult<()> {
        println!(
            "[AudioManager] Starting playback: {:?} ({}Hz, {}ch, {} filters)",
            audio_file,
            sample_rate,
            channels,
            filters.len()
        );

        // Update state to reflect we're starting
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Idle;
            state.current_file = Some(audio_file.clone());
            state.output_device = output_device.clone();
            state.sample_rate = sample_rate;
            state.channels = channels;
            state.filters = filters.clone();
            state.channel_map_mode = channel_map_mode;
            state.playback_channel_map = output_map.clone();
            state.error_message = None;
        }

        // Verify audio file exists
        if !audio_file.exists() {
            let error = format!("Audio file not found: {:?}", audio_file);
            self.set_error(&error)?;
            return Err(CamillaError::IOError(error));
        }

        // Generate config
        let config = generate_playback_config(
            &audio_file,
            output_device.as_deref(),
            sample_rate,
            channels,
        )?;

        // Validate the configuration
        validate_config(&config, "playback")?;

        // Print the configuration for debugging
        println!("[CamillaDSP] Generated configuration:");
        println!("{}", serde_yaml::to_string(&config).unwrap());

        // Write config to temp file
        let temp_file = write_config_to_temp(&config)?;
        let config_path = temp_file.path().to_path_buf();

        // Store temp file to keep it alive
        {
            let mut temp_config = self.temp_config_file.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!(
                    "Failed to lock temp config: {}",
                    e
                ))
            })?;
            *temp_config = Some(temp_file);
        }

        // Start the CamillaDSP process
        {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.start(config_path)?;
        }

        // Wait for WebSocket to be ready and verify connection
        let ws_url = {
            let process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        // Use shorter retry for faster startup
        client
            .connect_with_retry(3, std::time::Duration::from_millis(300))
            .await?;

        // Update state to playing
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Playing;
            state.position_seconds = 0.0;
        }

        println!("[AudioManager] Playback started successfully");
        Ok(())
    }

    /// Stop playback
    pub async fn stop_playback(&self) -> CamillaResult<()> {
        println!("[AudioManager] Stopping playback");

        // Try to stop via WebSocket first
        let ws_url = {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            if !process.is_running() {
                println!("[AudioManager] Process not running, nothing to stop");
                return Ok(());
            }
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        let _ = client.stop().await; // Ignore errors, we'll kill the process anyway

        // Stop the process
        {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.stop()?;
        }

        // Update state
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Idle;
            state.position_seconds = 0.0;
            state.current_file = None;
        }

        // Clean up temp config file
        {
            let mut temp_config = self.temp_config_file.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!(
                    "Failed to lock temp config: {}",
                    e
                ))
            })?;
            *temp_config = None;
        }

        println!("[AudioManager] Playback stopped");
        Ok(())
    }

    /// Update EQ filters and loudness in real-time without restarting CamillaDSP
    /// Supports both streaming (stdin) and file-based playback modes
    pub async fn update_filters(
        &self,
        filters: Vec<FilterParams>,
        loudness: Option<LoudnessCompensation>,
    ) -> CamillaResult<()> {
        println!(
            "[AudioManager] Updating {} filters{}",
            filters.len(),
            if loudness.is_some() {
                " with loudness compensation"
            } else {
                ""
            }
        );

        // Check if CamillaDSP is running
        {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            if !process.is_running() {
                println!("[AudioManager] CamillaDSP not running, skipping filter update");
                return Ok(()); // Silently succeed - filters will be applied when playback starts
            }
        }

        // Validate filters
        for filter in &filters {
            filter.validate()?;
        }

        // Validate loudness if provided
        if let Some(ref lc) = loudness {
            lc.validate()?;
        }

        // Get current state to determine mode and rebuild config
        let (
            is_streaming,
            audio_file,
            output_device,
            sample_rate,
            channels,
            channel_map_mode,
            playback_channel_map,
        ) = {
            let state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;

            // Streaming mode has no current_file
            let is_streaming = state.current_file.is_none();
            let file = state.current_file.clone();

            (
                is_streaming,
                file,
                state.output_device.clone(),
                state.sample_rate,
                state.channels,
                state.channel_map_mode,
                state.playback_channel_map.clone(),
            )
        };

        // Generate new config based on mode (streaming vs file-based)
        let config = if is_streaming {
            println!("[AudioManager] Updating filters for streaming mode");
            generate_streaming_config(
                output_device.as_deref(),
                sample_rate,
                channels,
                &filters,
                channel_map_mode,
                playback_channel_map.as_deref(),
                loudness.as_ref(),
            )?
        } else {
            println!("[AudioManager] Updating filters for file playback mode");
            let file = audio_file.ok_or(CamillaError::ProcessNotRunning)?;
            generate_playback_config(
                &file,
                output_device.as_deref(),
                sample_rate,
                channels,
            )?
        };

        let config_yaml = serde_yaml::to_string(&config)?;

        // Send config update via WebSocket (hot-reload without restarting)
        let ws_url = {
            let process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        client.set_config(config_yaml).await?;

        // Update state with new filters only after successful WebSocket update
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.filters = filters;
        }

        println!("[AudioManager] Filters updated successfully via WebSocket");
        Ok(())
    }

    /// Start recording from input device
    pub async fn start_recording(
        &self,
        input_device: Option<String>,
        output_file: PathBuf,
        sample_rate: u32,
        channels: u16,
        input_map: Option<Vec<u16>>,
    ) -> CamillaResult<()> {
        println!(
            "[AudioManager] Starting recording: {:?} ({}Hz, {}ch)",
            output_file, sample_rate, channels
        );

        // Update state
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.state = AudioState::Recording;
            state.input_device = input_device.clone();
            state.sample_rate = sample_rate;
            state.channels = channels;
            state.error_message = None;
            state.capture_channel_map = input_map.clone();
            state.recording_output_file = Some(output_file.clone());
            state.output_device = None; // No output device for recording
        }

        // Generate recording config
        let config = generate_recording_config(
            input_device.as_deref(),
            &output_file,
            sample_rate,
            channels,
            input_map.as_deref(),
        )?;

        // Validate the configuration
        validate_config(&config, "recording")?;

        // Write config to temp file
        let temp_file = write_config_to_temp(&config)?;
        let config_path = temp_file.path().to_path_buf();

        // Store temp file
        {
            let mut temp_config = self.temp_config_file.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!(
                    "Failed to lock temp config: {}",
                    e
                ))
            })?;
            *temp_config = Some(temp_file);
        }

        // Start the CamillaDSP process
        {
            let mut process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.start(config_path)?;
        }

        println!("[AudioManager] Recording started");
        Ok(())
    }

    /// Stop recording
    pub async fn stop_recording(&self) -> CamillaResult<()> {
        println!("[AudioManager] Stopping recording");

        // Get recording parameters before stopping
        let (output_file, sample_rate, channels) = {
            let state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;

            let output_file = state
                .recording_output_file
                .clone()
                .ok_or_else(|| CamillaError::ProcessNotRunning)?;

            (output_file, state.sample_rate, state.channels)
        };

        // Stop the CamillaDSP process (writes raw FLOAT32LE file)
        self.stop_playback().await?;

        // Convert raw file to WAV format
        // CamillaDSP writes to the specified output file as raw FLOAT32LE
        // We need to convert it to a proper WAV file
        let raw_file = output_file.with_extension("");
        let raw_file = PathBuf::from(format!("{}.raw", raw_file.display()));

        // CamillaDSP with wav_header=true writes a complete WAV file directly
        // But it may use RF64 format with 0xFFFFFFFF placeholders - fix those
        if output_file.exists() {
            fix_rf64_wav(&output_file)?;
            println!("[AudioManager] Recording saved as WAV: {:?}", output_file);
        } else if raw_file.exists() {
            // Fallback: if we get a .raw file instead, convert it
            println!("[AudioManager] Converting raw audio to WAV format...");
            convert_raw_to_wav(&raw_file, &output_file, sample_rate, channels)?;
            // Remove raw file after conversion
            let _ = std::fs::remove_file(&raw_file);
            println!("[AudioManager] Recording saved as WAV: {:?}", output_file);
        } else {
            println!("[AudioManager] Warning: Recording file not found");
        }

        // Clear recording state
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.recording_output_file = None;
        }

        Ok(())
    }

    /// Check if audio is currently playing
    pub fn is_playing(&self) -> CamillaResult<bool> {
        let state = self.state.lock().map_err(|e| {
            CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
        })?;
        Ok(state.state == AudioState::Playing)
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> CamillaResult<bool> {
        let state = self.state.lock().map_err(|e| {
            CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
        })?;
        Ok(state.state == AudioState::Recording)
    }

    /// Get signal peak from WebSocket (for VU meters)
    pub async fn get_signal_peak(&self) -> CamillaResult<f32> {
        let ws_url = {
            let process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        client.get_playback_signal_peak().await
    }

    /// Set error state
    fn set_error(&self, error: &str) -> CamillaResult<()> {
        let mut state = self.state.lock().map_err(|e| {
            CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
        })?;
        state.state = AudioState::Error;
        state.error_message = Some(error.to_string());
        Ok(())
    }
}
