use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ChildStdin;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// Import logging macros from autoeq-env
use autoeq_env::{log_debug, log_error, log_info, log_warn};

const FRAMES_PER_CHUNK: usize = 256;

use crate::camilla::{AudioManager, ChannelMapMode};
use crate::filters::FilterParams;
use crate::loudness_compensation::LoudnessCompensation;
use crate::loudness_monitor::{LoudnessInfo, LoudnessMonitor};
use crate::plugins::PluginHost;
use crate::spectrum_analyzer::{SpectrumAnalyzer, SpectrumConfig, SpectrumInfo};
use crate::{
    AudioDecoderError, AudioDecoderResult, AudioFormat, AudioSpec, create_decoder, probe_file,
};

/// High-level audio streaming manager that combines FLAC decoding with CamillaDSP processing
pub struct AudioStreamingManager {
    /// CamillaDSP audio manager
    audio_manager: AudioManager,
    /// Current decoder thread handle
    decoder_thread: Option<JoinHandle<()>>,
    /// Command channel for controlling the decoder
    command_tx: Option<Sender<StreamingCommand>>,
    /// Event channel for receiving decoder events (wrapped in Arc<Mutex<>> for thread-safety)
    event_rx: Option<Arc<Mutex<Receiver<StreamingEvent>>>>,
    /// Current streaming state
    state: Arc<Mutex<StreamingState>>,
    /// Current audio file information
    current_audio_info: Option<AudioFileInfo>,
    /// Number of CamillaDSP chunks to buffer (1 chunk = 1024 frames)
    /// Range: 32 (low latency) to 1024 (high reliability)
    buffer_chunks: usize,
    /// Shared underrun counter for adaptive buffering
    underrun_count: Arc<Mutex<usize>>,
    /// Real-time loudness monitor (optional)
    loudness_monitor: Option<Arc<Mutex<LoudnessMonitor>>>,
    /// Real-time spectrum analyzer (optional)
    spectrum_monitor: Option<Arc<Mutex<SpectrumAnalyzer>>>,
    /// Plugin host for audio processing (optional)
    plugin_host: Option<Arc<Mutex<PluginHost>>>,
}

/// Commands for controlling the streaming decoder
#[derive(Debug, Clone)]
pub enum StreamingCommand {
    /// Start streaming playback
    Start,
    /// Pause streaming (decoder continues, but no data sent to CamillaDSP)
    Pause,
    /// Resume streaming
    Resume,
    /// Stop streaming and cleanup
    Stop,
    /// Seek to position in seconds
    SeekSeconds(f64),
}

/// Events emitted by the streaming decoder
#[derive(Debug, Clone)]
pub enum StreamingEvent {
    /// State has changed
    StateChanged(StreamingState),
    /// End of stream reached (song completed)
    EndOfStream,
    /// An error occurred
    Error(String),
}

/// Current state of the streaming manager
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Idle,
    Loading,
    Ready,
    Playing,
    Paused,
    Seeking,
    Error,
}

/// Information about the currently loaded audio file
#[derive(Debug, Clone)]
pub struct AudioFileInfo {
    pub path: PathBuf,
    pub format: AudioFormat,
    pub spec: AudioSpec,
    pub duration_seconds: Option<f64>,
}

impl AudioStreamingManager {
    /// Create a new streaming manager with the given CamillaDSP binary path
    pub fn new(camilla_binary_path: PathBuf) -> Self {
        Self {
            audio_manager: AudioManager::new(camilla_binary_path),
            decoder_thread: None,
            command_tx: None,
            event_rx: None,
            state: Arc::new(Mutex::new(StreamingState::Idle)),
            current_audio_info: None,
            buffer_chunks: 128, // Default: balanced performance
            underrun_count: Arc::new(Mutex::new(0)),
            loudness_monitor: None,
            spectrum_monitor: None,
            plugin_host: None,
        }
    }

    /// Set the number of CamillaDSP chunks to buffer
    /// - 32: Low latency, high performance systems
    /// - 128: Balanced (default)
    /// - 1024: High reliability, poor network/slow systems
    pub fn set_buffer_chunks(&mut self, chunks: usize) {
        self.buffer_chunks = chunks.clamp(32, 1024);
    }

    /// Get the current buffer size in chunks
    pub fn buffer_chunks(&self) -> usize {
        self.buffer_chunks
    }

    /// Load an audio file and prepare for streaming
    pub async fn load_file<P: AsRef<Path>>(
        &mut self,
        file_path: P,
    ) -> AudioDecoderResult<AudioFileInfo> {
        let path = file_path.as_ref().to_path_buf();

        self.set_state(StreamingState::Loading);

        // Stop any current playback
        self.stop().await?;

        println!("[AudioStreamingManager] Loading file: {:?}", path);

        // Probe the file to get format and spec information
        let (format, spec) = probe_file(&path)?;

        let duration_seconds = spec.duration().map(|d| d.as_secs_f64());

        let audio_info = AudioFileInfo {
            path: path.clone(),
            format,
            spec,
            duration_seconds,
        };

        println!(
            "[AudioStreamingManager] Loaded {} file: {}Hz, {}ch, {:?}s duration",
            audio_info.format,
            audio_info.spec.sample_rate,
            audio_info.spec.channels,
            audio_info.duration_seconds
        );

        self.current_audio_info = Some(audio_info.clone());

        self.set_state(StreamingState::Ready);

        Ok(audio_info)
    }

    /// Start streaming playback with the given settings
    pub async fn start_playback(
        &mut self,
        output_device: Option<String>,
        filters: Vec<FilterParams>,
        channel_map_mode: ChannelMapMode,
        output_map: Option<Vec<u16>>,
        loudness: Option<LoudnessCompensation>,
    ) -> AudioDecoderResult<()> {
        let audio_info = self
            .current_audio_info
            .as_ref()
            .ok_or_else(|| AudioDecoderError::ConfigError("No file loaded".to_string()))?;

        println!("[AudioStreamingManager] Starting playback");

        // Determine the channel count that will be sent to CamillaDSP
        // If plugin host is enabled, use its output channel count, otherwise use file's channel count
        let mut spec_for_camilla = audio_info.spec.clone();
        if let Some(ref plugin_host) = self.plugin_host {
            let output_channels = plugin_host
                .lock()
                .map_err(|e| {
                    AudioDecoderError::ConfigError(format!("Failed to lock plugin host: {}", e))
                })?
                .output_channels();

            if output_channels != audio_info.spec.channels as usize {
                println!(
                    "[AudioStreamingManager] Plugin host changes channel count: {} → {}",
                    audio_info.spec.channels, output_channels
                );
                spec_for_camilla.channels = output_channels as u16;
            }
        }

        // Start CamillaDSP with streaming configuration
        self.audio_manager
            .start_streaming_playback(
                spec_for_camilla,
                output_device,
                filters,
                channel_map_mode,
                output_map,
                loudness,
            )
            .await
            .map_err(|e| AudioDecoderError::ConfigError(format!("CamillaDSP error: {}", e)))?;

        // Start the decoder thread (it will pre-buffer before playing)
        self.start_decoder_thread().await?;

        // Send start command - decoder will honor it after pre-buffering
        if let Some(ref cmd_tx) = self.command_tx {
            cmd_tx
                .send(StreamingCommand::Start)
                .map_err(|_| AudioDecoderError::StreamEnded)?;
        }

        // Note: State will be set to Playing by the decoder thread after pre-buffering completes

        Ok(())
    }

    /// Pause playback
    pub async fn pause(&self) -> AudioDecoderResult<()> {
        println!("[AudioStreamingManager] Pausing playback");

        if let Some(ref cmd_tx) = self.command_tx {
            cmd_tx
                .send(StreamingCommand::Pause)
                .map_err(|_| AudioDecoderError::StreamEnded)?;
        }

        self.set_state(StreamingState::Paused);

        Ok(())
    }

    /// Resume playback
    pub async fn resume(&self) -> AudioDecoderResult<()> {
        println!("[AudioStreamingManager] Resuming playback");

        if let Some(ref cmd_tx) = self.command_tx {
            cmd_tx
                .send(StreamingCommand::Resume)
                .map_err(|_| AudioDecoderError::StreamEnded)?;
        }

        self.set_state(StreamingState::Playing);

        Ok(())
    }

    /// Stop playback
    pub async fn stop(&mut self) -> AudioDecoderResult<()> {
        println!("[AudioStreamingManager] Stopping playback");

        // Send stop command to decoder thread
        if let Some(ref cmd_tx) = self.command_tx {
            let _ = cmd_tx.send(StreamingCommand::Stop);
        }

        // Wait for decoder thread to finish
        if let Some(handle) = self.decoder_thread.take() {
            let _ = handle.join();
        }

        // Stop CamillaDSP
        self.audio_manager
            .stop_playback()
            .await
            .map_err(|e| AudioDecoderError::ConfigError(format!("CamillaDSP error: {}", e)))?;

        // Clear command and event channels
        self.command_tx = None;
        self.event_rx = None;

        self.set_state(StreamingState::Idle);

        Ok(())
    }

    /// Seek to a specific position in seconds
    pub async fn seek(&self, seconds: f64) -> AudioDecoderResult<()> {
        println!("[AudioStreamingManager] Seeking to {}s", seconds);

        if let Some(ref cmd_tx) = self.command_tx {
            cmd_tx
                .send(StreamingCommand::SeekSeconds(seconds))
                .map_err(|_| AudioDecoderError::StreamEnded)?;
        }

        self.set_state(StreamingState::Seeking);

        Ok(())
    }

    /// Get current streaming state
    fn set_state(&self, new_state: StreamingState) {
        {
            let mut state = self.state.lock().unwrap();
            *state = new_state;
        }
    }

    /// Get current streaming state
    pub fn get_state(&self) -> StreamingState {
        *self.state.lock().unwrap()
    }

    /// Get current audio file information
    pub fn get_audio_info(&self) -> Option<&AudioFileInfo> {
        self.current_audio_info.as_ref()
    }

    /// Get the underlying AudioManager for access to CamillaDSP features
    pub fn audio_manager(&self) -> &AudioManager {
        &self.audio_manager
    }

    /// Enable real-time loudness monitoring
    pub fn enable_loudness_monitoring(&mut self) -> Result<(), String> {
        let audio_info = self
            .current_audio_info
            .as_ref()
            .ok_or_else(|| "No audio file loaded".to_string())?;

        let monitor =
            LoudnessMonitor::new(audio_info.spec.channels as u32, audio_info.spec.sample_rate)?;

        self.loudness_monitor = Some(Arc::new(Mutex::new(monitor)));
        Ok(())
    }

    /// Disable real-time loudness monitoring
    pub fn disable_loudness_monitoring(&mut self) {
        self.loudness_monitor = None;
    }

    /// Get current loudness measurements (if monitoring is enabled)
    pub fn get_loudness(&self) -> Option<LoudnessInfo> {
        self.loudness_monitor
            .as_ref()
            .and_then(|m| m.lock().ok().map(|lm| lm.get_loudness()))
    }

    /// Check if loudness monitoring is enabled
    pub fn is_loudness_monitoring_enabled(&self) -> bool {
        self.loudness_monitor.is_some()
    }

    /// Enable real-time spectrum monitoring
    pub fn enable_spectrum_monitoring(&mut self) -> Result<(), String> {
        let audio_info = self
            .current_audio_info
            .as_ref()
            .ok_or_else(|| "No audio file loaded".to_string())?;

        let config = SpectrumConfig::default();
        let analyzer = SpectrumAnalyzer::new(
            audio_info.spec.channels as u32,
            audio_info.spec.sample_rate,
            config,
        )?;

        self.spectrum_monitor = Some(Arc::new(Mutex::new(analyzer)));
        Ok(())
    }

    /// Disable real-time spectrum monitoring
    pub fn disable_spectrum_monitoring(&mut self) {
        self.spectrum_monitor = None;
    }

    /// Get current spectrum measurements (if monitoring is enabled)
    pub fn get_spectrum(&self) -> Option<SpectrumInfo> {
        self.spectrum_monitor
            .as_ref()
            .map(|m| m.lock().unwrap().get_spectrum())
    }

    /// Check if spectrum monitoring is enabled
    pub fn is_spectrum_monitoring_enabled(&self) -> bool {
        self.spectrum_monitor.is_some()
    }

    /// Enable plugin host for custom audio processing
    /// The plugin host is inserted in the audio pipeline before CamillaDSP
    pub fn enable_plugin_host(&mut self) -> Result<(), String> {
        let audio_info = self
            .current_audio_info
            .as_ref()
            .ok_or_else(|| "No audio file loaded".to_string())?;

        let host = PluginHost::new(
            audio_info.spec.channels as usize,
            audio_info.spec.sample_rate,
        );
        self.plugin_host = Some(Arc::new(Mutex::new(host)));
        Ok(())
    }

    /// Disable plugin host
    pub fn disable_plugin_host(&mut self) {
        self.plugin_host = None;
    }

    /// Check if plugin host is enabled
    pub fn is_plugin_host_enabled(&self) -> bool {
        self.plugin_host.is_some()
    }

    /// Get mutable access to the plugin host
    /// Use this to add/remove plugins and configure parameters
    pub fn with_plugin_host<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut PluginHost) -> R,
    {
        let host = self
            .plugin_host
            .as_ref()
            .ok_or_else(|| "Plugin host not enabled".to_string())?;

        let mut host_lock = host
            .lock()
            .map_err(|e| format!("Failed to lock host: {}", e))?;
        Ok(f(&mut host_lock))
    }

    /// Try to receive the next event from the decoder thread (non-blocking)
    pub fn try_recv_event(&self) -> Option<StreamingEvent> {
        self.event_rx
            .as_ref()
            .and_then(|rx| rx.lock().unwrap().try_recv().ok())
    }

    /// Drain all pending events from the decoder thread
    pub fn drain_events(&self) -> Vec<StreamingEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_recv_event() {
            events.push(event);
        }
        events
    }

    /// Start the decoder thread that feeds PCM data to CamillaDSP via stdin
    async fn start_decoder_thread(&mut self) -> AudioDecoderResult<()> {
        let audio_info = self.current_audio_info.as_ref().unwrap();
        let path = audio_info.path.clone();
        let state = Arc::clone(&self.state);
        let buffer_chunks = self.buffer_chunks;
        let underrun_count = Arc::clone(&self.underrun_count);

        // Create command and event channels
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        self.command_tx = Some(cmd_tx);
        self.event_rx = Some(Arc::new(Mutex::new(event_rx)));

        // Get the CamillaDSP stdin handle from the audio manager
        let stdin = self.audio_manager.take_stdin().ok_or_else(|| {
            AudioDecoderError::ConfigError("CamillaDSP stdin not available".to_string())
        })?;

        // Clone loudness monitor, spectrum analyzer, and plugin host for decoder thread
        let loudness_monitor = self.loudness_monitor.clone();
        let spectrum_monitor = self.spectrum_monitor.clone();
        let plugin_host = self.plugin_host.clone();

        // Spawn decoder thread
        let handle = thread::spawn(move || {
            if let Err(e) = Self::decoder_thread_main(
                path,
                state,
                cmd_rx,
                event_tx,
                stdin,
                buffer_chunks,
                underrun_count,
                loudness_monitor,
                spectrum_monitor,
                plugin_host,
            ) {
                eprintln!("[AudioStreamingManager] Decoder thread error: {:?}", e);
            }
        });

        self.decoder_thread = Some(handle);

        Ok(())
    }

    /// Main decoder thread function with pre-buffering and adaptive buffering
    fn decoder_thread_main(
        path: PathBuf,
        state: Arc<Mutex<StreamingState>>,
        cmd_rx: Receiver<StreamingCommand>,
        event_tx: Sender<StreamingEvent>,
        mut stdin: ChildStdin,
        mut buffer_chunks: usize,
        underrun_count: Arc<Mutex<usize>>,
        loudness_monitor: Option<Arc<Mutex<LoudnessMonitor>>>,
        spectrum_monitor: Option<Arc<Mutex<SpectrumAnalyzer>>>,
        plugin_host: Option<Arc<Mutex<PluginHost>>>,
    ) -> AudioDecoderResult<()> {
        log_info!("Decoder thread starting for: {:?}", path);
        log_debug!("Buffer size: {} chunks (1024 frames each)", buffer_chunks);

        // Helper function to process audio through plugins
        let process_through_plugins =
            |samples: &[f32], plugin_host: &Option<Arc<Mutex<PluginHost>>>| -> Vec<f32> {
                if let Some(host) = plugin_host {
                    if let Ok(mut host_lock) = host.lock() {
                        let num_frames = samples.len() / host_lock.input_channels();
                        let output_size = num_frames * host_lock.output_channels();
                        let mut output = vec![0.0; output_size];

                        if let Err(e) = host_lock.process(samples, &mut output) {
                            log_warn!("Plugin processing error: {}", e);
                            // On error, return original samples
                            return samples.to_vec();
                        }
                        return output;
                    }
                }
                // No plugin host or lock failed - pass through
                samples.to_vec()
            };

        // Create decoder
        let mut decoder = create_decoder(&path)?;
        let spec = decoder.spec().clone();

        // Determine actual output channel count (may differ from input if plugin host changes it)
        let output_channels = if let Some(ref host) = plugin_host {
            host.lock().unwrap().output_channels()
        } else {
            spec.channels as usize
        };

        log_info!(
            "Audio pipeline: file {}ch → output {}ch",
            spec.channels,
            output_channels
        );

        // Calculate buffer target in frames (1 chunk = 1024 frames)
        let mut target_buffer_frames = buffer_chunks * FRAMES_PER_CHUNK;
        let mut last_underrun_check = std::time::Instant::now();

        // Buffer for decoded audio data (stores raw PCM bytes)
        // Use output_channels for buffer capacity since that's what will be written
        let mut audio_buffer: VecDeque<u8> =
            VecDeque::with_capacity(target_buffer_frames * output_channels * 4);
        let mut buffered_frames: usize = 0;
        let mut playing = false;
        let mut pre_buffered = false;
        let mut packet_count = 0usize;

        // Track when buffer becomes empty for too long
        let mut buffer_empty_since: Option<std::time::Instant> = None;
        const BUFFER_EMPTY_TIMEOUT: Duration = Duration::from_secs(3);

        log_info!(
            "Pre-buffering {} frames ({:.2}s at {}Hz)...",
            target_buffer_frames,
            target_buffer_frames as f64 / spec.sample_rate as f64,
            spec.sample_rate
        );

        loop {
            // Check for commands (batched - check every 10 packets to reduce overhead)
            if packet_count.is_multiple_of(10)
                && let Ok(command) = cmd_rx.try_recv()
            {
                match command {
                    StreamingCommand::Start | StreamingCommand::Resume => {
                        playing = true;
                        // Reset empty buffer timer when starting/resuming
                        buffer_empty_since = None;
                        if pre_buffered {
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamingState::Playing;
                            let _ = event_tx
                                .send(StreamingEvent::StateChanged(StreamingState::Playing));
                        }
                    }
                    StreamingCommand::Pause => {
                        playing = false;
                        // Reset empty buffer timer when pausing
                        buffer_empty_since = None;
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Paused;
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Paused));
                    }
                    StreamingCommand::Stop => {
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Idle;
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Idle));
                        break;
                    }
                    StreamingCommand::SeekSeconds(seconds) => {
                        let frame_position = (seconds * spec.sample_rate as f64) as u64;
                        if let Err(e) = decoder.seek(frame_position) {
                            eprintln!("[AudioStreamingManager] Seek error: {:?}", e);
                        } else {
                            // Clear buffer after seek
                            audio_buffer.clear();
                            buffered_frames = 0;
                            pre_buffered = false;
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = if playing {
                                StreamingState::Playing
                            } else {
                                StreamingState::Paused
                            };
                        }
                    }
                }
            }
            packet_count += 1;

            // Pre-buffering phase: decode and accumulate, also write to keep CamillaDSP fed
            if !pre_buffered {
                match decoder.decode_next() {
                    Ok(Some(decoded_audio)) => {
                        // Feed samples to loudness monitor if enabled
                        if let Some(ref monitor) = loudness_monitor {
                            if let Ok(a) = monitor.lock() {
                                if let Err(e) = a.add_frames(&decoded_audio.samples) {
                                    eprintln!(
                                        "[AudioStreamingManager] Loudness monitoring error: {}",
                                        e
                                    );
                                }
                            }
                        }

                        // Feed samples to spectrum analyzer if enabled
                        if let Some(ref analyzer) = spectrum_monitor {
                            if let Ok(mut a) = analyzer.lock() {
                                if let Err(e) = a.add_frames(&decoded_audio.samples) {
                                    eprintln!(
                                        "[AudioStreamingManager] Spectrum monitoring error: {}",
                                        e
                                    );
                                }
                            }
                        }

                        // Process through plugin host if enabled
                        let processed_samples =
                            process_through_plugins(&decoded_audio.samples, &plugin_host);

                        // Convert to bytes (F32LE format)
                        let pcm_bytes: Vec<u8> = processed_samples
                            .iter()
                            .flat_map(|&sample| sample.to_le_bytes())
                            .collect();

                        let frames_in_packet = pcm_bytes.len() / (output_channels * 4);

                        // Only accumulate in buffer during pre-buffering, don't write yet
                        audio_buffer.extend(pcm_bytes.iter());
                        buffered_frames += frames_in_packet;

                        // Check if we've reached the target buffer size
                        if buffered_frames >= target_buffer_frames {
                            log_info!(
                                "Pre-buffer complete: {} frames buffered ({:.2}s at {}Hz)",
                                buffered_frames,
                                buffered_frames as f64 / spec.sample_rate as f64,
                                spec.sample_rate
                            );

                            // Write HALF the pre-buffer to CamillaDSP, keep the rest in buffer
                            // This ensures we have data to continue streaming immediately
                            let write_size = audio_buffer.len() / 2;
                            let prebuffer_data: Vec<u8> =
                                audio_buffer.drain(..write_size).collect();
                            let frames_to_write = write_size / (output_channels * 4);

                            log_debug!(
                                "Writing half of pre-buffer ({} frames, {:.2}s) to CamillaDSP, keeping {} frames in buffer",
                                frames_to_write,
                                frames_to_write as f64 / spec.sample_rate as f64,
                                buffered_frames - frames_to_write
                            );

                            if let Err(e) = stdin.write_all(&prebuffer_data) {
                                log_error!("Failed to write pre-buffer: {:?}", e);
                                let mut state_lock = state.lock().unwrap();
                                *state_lock = StreamingState::Error;
                                let _ = event_tx.send(StreamingEvent::Error(format!(
                                    "Failed to write pre-buffer: {:?}",
                                    e
                                )));
                                let _ = event_tx
                                    .send(StreamingEvent::StateChanged(StreamingState::Error));
                                break;
                            }
                            if let Err(e) = stdin.flush() {
                                log_error!("Failed to flush pre-buffer: {:?}", e);
                                let mut state_lock = state.lock().unwrap();
                                *state_lock = StreamingState::Error;
                                let _ = event_tx.send(StreamingEvent::Error(format!(
                                    "Failed to flush pre-buffer: {:?}",
                                    e
                                )));
                                let _ = event_tx
                                    .send(StreamingEvent::StateChanged(StreamingState::Error));
                                break;
                            }

                            // Update buffer state
                            buffered_frames -= frames_to_write;
                            log_info!(
                                "Pre-buffer written, playback starting ({} frames remaining in buffer)",
                                buffered_frames
                            );

                            pre_buffered = true;

                            if playing {
                                let mut state_lock = state.lock().unwrap();
                                *state_lock = StreamingState::Playing;
                                let _ = event_tx
                                    .send(StreamingEvent::StateChanged(StreamingState::Playing));
                            }
                        }
                    }
                    Ok(None) => {
                        // End of stream during pre-buffering
                        log_info!("End of stream during pre-buffering");
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Idle;
                        let _ = event_tx.send(StreamingEvent::EndOfStream);
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Idle));
                        break;
                    }
                    Err(e) => {
                        log_error!("Decode error during pre-buffering: {:?}", e);
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Error;
                        let _ = event_tx.send(StreamingEvent::Error(format!("{:?}", e)));
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Error));
                        break;
                    }
                }
                continue;
            }

            // Detect underrun: if buffer dropped too low, count it
            if pre_buffered && buffered_frames < target_buffer_frames / 10 {
                let mut underruns = underrun_count.lock().unwrap();
                *underruns += 1;
                log_warn!(
                    "Buffer critically low ({} frames, target {})",
                    buffered_frames,
                    target_buffer_frames
                );
            }

            // Adaptive buffering: check for underruns periodically and increase buffer size
            if last_underrun_check.elapsed() > Duration::from_secs(5) && pre_buffered {
                let underruns = *underrun_count.lock().unwrap();
                if underruns > 0 {
                    // Increase buffer by 50% (up to max of 1024 chunks)
                    let new_chunks = ((buffer_chunks as f64 * 2.0) as usize).min(FRAMES_PER_CHUNK);
                    if new_chunks > buffer_chunks {
                        buffer_chunks = new_chunks;
                        target_buffer_frames = buffer_chunks * FRAMES_PER_CHUNK;
                        println!(
                            "[AudioStreamingManager] Detected {} underrun(s), increasing buffer to {} chunks ({} frames)",
                            underruns, buffer_chunks, target_buffer_frames
                        );
                        // Reset underrun counter
                        *underrun_count.lock().unwrap() = 0;
                    }
                }
                last_underrun_check = std::time::Instant::now();
            }

            // Playback phase: write buffered data and decode ahead
            if playing && pre_buffered {
                // Check if buffer has been empty for too long (stalled stream)
                if buffered_frames == 0 {
                    if buffer_empty_since.is_none() {
                        // Buffer just became empty, start timer
                        buffer_empty_since = Some(std::time::Instant::now());
                        log_warn!("Buffer is empty, starting timeout timer");
                    } else if let Some(empty_start) = buffer_empty_since {
                        // Check if buffer has been empty for too long
                        if empty_start.elapsed() > BUFFER_EMPTY_TIMEOUT {
                            log_error!(
                                "Buffer has been empty for more than {:?}, stopping streaming",
                                BUFFER_EMPTY_TIMEOUT
                            );
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamingState::Error;
                            let _ = event_tx.send(StreamingEvent::Error(
                                "Streaming stalled: buffer empty for too long".to_string(),
                            ));
                            let _ =
                                event_tx.send(StreamingEvent::StateChanged(StreamingState::Error));
                            break;
                        }
                    }
                } else {
                    // Buffer has data, reset empty timer
                    if buffer_empty_since.is_some() {
                        log_info!("Buffer recovered, has {} frames", buffered_frames);
                    }
                    buffer_empty_since = None;
                }

                // Write from buffer to stdin if we have data
                if !audio_buffer.is_empty() {
                    // Write in chunks for better performance
                    let write_size = std::cmp::min(audio_buffer.len(), FRAMES_PER_CHUNK * 4); // Write up to 4 CamillaDSP chunks at once
                    let chunk: Vec<u8> = audio_buffer.drain(..write_size).collect();

                    if let Err(e) = stdin.write_all(&chunk) {
                        eprintln!(
                            "[AudioStreamingManager] Failed to write to CamillaDSP stdin: {:?}",
                            e
                        );
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Error;
                        let _ = event_tx.send(StreamingEvent::Error(format!(
                            "Failed to write to CamillaDSP: {:?}",
                            e
                        )));
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Error));
                        break;
                    }

                    // Flush to ensure immediate delivery
                    if let Err(e) = stdin.flush() {
                        eprintln!("[AudioStreamingManager] Failed to flush stdin: {:?}", e);
                        let mut state_lock = state.lock().unwrap();
                        *state_lock = StreamingState::Error;
                        let _ = event_tx.send(StreamingEvent::Error(format!(
                            "Failed to flush stdin: {:?}",
                            e
                        )));
                        let _ = event_tx.send(StreamingEvent::StateChanged(StreamingState::Error));
                        break;
                    }

                    let frames_written = write_size / (output_channels * 4);
                    buffered_frames = buffered_frames.saturating_sub(frames_written);
                }

                // Decode ahead to maintain buffer (keep it at least 75% full for smooth playback)
                // For AAC, batch decode multiple packets at once for better performance
                while buffered_frames < (target_buffer_frames * 3) / 4 {
                    match decoder.decode_next() {
                        Ok(Some(decoded_audio)) => {
                            // Feed samples to loudness monitor if enabled
                            if let Some(ref monitor) = loudness_monitor {
                                if let Ok(lm) = monitor.lock() {
                                    if let Err(e) = lm.add_frames(&decoded_audio.samples) {
                                        eprintln!(
                                            "[AudioStreamingManager] Loudness monitoring error: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            // Feed samples to spectrum analyzer if enabled
                            if let Some(ref analyzer) = spectrum_monitor {
                                if let Ok(mut a) = analyzer.lock() {
                                    if let Err(e) = a.add_frames(&decoded_audio.samples) {
                                        eprintln!(
                                            "[AudioStreamingManager] Spectrum monitoring error: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            // Process through plugin host if enabled
                            let processed_samples =
                                process_through_plugins(&decoded_audio.samples, &plugin_host);

                            // Convert to bytes (F32LE format)
                            let pcm_bytes: Vec<u8> = processed_samples
                                .iter()
                                .flat_map(|&sample| sample.to_le_bytes())
                                .collect();

                            let frames_in_packet = pcm_bytes.len() / (output_channels * 4);

                            audio_buffer.extend(pcm_bytes.iter());
                            buffered_frames += frames_in_packet;
                        }
                        Ok(None) => {
                            // End of stream - drain remaining buffer
                            log_info!(
                                "End of stream reached, buffer has {} frames remaining",
                                buffered_frames
                            );
                            if audio_buffer.is_empty() {
                                log_info!("Buffer drained, playback completed");
                                let mut state_lock = state.lock().unwrap();
                                *state_lock = StreamingState::Idle;
                                let _ = event_tx.send(StreamingEvent::EndOfStream);
                                let _ = event_tx
                                    .send(StreamingEvent::StateChanged(StreamingState::Idle));
                                playing = false;
                            }
                            break;
                        }
                        Err(e) => {
                            eprintln!("[AudioStreamingManager] Decode error: {:?}", e);
                            let mut state_lock = state.lock().unwrap();
                            *state_lock = StreamingState::Error;
                            let _ = event_tx.send(StreamingEvent::Error(format!("{:?}", e)));
                            let _ =
                                event_tx.send(StreamingEvent::StateChanged(StreamingState::Error));
                            break;
                        }
                    }
                }
            } else if !playing {
                // Sleep when paused
                thread::sleep(Duration::from_millis(50));
            }
        }

        println!("[AudioStreamingManager] Decoder thread exiting");
        Ok(())
    }

    /// Update EQ filters and loudness in real-time without restarting playback
    /// Works for both streaming and file-based playback modes
    pub async fn update_filters(
        &self,
        filters: Vec<FilterParams>,
        loudness: Option<LoudnessCompensation>,
    ) -> AudioDecoderResult<()> {
        self.audio_manager
            .update_filters(filters, loudness)
            .await
            .map_err(|e| AudioDecoderError::ConfigError(format!("Failed to update filters: {}", e)))
    }
}

impl Drop for AudioStreamingManager {
    fn drop(&mut self) {
        // Ensure cleanup
        let _ = futures::executor::block_on(self.stop());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_streaming_manager_creation() {
        let camilla_path = PathBuf::from("/usr/local/bin/camilladsp");
        let manager = AudioStreamingManager::new(camilla_path);
        assert_eq!(manager.get_state(), StreamingState::Idle);
        assert!(manager.get_audio_info().is_none());
    }

    #[test]
    fn test_audio_file_info() {
        let spec = AudioSpec {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            total_frames: Some(441000), // 10 seconds
        };

        let info = AudioFileInfo {
            path: PathBuf::from("test.flac"),
            format: AudioFormat::Flac,
            spec,
            duration_seconds: Some(10.0),
        };

        assert_eq!(info.format, AudioFormat::Flac);
        assert_eq!(info.spec.sample_rate, 44100);
        assert_eq!(info.duration_seconds, Some(10.0));
    }
}
