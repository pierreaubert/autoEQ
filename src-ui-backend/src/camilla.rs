use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::fs;
use std::io::Write;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CamillaError {
    ProcessNotRunning,
    ProcessStartFailed(String),
    ProcessCommunicationFailed(String),
    ConfigGenerationFailed(String),
    BinaryNotFound(String),
    WebSocketError(String),
    InvalidConfiguration(String),
    IOError(String),
    Timeout(String),
}

impl std::fmt::Display for CamillaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CamillaError::ProcessNotRunning => write!(f, "CamillaDSP process is not running"),
            CamillaError::ProcessStartFailed(msg) => {
                write!(f, "Failed to start CamillaDSP process: {}", msg)
            }
            CamillaError::ProcessCommunicationFailed(msg) => {
                write!(f, "Failed to communicate with CamillaDSP: {}", msg)
            }
            CamillaError::ConfigGenerationFailed(msg) => {
                write!(f, "Failed to generate CamillaDSP config: {}", msg)
            }
            CamillaError::BinaryNotFound(msg) => {
                write!(f, "CamillaDSP binary not found: {}", msg)
            }
            CamillaError::WebSocketError(msg) => write!(f, "WebSocket error: {}", msg),
            CamillaError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            CamillaError::IOError(msg) => write!(f, "IO error: {}", msg),
            CamillaError::Timeout(msg) => write!(f, "Operation timed out: {}", msg),
        }
    }
}

impl std::error::Error for CamillaError {}

impl From<std::io::Error> for CamillaError {
    fn from(err: std::io::Error) -> Self {
        CamillaError::IOError(err.to_string())
    }
}

impl From<serde_yaml::Error> for CamillaError {
    fn from(err: serde_yaml::Error) -> Self {
        CamillaError::ConfigGenerationFailed(err.to_string())
    }
}

impl From<serde_json::Error> for CamillaError {
    fn from(err: serde_json::Error) -> Self {
        CamillaError::WebSocketError(format!("JSON error: {}", err))
    }
}

pub type CamillaResult<T> = Result<T, CamillaError>;

// ============================================================================
// Filter Parameters
// ============================================================================

/// Loudness compensation settings (CamillaDSP Loudness filter)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoudnessCompensation {
    pub reference_level: f64, // -100 .. +20
    pub low_boost: f64,       // 0 .. 20
    pub high_boost: f64,      // 0 .. 20
    #[serde(default)]
    pub attenuate_mid: bool,
}

impl LoudnessCompensation {
    pub fn new(reference_level: f64, low_boost: f64, high_boost: f64) -> CamillaResult<Self> {
        let lc = Self {
            reference_level,
            low_boost,
            high_boost,
            attenuate_mid: false,
        };
        lc.validate()?;
        Ok(lc)
    }

    pub fn validate(&self) -> CamillaResult<()> {
        if !(self.reference_level >= -100.0 && self.reference_level <= 20.0) {
            return Err(CamillaError::InvalidConfiguration(format!(
                "reference_level out of range (-100..20): {}",
                self.reference_level
            )));
        }
        if !(self.low_boost >= 0.0 && self.low_boost <= 20.0) {
            return Err(CamillaError::InvalidConfiguration(format!(
                "low_boost out of range (0..20): {}",
                self.low_boost
            )));
        }
        if !(self.high_boost >= 0.0 && self.high_boost <= 20.0) {
            return Err(CamillaError::InvalidConfiguration(format!(
                "high_boost out of range (0..20): {}",
                self.high_boost
            )));
        }
        Ok(())
    }
}

/// Parametric EQ filter parameters (Biquad)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilterParams {
    /// Center frequency in Hz
    pub frequency: f64,
    /// Q factor (bandwidth)
    pub q: f64,
    /// Gain in dB
    pub gain: f64,
    /// Filter type (default: "Peaking")
    #[serde(default = "default_filter_type")]
    pub filter_type: String,
}

fn default_filter_type() -> String {
    "Peaking".to_string()
}

impl FilterParams {
    pub fn new(frequency: f64, q: f64, gain: f64) -> Self {
        Self {
            frequency,
            q,
            gain,
            filter_type: default_filter_type(),
        }
    }

    pub fn validate(&self) -> CamillaResult<()> {
        if self.frequency < 20.0 || self.frequency > 20000.0 {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Frequency must be between 20 and 20000 Hz, got {}",
                self.frequency
            )));
        }
        if self.q <= 0.0 || self.q > 100.0 {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Q must be between 0 and 100, got {}",
                self.q
            )));
        }
        if self.gain.abs() > 30.0 {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Gain must be between -30 and +30 dB, got {}",
                self.gain
            )));
        }
        Ok(())
    }
}

// ============================================================================
// Audio State
// ============================================================================

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
    pub capture: Option<CaptureDevice>,
    pub playback: PlaybackDevice,
    /// Enable automatic sample rate adjustment (allows CamillaDSP to adapt to device rate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_rate_adjust: Option<bool>,
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

// ============================================================================
// Channel mapping mode
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChannelMapMode {
    Normal,
    Swap,
}

// ============================================================================
// CamillaDSP Process Management
// ============================================================================

/// CamillaDSP subprocess manager
pub struct CamillaDSPProcess {
    /// Child process handle
    process: Option<Child>,
    /// Stdin handle for streaming audio data
    stdin: Option<ChildStdin>,
    /// Path to the CamillaDSP binary
    binary_path: PathBuf,
    /// Path to the config file
    config_path: Option<PathBuf>,
    /// WebSocket port
    websocket_port: u16,
    /// Process health check interval
    health_check_interval: Duration,
}

impl CamillaDSPProcess {
    /// Create a new CamillaDSP process manager
    pub fn new(binary_path: PathBuf) -> Self {
        // Find an available port dynamically to avoid conflicts
        let websocket_port = find_available_port().unwrap_or(1234);
        Self {
            process: None,
            stdin: None,
            binary_path,
            config_path: None,
            websocket_port,
            health_check_interval: Duration::from_secs(5),
        }
    }

    /// Set the WebSocket port
    pub fn with_port(mut self, port: u16) -> Self {
        self.websocket_port = port;
        self
    }

    /// Set health check interval
    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    /// Check if the process is currently running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited
                    self.process = None;
                    false
                }
                Ok(None) => {
                    // Process is still running
                    true
                }
                Err(_) => {
                    // Error checking status, assume not running
                    self.process = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the WebSocket URL for this instance
    pub fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.websocket_port)
    }

    /// Get the config path
    pub fn config_path(&self) -> Option<&PathBuf> {
        self.config_path.as_ref()
    }

    /// Start the CamillaDSP process with the given config file
    pub fn start(&mut self, config_path: PathBuf) -> CamillaResult<()> {
        // Check if already running
        if self.is_running() {
            return Err(CamillaError::ProcessStartFailed(
                "CamillaDSP process is already running".to_string(),
            ));
        }

        // Verify binary exists
        if !self.binary_path.exists() {
            return Err(CamillaError::BinaryNotFound(format!(
                "Binary not found at path: {:?}",
                self.binary_path
            )));
        }

        // Verify config file exists
        if !config_path.exists() {
            return Err(CamillaError::ConfigGenerationFailed(format!(
                "Config file not found at path: {:?}",
                config_path
            )));
        }

        println!(
            "[CamillaDSP] Starting subprocess with config: {:?}",
            config_path
        );
        println!("[CamillaDSP] Binary path: {:?}", self.binary_path);
        println!("[CamillaDSP] WebSocket port: {}", self.websocket_port);

        // Build command
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-p")
            .arg(self.websocket_port.to_string())
            .arg("-v") // Verbose mode to see errors
            .arg(config_path.to_str().ok_or_else(|| {
                CamillaError::ConfigGenerationFailed("Invalid config path encoding".to_string())
            })?)
            .stdin(Stdio::piped()) // Piped stdin for streaming audio
            .stdout(Stdio::inherit()) // Show output directly
            .stderr(Stdio::inherit());

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            CamillaError::ProcessStartFailed(format!("Failed to spawn CamillaDSP process: {}", e))
        })?;

        // Take ownership of stdin handle
        let stdin = child.stdin.take();

        self.process = Some(child);
        self.stdin = stdin;
        self.config_path = Some(config_path);

        // Give the process a moment to start
        thread::sleep(Duration::from_millis(500));

        // Verify it's running
        if !self.is_running() {
            return Err(CamillaError::ProcessStartFailed(
                "Process exited immediately after start (check console output above)".to_string(),
            ));
        }

        println!("[CamillaDSP] Process started successfully");
        Ok(())
    }

    /// Get a mutable reference to the stdin handle for writing audio data
    pub fn stdin_mut(&mut self) -> Option<&mut ChildStdin> {
        self.stdin.as_mut()
    }

    /// Stop the CamillaDSP process gracefully
    pub fn stop(&mut self) -> CamillaResult<()> {
        // Drop stdin to signal end of stream
        self.stdin = None;

        if let Some(mut child) = self.process.take() {
            println!("[CamillaDSP] Stopping subprocess...");

            // Try graceful termination first
            #[cfg(unix)]
            {
                // Send SIGTERM on Unix
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }

            #[cfg(windows)]
            {
                // On Windows, kill is the only option
                let _ = child.kill();
            }

            // Wait for graceful shutdown with timeout
            let timeout = Duration::from_secs(5);
            let start = Instant::now();

            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        println!("[CamillaDSP] Process exited with status: {:?}", status);
                        self.config_path = None;
                        return Ok(());
                    }
                    Ok(None) => {
                        // Still running
                        if start.elapsed() > timeout {
                            println!("[CamillaDSP] Graceful shutdown timed out, forcing kill...");
                            child.kill().map_err(|e| {
                                CamillaError::ProcessCommunicationFailed(format!(
                                    "Failed to kill process: {}",
                                    e
                                ))
                            })?;
                            let _ = child.wait();
                            self.config_path = None;
                            return Ok(());
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        return Err(CamillaError::ProcessCommunicationFailed(format!(
                            "Error waiting for process: {}",
                            e
                        )));
                    }
                }
            }
        } else {
            println!("[CamillaDSP] No process to stop");
            Ok(())
        }
    }

    /// Restart the CamillaDSP process
    pub fn restart(&mut self) -> CamillaResult<()> {
        println!("[CamillaDSP] Restarting subprocess...");

        // Store the config path before stopping
        let config_path = self
            .config_path
            .clone()
            .ok_or(CamillaError::ProcessNotRunning)?;

        // Stop the current process
        self.stop()?;

        // Wait a moment before restarting
        thread::sleep(Duration::from_millis(500));

        // Start with the same config
        self.start(config_path)
    }

    /// Get process ID if running
    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().map(|p| p.id())
    }
}

fn find_available_port() -> Option<u16> {
    // Try up to 200 attempts to find a free port above 1024
    let start = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| (d.as_nanos() % 50000) as u16)
        .unwrap_or(0))
        + 1025; // ensure >1024

    for i in 0..200u16 {
        let port = 1025 + ((start + i) % (65535 - 1025));
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            // Successfully bound; release immediately and use this port
            drop(listener);
            return Some(port);
        }
    }
    None
}

impl Drop for CamillaDSPProcess {
    fn drop(&mut self) {
        // Ensure cleanup on drop
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

// ============================================================================
// WebSocket Communication
// ============================================================================

/// WebSocket command to send to CamillaDSP
#[derive(Debug, Clone)]
pub enum CamillaCommand {
    GetConfig,
    SetConfig { config: String },
    GetState,
    Stop,
    GetCaptureSignalPeak,
    GetPlaybackSignalPeak,
    GetBufferLevel,
}

// We parse responses dynamically since CamillaDSP uses externally tagged
// commands like {"GetState": {"result": "Ok", "value": "Running"}}

/// WebSocket client for CamillaDSP control
pub struct CamillaWebSocketClient {
    url: String,
    timeout: Duration,
}

impl CamillaWebSocketClient {
    /// Create a new WebSocket client
    pub fn new(url: String) -> Self {
        Self {
            url,
            timeout: Duration::from_secs(5),
        }
    }

    /// Set the timeout for WebSocket operations
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Send a command and wait for response
    pub async fn send_command(&self, command: CamillaCommand) -> CamillaResult<String> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .map_err(|e| CamillaError::WebSocketError(format!("Connection failed: {}", e)))?;

        let (mut write, mut read) = ws_stream.split();

        // Build and send command
        match command {
            CamillaCommand::SetConfig { ref config } => {
                let command_json =
                    serde_json::json!({ "SetConfig": { "config": config } }).to_string();
                println!("[WebSocket] Sending command: {}", command_json);
                write
                    .send(Message::Text(command_json))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::GetConfig => {
                println!("[WebSocket] Sending command: GetConfig");
                let txt = serde_json::to_string(&"GetConfig").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::GetState => {
                println!("[WebSocket] Sending command: GetState");
                let txt = serde_json::to_string(&"GetState").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::Stop => {
                println!("[WebSocket] Sending command: Stop");
                let txt = serde_json::to_string(&"Stop").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::GetCaptureSignalPeak => {
                println!("[WebSocket] Sending command: GetCaptureSignalPeak");
                let txt = serde_json::to_string(&"GetCaptureSignalPeak").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::GetPlaybackSignalPeak => {
                println!("[WebSocket] Sending command: GetPlaybackSignalPeak");
                let txt = serde_json::to_string(&"GetPlaybackSignalPeak").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
            CamillaCommand::GetBufferLevel => {
                println!("[WebSocket] Sending command: GetBufferLevel");
                let txt = serde_json::to_string(&"GetBufferLevel").unwrap();
                write
                    .send(Message::Text(txt))
                    .await
                    .map_err(|e| CamillaError::WebSocketError(format!("Send failed: {}", e)))?;
            }
        }

        // Wait for response with timeout
        let response_future = read.next();
        let response_msg = tokio::time::timeout(self.timeout, response_future)
            .await
            .map_err(|_| CamillaError::Timeout("WebSocket response timeout".to_string()))?
            .ok_or_else(|| CamillaError::WebSocketError("Connection closed".to_string()))?
            .map_err(|e| CamillaError::WebSocketError(format!("Receive failed: {}", e)))?;

        match response_msg {
            Message::Text(text) => {
                println!("[WebSocket] Received response: {}", text);
                Ok(text)
            }
            _ => Err(CamillaError::WebSocketError(
                "Unexpected message type".to_string(),
            )),
        }
    }

    /// Get current state
    pub async fn get_state(&self) -> CamillaResult<String> {
        let text = self.send_command(CamillaCommand::GetState).await?;
        // Expected: {"GetState": {"result":"Ok","value":"Running"}}
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let state = v
            .get("GetState")
            .and_then(|x| x.get("value"))
            .and_then(|x| x.as_str())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;
        Ok(state.to_string())
    }

    /// Get current configuration
    pub async fn get_config(&self) -> CamillaResult<String> {
        let text = self.send_command(CamillaCommand::GetConfig).await?;
        // Expect {"GetConfig": {"result":"Ok","value":"<yaml>"}}
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let cfg = v
            .get("GetConfig")
            .and_then(|x| x.get("value"))
            .and_then(|x| x.as_str())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;
        Ok(cfg.to_string())
    }

    /// Set new configuration
    pub async fn set_config(&self, config_yaml: String) -> CamillaResult<()> {
        let text = self
            .send_command(CamillaCommand::SetConfig {
                config: config_yaml,
            })
            .await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let ok = v
            .get("SetConfig")
            .and_then(|x| x.get("result"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            == "Ok";
        if ok {
            Ok(())
        } else {
            Err(CamillaError::ProcessCommunicationFailed(
                "SetConfig failed".to_string(),
            ))
        }
    }

    /// Stop playback
    pub async fn stop(&self) -> CamillaResult<()> {
        let text = self.send_command(CamillaCommand::Stop).await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let ok = v
            .get("Stop")
            .and_then(|x| x.get("result"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            == "Ok";
        if ok {
            Ok(())
        } else {
            Err(CamillaError::ProcessCommunicationFailed(
                "Stop failed".to_string(),
            ))
        }
    }

    /// Get capture signal peak (volume level)
    pub async fn get_capture_signal_peak(&self) -> CamillaResult<f32> {
        let text = self
            .send_command(CamillaCommand::GetCaptureSignalPeak)
            .await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let value = v
            .get("GetCaptureSignalPeak")
            .and_then(|x| x.get("value"))
            .and_then(|x| x.as_f64())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;
        Ok(value as f32)
    }

    /// Get playback signal peak (volume level)
    pub async fn get_playback_signal_peak(&self) -> CamillaResult<f32> {
        let text = self
            .send_command(CamillaCommand::GetPlaybackSignalPeak)
            .await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let value = v
            .get("GetPlaybackSignalPeak")
            .and_then(|x| x.get("value"))
            .and_then(|x| x.as_f64())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;
        Ok(value as f32)
    }

    /// Get buffer level
    pub async fn get_buffer_level(&self) -> CamillaResult<i32> {
        let text = self.send_command(CamillaCommand::GetBufferLevel).await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let value = v
            .get("GetBufferLevel")
            .and_then(|x| x.get("value"))
            .and_then(|x| x.as_i64())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;
        Ok(value as i32)
    }

    /// Test connection to WebSocket server
    pub async fn test_connection(&self) -> CamillaResult<bool> {
        match self.get_state().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Retry connection with exponential backoff
    pub async fn connect_with_retry(
        &self,
        max_retries: u32,
        initial_delay: Duration,
    ) -> CamillaResult<()> {
        let mut delay = initial_delay;

        for attempt in 0..max_retries {
            println!(
                "[WebSocket] Connection attempt {} of {}",
                attempt + 1,
                max_retries
            );

            match self.test_connection().await {
                Ok(true) => {
                    println!("[WebSocket] Connected successfully");
                    return Ok(());
                }
                Ok(false) | Err(_) => {
                    if attempt < max_retries - 1 {
                        println!("[WebSocket] Connection failed, retrying in {:?}", delay);
                        tokio::time::sleep(delay).await;
                        // Exponential backoff with max 10 seconds
                        delay = std::cmp::min(delay * 2, Duration::from_secs(10));
                    }
                }
            }
        }

        Err(CamillaError::WebSocketError(format!(
            "Failed to connect after {} attempts",
            max_retries
        )))
    }
}

// ============================================================================
// Audio Manager - High-Level API
// ============================================================================

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
        audio_spec: crate::audio_decoder::decoder::AudioSpec,
        output_device: Option<String>,
        filters: Vec<FilterParams>,
        channel_map_mode: ChannelMapMode,
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
            .connect_with_retry(3, Duration::from_millis(300))
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
        channel_map_mode: ChannelMapMode,
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
            &filters,
            channel_map_mode,
            output_map.as_deref(),
            loudness.as_ref(),
        )?;

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
            .connect_with_retry(3, Duration::from_millis(300))
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

    /// Update EQ filters in real-time
    pub async fn update_filters(&self, filters: Vec<FilterParams>) -> CamillaResult<()> {
        println!("[AudioManager] Updating {} filters", filters.len());

        // Validate filters
        for filter in &filters {
            filter.validate()?;
        }

        // Get current state to rebuild config
        let (
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

            let file = state
                .current_file
                .clone()
                .ok_or(CamillaError::ProcessNotRunning)?;

            (
                file,
                state.output_device.clone(),
                state.sample_rate,
                state.channels,
                state.channel_map_mode,
                state.playback_channel_map.clone(),
            )
        };

        // Generate new config with updated filters
        let config = generate_playback_config(
            &audio_file,
            output_device.as_deref(),
            sample_rate,
            channels,
            &filters,
            channel_map_mode,
            playback_channel_map.as_deref(),
            None,
        )?;

        let config_yaml = serde_yaml::to_string(&config)?;

        // Send config update via WebSocket
        let ws_url = {
            let process = self.process.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock process: {}", e))
            })?;
            process.websocket_url()
        };

        let client = CamillaWebSocketClient::new(ws_url);
        client.set_config(config_yaml).await?;

        // Update state with new filters
        {
            let mut state = self.state.lock().map_err(|e| {
                CamillaError::ProcessCommunicationFailed(format!("Failed to lock state: {}", e))
            })?;
            state.filters = filters;
        }

        println!("[AudioManager] Filters updated successfully");
        Ok(())
    }

    /// Start recording from input device
    pub async fn start_recording(
        &self,
        output_file: PathBuf,
        input_device: Option<String>,
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
        }

        // Generate recording config
        let config = generate_recording_config(
            &output_file,
            input_device.as_deref(),
            sample_rate,
            channels,
            input_map.as_deref(),
        )?;

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

        // If the output file already exists (as raw data), convert it
        if output_file.exists() {
            println!("[AudioManager] Converting raw audio to WAV format...");
            convert_raw_to_wav(&output_file, &output_file, sample_rate, channels)?;
            println!("[AudioManager] Recording saved as WAV: {:?}", output_file);
        } else if raw_file.exists() {
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

// ============================================================================
// Config Generation
// ============================================================================

/// Generate a CamillaDSP config for streaming playback with stdin input
pub fn generate_streaming_config(
    output_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    filters: &[FilterParams],
    map_mode: ChannelMapMode,
    output_map: Option<&[u16]>,
    loudness: Option<&LoudnessCompensation>,
) -> CamillaResult<CamillaDSPConfig> {
    // Validate all filters
    for filter in filters {
        filter.validate()?;
    }

    // Detect device native sample rate for proper resampling
    let device_sample_rate = get_device_native_sample_rate(output_device).unwrap_or(sample_rate); // Fallback to file rate if detection fails

    let needs_resampling = sample_rate != device_sample_rate;

    if needs_resampling {
        println!(
            "[CamillaDSP] Sample rate mismatch detected: file={}Hz, device={}Hz. Adding resampler.",
            sample_rate, device_sample_rate
        );
    }

    // Create capture device (stdin input)
    let capture = CaptureDevice {
        device_type: "Stdin".to_string(),
        device: None,
        filename: None,
        channels: Some(ChannelsSetting::Count(channels)),
        format: Some("FLOAT32LE".to_string()), // Our decoder outputs f32 samples
    };

    // Create playback device
    let (playback_type, device_name) = map_output_device(output_device)?;
    // Prepare output channel_map if provided
    let effective_output_map: Option<Vec<u16>> = if let Some(map) = output_map {
        if map.len() as u16 >= channels {
            // Use the last `channels` entries to select L/R, as often used for dedicated output pairs
            let start = map.len() - channels as usize;
            Some(map[start..].to_vec())
        } else {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Output channel_map length ({}) must be >= channels ({})",
                map.len(),
                channels
            )));
        }
    } else {
        None
    };

    // Determine total number of output channels required
    let mixer_out_channels: u16 = if let Some(ref outs) = effective_output_map {
        outs.iter().copied().max().unwrap_or(1) + 1
    } else {
        channels
    };

    let playback = PlaybackDevice {
        device_type: playback_type,
        device: device_name,
        filename: None,
        channels: Some(ChannelsSetting::Count(mixer_out_channels)),
        format: None, // Let CoreAudio use default format
    };

    // Generate resampler config if needed
    let resampler_config = if needs_resampling {
        // For streaming from Stdin, Camilla doesn't know the source rate.
        // Use Synchronous resampler and provide explicit in/out rates.
        let resampler_yaml = format!(
            r#"
            type: Synchronous
            in_rate: {in_rate}
            out_rate: {out_rate}
            "#,
            in_rate = sample_rate,
            out_rate = device_sample_rate
        );
        Some(
            serde_yaml::from_str::<serde_yaml::Value>(&resampler_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!("Failed to generate resampler: {}", e))
            })?,
        )
    } else {
        None
    };

    let devices = DeviceConfig {
        samplerate: device_sample_rate, // Use device's native rate (output side)
        chunksize: 1024,
        capture: Some(capture),
        playback,
        // With a synchronous resampler, rate adjust is not applicable.
        enable_rate_adjust: Some(false),
        resampler: resampler_config,
    };

    // Generate filters section, optionally including Loudness filter
    let filters_section = {
        let mut fm = serde_yaml::Mapping::new();

        if let Some(lc) = loudness {
            let loud_yaml = format!(
                r#"
                loudness:
                  type: Loudness
                  parameters:
                    fader: Main
                    reference_level: {ref_level}
                    high_boost: {high}
                    low_boost: {low}
                    attenuate_mid: {atten}
                "#,
                ref_level = lc.reference_level,
                high = lc.high_boost,
                low = lc.low_boost,
                atten = if lc.attenuate_mid { "true" } else { "false" }
            );
            let v: serde_yaml::Value = serde_yaml::from_str(&loud_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!(
                    "Failed to generate loudness filter: {}",
                    e
                ))
            })?;
            if let serde_yaml::Value::Mapping(m) = v {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if !filters.is_empty() {
            let peq_val = generate_filters_yaml(filters)?;
            if let serde_yaml::Value::Mapping(m) = peq_val {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if fm.is_empty() {
            None
        } else {
            Some(serde_yaml::Value::Mapping(fm))
        }
    };

    // Build destination map per input channel
    let dest_map: Vec<u16> = {
        let mut base: Vec<u16> = (0..channels).collect();
        if let Some(ref outs) = effective_output_map {
            if outs.len() as u16 >= channels {
                let start = outs.len() - channels as usize;
                base = outs[start..].to_vec();
            }
        }
        // Optional swap for first two channels
        if map_mode == ChannelMapMode::Swap && channels >= 2 {
            let mut swapped = base.clone();
            swapped.swap(0, 1);
            swapped
        } else {
            base
        }
    };

    // Generate mixers section (matrix routing)
    let mixers_section = Some(generate_matrix_mixer_yaml(
        channels,
        mixer_out_channels,
        &dest_map,
    ));

    // Generate pipeline - always include mixer; add filters and resampler if needed
    let pipeline = Some(generate_pipeline(
        mixer_out_channels,
        filters,
        needs_resampling,
        loudness.is_some(),
    ));

    Ok(CamillaDSPConfig {
        devices,
        filters: filters_section,
        mixers: mixers_section,
        pipeline,
    })
}

/// Generate a CamillaDSP config for file playback with EQ filters
pub fn generate_playback_config(
    audio_file: &PathBuf,
    output_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    filters: &[FilterParams],
    map_mode: ChannelMapMode,
    output_map: Option<&[u16]>,
    loudness: Option<&LoudnessCompensation>,
) -> CamillaResult<CamillaDSPConfig> {
    // Validate all filters
    for filter in filters {
        filter.validate()?;
    }

    // Create capture device (file input)
    // Prefer absolute path if the file exists; otherwise, use the provided path without failing
    let filename_str = if audio_file.exists() {
        audio_file
            .canonicalize()
            .map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!(
                    "Failed to resolve audio file path {:?}: {}",
                    audio_file, e
                ))
            })?
            .to_str()
            .ok_or_else(|| {
                CamillaError::ConfigGenerationFailed("Invalid audio file path encoding".to_string())
            })?
            .to_string()
    } else {
        audio_file.to_string_lossy().to_string()
    };

    let capture = CaptureDevice {
        device_type: "File".to_string(),
        device: None,
        filename: Some(filename_str),
        channels: None, // File input infers channels from file
        format: None,   // File input infers format from file
    };

    // Create playback device
    let (playback_type, device_name) = map_output_device(output_device)?;
    // Prepare output channel_map if provided
    let effective_output_map: Option<Vec<u16>> = if let Some(map) = output_map {
        if map.len() as u16 >= channels {
            // Use the last `channels` entries to select L/R, as often used for dedicated output pairs
            let start = map.len() - channels as usize;
            Some(map[start..].to_vec())
        } else {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Output channel_map length ({}) must be >= channels ({})",
                map.len(),
                channels
            )));
        }
    } else {
        None
    };

    // Determine total number of output channels required
    let mixer_out_channels: u16 = if let Some(ref outs) = effective_output_map {
        outs.iter().copied().max().unwrap_or(1) + 1
    } else {
        channels
    };

    let playback = PlaybackDevice {
        device_type: playback_type,
        device: device_name,
        filename: None,
        channels: Some(ChannelsSetting::Count(mixer_out_channels)),
        format: None, // Let CoreAudio use default format
    };

    // Detect device native sample rate for file playback too
    let device_sample_rate = get_device_native_sample_rate(output_device).unwrap_or(sample_rate);

    let needs_resampling = sample_rate != device_sample_rate;

    if needs_resampling {
        println!(
            "[CamillaDSP] Sample rate mismatch detected: file={}Hz, device={}Hz. Adding resampler.",
            sample_rate, device_sample_rate
        );
    }

    // Generate resampler config if needed
    let resampler_config = if needs_resampling {
        // CamillaDSP v3 expects explicit parameters per resampler type.
        // Use AsyncPoly with linear interpolation for broad compatibility.
        let resampler_yaml = r#"
            type: AsyncPoly
            interpolation: Linear
        "#;
        Some(
            serde_yaml::from_str::<serde_yaml::Value>(resampler_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!("Failed to generate resampler: {}", e))
            })?,
        )
    } else {
        None
    };

    let devices = DeviceConfig {
        // Use the requested playback sample rate in the config; add a resampler if needed
        samplerate: sample_rate,
        chunksize: 1024,
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
        resampler: resampler_config,
    };

    // Generate filters section, optionally including Loudness filter
    let filters_section = {
        let mut fm = serde_yaml::Mapping::new();

        if let Some(lc) = loudness {
            let loud_yaml = format!(
                r#"
                loudness:
                  type: Loudness
                  parameters:
                    fader: Main
                    reference_level: {ref_level}
                    high_boost: {high}
                    low_boost: {low}
                    attenuate_mid: {atten}
                "#,
                ref_level = lc.reference_level,
                high = lc.high_boost,
                low = lc.low_boost,
                atten = if lc.attenuate_mid { "true" } else { "false" }
            );
            let v: serde_yaml::Value = serde_yaml::from_str(&loud_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!(
                    "Failed to generate loudness filter: {}",
                    e
                ))
            })?;
            if let serde_yaml::Value::Mapping(m) = v {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if !filters.is_empty() {
            let peq_val = generate_filters_yaml(filters)?;
            if let serde_yaml::Value::Mapping(m) = peq_val {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if fm.is_empty() {
            None
        } else {
            Some(serde_yaml::Value::Mapping(fm))
        }
    };

    // Build destination map per input channel
    let dest_map: Vec<u16> = {
        let mut base: Vec<u16> = (0..channels).collect();
        if let Some(ref outs) = effective_output_map {
            if outs.len() as u16 >= channels {
                let start = outs.len() - channels as usize;
                base = outs[start..].to_vec();
            }
        }
        if map_mode == ChannelMapMode::Swap && channels >= 2 {
            let mut swapped = base.clone();
            swapped.swap(0, 1);
            swapped
        } else {
            base
        }
    };

    // Generate mixers section (matrix routing)
    let mixers_section = Some(generate_matrix_mixer_yaml(
        channels,
        mixer_out_channels,
        &dest_map,
    ));

    // Always include pipeline with at least the mixer
    let pipeline = Some(generate_pipeline(
        mixer_out_channels,
        filters,
        needs_resampling,
        loudness.is_some(),
    ));

    Ok(CamillaDSPConfig {
        devices,
        filters: filters_section,
        mixers: mixers_section,
        pipeline,
    })
}

/// Convert raw FLOAT32LE audio to WAV format using hound
fn convert_raw_to_wav(
    raw_path: &std::path::Path,
    wav_path: &std::path::Path,
    sample_rate: u32,
    channels: u16,
) -> CamillaResult<()> {
    use std::fs::File;
    use std::io::BufReader;

    println!(
        "[CamillaDSP] Converting raw audio to WAV: {:?} -> {:?}",
        raw_path, wav_path
    );

    // Read raw FLOAT32LE data
    let raw_file = File::open(raw_path)
        .map_err(|e| CamillaError::IOError(format!("Failed to open raw file: {}", e)))?;
    let mut reader = BufReader::new(raw_file);
    let mut raw_data = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut raw_data)
        .map_err(|e| CamillaError::IOError(format!("Failed to read raw data: {}", e)))?;

    // Interpret as f32 samples
    let sample_count = raw_data.len() / 4;
    let mut samples = Vec::with_capacity(sample_count);
    for chunk in raw_data.chunks_exact(4) {
        let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
        samples.push(f32::from_le_bytes(bytes));
    }

    // Write WAV file using hound
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut wav_writer = hound::WavWriter::create(wav_path, spec)
        .map_err(|e| CamillaError::IOError(format!("Failed to create WAV writer: {}", e)))?;

    // Write all samples
    for &sample in &samples {
        wav_writer
            .write_sample(sample)
            .map_err(|e| CamillaError::IOError(format!("Failed to write sample: {}", e)))?;
    }

    wav_writer
        .finalize()
        .map_err(|e| CamillaError::IOError(format!("Failed to finalize WAV file: {}", e)))?;

    println!(
        "[CamillaDSP] WAV conversion complete: {} samples, {}Hz, {}ch",
        sample_count, sample_rate, channels
    );
    Ok(())
}

/// Generate a CamillaDSP config for recording
pub fn generate_recording_config(
    output_file: &std::path::Path,
    input_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    input_map: Option<&[u16]>,
) -> CamillaResult<CamillaDSPConfig> {
    // Create capture device (audio input)
    let (capture_type, device_name) = map_input_device(input_device)?;
    // Prepare input channel_map if provided
    let effective_input_map: Option<Vec<u16>> = if let Some(map) = input_map {
        if map.len() as u16 >= channels {
            // Use the first `channels` entries for input channels
            Some(map[..channels as usize].to_vec())
        } else {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Input channel_map length ({}) must be >= channels ({})",
                map.len(),
                channels
            )));
        }
    } else {
        None
    };

    let capture = CaptureDevice {
        device_type: capture_type.clone(),
        device: device_name,
        filename: None,
        // CamillaDSP v3 CoreAudio expects channels as count (usize), not indices
        // When hardware channel mapping is provided, we capture enough channels to include
        // the highest requested channel index, then use mixer to route correctly
        channels: Some(ChannelsSetting::Count(match &effective_input_map {
            Some(map) if !map.is_empty() && capture_type == "CoreAudio" => {
                // For CoreAudio: Calculate max channel index + 1 to ensure we capture enough channels
                let max_channel = map.iter().max().copied().unwrap_or(0);
                max_channel + 1
            }
            _ => channels,
        })),
        format: None, // Let backend pick a compatible native format
    };

    // Create playback device (file output)
    let playback = PlaybackDevice {
        device_type: "File".to_string(),
        device: None,
        filename: Some(
            output_file
                .to_str()
                .ok_or_else(|| {
                    CamillaError::ConfigGenerationFailed(
                        "Invalid output file path encoding".to_string(),
                    )
                })?
                .to_string(),
        ),
        channels: Some(ChannelsSetting::Count(channels)), // Specify channels for file output
        format: Some("FLOAT32LE".to_string()),
    };

    let devices = DeviceConfig {
        samplerate: sample_rate,
        chunksize: 1024,
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
        resampler: None,
    };

    // If hardware channel mapping is provided and we're using CoreAudio,
    // we need to add a mixer to route the selected channels to the output
    let (mixers_section, pipeline) = if let Some(ref map) = effective_input_map
        && capture_type == "CoreAudio"
    {
        // Get the actual number of channels we're capturing
        let capture_channels = match devices.capture.as_ref().unwrap().channels {
            Some(ChannelsSetting::Count(n)) => n,
            _ => channels,
        };

        // Build the channel routing map:
        // For each captured input channel (0..capture_channels), specify which output channel it goes to.
        // The hardware channels specified in `map` go to sequential output channels (0..channels).
        // All other input channels are routed to a non-existent output (captured but discarded).
        let mut channel_routing: Vec<u16> = vec![channels; capture_channels as usize]; // Default: route to out-of-bounds (discard)
        for (output_idx, &hardware_channel) in map.iter().enumerate() {
            if (hardware_channel as usize) < channel_routing.len() && (output_idx as u16) < channels
            {
                channel_routing[hardware_channel as usize] = output_idx as u16;
            }
        }

        // Generate mixer that routes hardware channels to output channels
        let mixer = generate_matrix_mixer_yaml(capture_channels, channels, &channel_routing);

        // Generate pipeline with just the mixer
        let mut pipeline_steps: Vec<serde_yaml::Value> = Vec::new();
        let mut mixer_map = serde_yaml::Mapping::new();
        mixer_map.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Mixer".to_string()),
        );
        mixer_map.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("matrix_mixer".to_string()),
        );
        pipeline_steps.push(serde_yaml::Value::Mapping(mixer_map));

        (Some(mixer), Some(pipeline_steps))
    } else {
        (None, None)
    };

    Ok(CamillaDSPConfig {
        devices,
        filters: None,
        mixers: mixers_section,
        pipeline,
    })
}

/// Get the native sample rate of an output device
fn get_device_native_sample_rate(device_name: Option<&str>) -> Option<u32> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();

    let device = if let Some(name) = device_name {
        // Find device by name
        host.output_devices()
            .ok()?
            .find(|d| d.name().ok().as_deref() == Some(name))
    } else {
        // Use default device
        host.default_output_device()
    };

    let device = device?;
    let config = device.default_output_config().ok()?;
    Some(config.sample_rate().0)
}

/// Map output device name to CamillaDSP format
fn map_output_device(device: Option<&str>) -> CamillaResult<(String, Option<String>)> {
    match device {
        None => {
            // Use default device for the platform
            #[cfg(target_os = "macos")]
            return Ok(("CoreAudio".to_string(), None));

            #[cfg(target_os = "linux")]
            return Ok(("Alsa".to_string(), Some("default".to_string())));

            #[cfg(target_os = "windows")]
            return Ok(("Wasapi".to_string(), None));

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            return Err(CamillaError::InvalidConfiguration(
                "Unsupported platform".to_string(),
            ));
        }
        Some(name) => {
            #[cfg(target_os = "macos")]
            return Ok(("CoreAudio".to_string(), Some(name.to_string())));

            #[cfg(target_os = "linux")]
            return Ok(("Alsa".to_string(), Some(name.to_string())));

            #[cfg(target_os = "windows")]
            return Ok(("Wasapi".to_string(), Some(name.to_string())));

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            return Err(CamillaError::InvalidConfiguration(
                "Unsupported platform".to_string(),
            ));
        }
    }
}

/// Map input device name to CamillaDSP format
fn map_input_device(device: Option<&str>) -> CamillaResult<(String, Option<String>)> {
    // Same logic as output device for now
    map_output_device(device)
}

/// Generate the filters section as YAML
fn generate_filters_yaml(filters: &[FilterParams]) -> CamillaResult<serde_yaml::Value> {
    let mut filters_map = serde_yaml::Mapping::new();

    for (idx, filter) in filters.iter().enumerate() {
        let filter_name = format!("peq{}", idx + 1);

        let mut params = serde_yaml::Mapping::new();
        params.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String(filter.filter_type.clone()),
        );
        params.insert(
            serde_yaml::Value::String("freq".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.frequency as i64)),
        );
        params.insert(
            serde_yaml::Value::String("gain".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.gain as i64)),
        );
        params.insert(
            serde_yaml::Value::String("q".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.q as i64)),
        );

        let mut filter_config = serde_yaml::Mapping::new();
        filter_config.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Biquad".to_string()),
        );
        filter_config.insert(
            serde_yaml::Value::String("parameters".to_string()),
            serde_yaml::Value::Mapping(params),
        );

        filters_map.insert(
            serde_yaml::Value::String(filter_name),
            serde_yaml::Value::Mapping(filter_config),
        );
    }

    Ok(serde_yaml::Value::Mapping(filters_map))
}

/// Generate a stereo mixer configuration
/// Generate a generic N-in / M-out matrix mixer configuration
/// mapping[i] gives the destination output channel index for input channel i
fn generate_matrix_mixer_yaml(
    in_channels: u16,
    out_channels: u16,
    mapping: &[u16],
) -> serde_yaml::Value {
    // Build YAML mapping entries
    // For each destination output channel, collect sources referencing it
    let mut dest_sources: Vec<Vec<u16>> = vec![Vec::new(); out_channels as usize];
    for (src, &dst) in mapping.iter().enumerate() {
        if (dst as usize) < dest_sources.len() {
            dest_sources[dst as usize].push(src as u16);
        }
    }

    // Compose YAML string manually to keep dependency surface small
    let mut s = String::new();
    s.push_str("matrix_mixer:\n");
    s.push_str("  channels:\n");
    s.push_str(&format!("    in: {}\n", in_channels));
    s.push_str(&format!("    out: {}\n", out_channels));
    s.push_str("  mapping:\n");

    for (dest, sources) in dest_sources.iter().enumerate() {
        s.push_str(&format!("    - dest: {}\n", dest));
        s.push_str("      sources:\n");
        if sources.is_empty() {
            // Leave empty sources list (silence on this destination)
            s.push_str("        []\n");
        } else {
            for &src in sources {
                s.push_str(&format!(
                    "        - channel: {}\n          gain: 0\n          inverted: false\n",
                    src
                ));
            }
        }
    }

    serde_yaml::from_str::<serde_yaml::Value>(&s).unwrap()
}

/// Generate the pipeline
/// Note: Resampler is NOT added to pipeline when using devices.resampler - it's automatic
fn generate_pipeline(
    _channels: u16,
    filters: &[FilterParams],
    _needs_resampling: bool,
    include_loudness: bool,
) -> Vec<serde_yaml::Value> {
    let mut pipeline: Vec<Value> = Vec::new();

    // Always add mixer with singular `name` as required by CamillaDSP v3
    let mut mixer_map = Mapping::new();
    mixer_map.insert(
        Value::String("type".to_string()),
        Value::String("Mixer".to_string()),
    );
    mixer_map.insert(
        Value::String("name".to_string()),
        Value::String("matrix_mixer".to_string()),
    );
    pipeline.push(Value::Mapping(mixer_map));

    let mut filter_names: Vec<Value> = Vec::new();

    // Optional Loudness filter
    if include_loudness {
        filter_names.push(Value::String("loudness".to_string()));
    }

    // Add all PEQ filters
    for (idx, _filter) in filters.iter().enumerate() {
        filter_names.push(Value::String(format!("peq{}", idx + 1)));
    }

    // Add a single Filter step with all the filter names
    if !filter_names.is_empty() {
        let mut filter_map = Mapping::new();
        filter_map.insert(
            Value::String("type".to_string()),
            Value::String("Filter".to_string()),
        );
        filter_map.insert(
            Value::String("names".to_string()),
            Value::Sequence(filter_names),
        );
        pipeline.push(Value::Mapping(filter_map));
    }

    pipeline
}

/// Write a config to a temporary YAML file
pub fn write_config_to_temp(config: &CamillaDSPConfig) -> CamillaResult<NamedTempFile> {
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to create temp file: {}", e))
    })?;

    let yaml = serde_yaml::to_string(config)?;
    temp_file.write_all(yaml.as_bytes()).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to write config: {}", e))
    })?;

    temp_file.flush().map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to flush config: {}", e))
    })?;

    println!("[CamillaDSP] Generated config:\n{}", yaml);

    Ok(temp_file)
}

/// Write a config to a specific file path
pub fn write_config_to_file(config: &CamillaDSPConfig, path: &PathBuf) -> CamillaResult<()> {
    let yaml = serde_yaml::to_string(config)?;
    fs::write(path, yaml).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to write config file: {}", e))
    })?;

    println!("[CamillaDSP] Wrote config to: {:?}", path);
    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Find the CamillaDSP binary in the system PATH or bundled location
pub fn find_camilladsp_binary() -> CamillaResult<PathBuf> {
    // Try bundled binary first (Tauri sidecar)
    // In production, the sidecar is in the same directory as the executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let bundled_name = if cfg!(windows) {
            "camilladsp.exe"
        } else {
            "camilladsp"
        };

        let bundled_path = exe_dir.join(bundled_name);
        if bundled_path.exists() {
            println!("[CamillaDSP] Using bundled binary: {:?}", bundled_path);
            return Ok(bundled_path);
        }

        // Check for Tauri sidecar in triplet subdirectory (e.g., camilladsp-aarch64-apple-darwin/camilladsp)
        let triplet = if cfg!(target_os = "windows") {
            if cfg!(target_arch = "x86_64") {
                "x86_64-pc-windows-msvc"
            } else {
                "aarch64-pc-windows-msvc"
            }
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else if cfg!(target_os = "linux") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-unknown-linux-gnu"
            } else {
                "x86_64-unknown-linux-gnu"
            }
        } else {
            ""
        };

        if !triplet.is_empty() {
            let bundled_path_triplet = exe_dir
                .join(format!("camilladsp-{}", triplet))
                .join(bundled_name);
            if bundled_path_triplet.exists() {
                println!(
                    "[CamillaDSP] Using bundled sidecar binary: {:?}",
                    bundled_path_triplet
                );
                return Ok(bundled_path_triplet);
            }
        }
    }

    // Try to find in PATH
    if let Ok(path) = which::which("camilladsp") {
        println!("[CamillaDSP] Using system binary from PATH: {:?}", path);
        return Ok(path);
    }

    // Check common installation locations
    let common_paths = vec![
        PathBuf::from("/usr/local/bin/camilladsp"),
        PathBuf::from("/usr/bin/camilladsp"),
        PathBuf::from("/opt/homebrew/bin/camilladsp"),
    ];

    for path in common_paths {
        if path.exists() {
            println!("[CamillaDSP] Using system binary: {:?}", path);
            return Ok(path);
        }
    }

    Err(CamillaError::BinaryNotFound(
        "CamillaDSP binary not found. Looked for:\n\
         1. Bundled binary (next to executable)\n\
         2. System PATH\n\
         3. Common locations (/usr/local/bin, /usr/bin, /opt/homebrew/bin)\n\
         \n\
         Please install CamillaDSP from https://github.com/HEnquist/camilladsp"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_params_validation() {
        // Valid filter
        let filter = FilterParams::new(1000.0, 1.0, 3.0);
        assert!(filter.validate().is_ok());

        // Invalid frequency (too low)
        let filter = FilterParams::new(10.0, 1.0, 3.0);
        assert!(filter.validate().is_err());

        // Invalid Q (too high)
        let filter = FilterParams::new(1000.0, 150.0, 3.0);
        assert!(filter.validate().is_err());

        // Invalid gain (too high)
        let filter = FilterParams::new(1000.0, 1.0, 50.0);
        assert!(filter.validate().is_err());
    }

    #[test]
    fn test_audio_state_default() {
        let state = AudioStreamState::default();
        assert_eq!(state.state, AudioState::Idle);
        assert_eq!(state.position_seconds, 0.0);
        assert_eq!(state.sample_rate, 48000);
        assert_eq!(state.channels, 2);
        assert!(state.filters.is_empty());
    }

    #[test]
    fn test_filter_params_serialization() {
        let filter = FilterParams::new(1000.0, 1.5, 3.5);
        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterParams = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, deserialized);
    }

    #[test]
    fn test_process_manager_creation() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let process = CamillaDSPProcess::new(binary_path.clone());
        assert_eq!(process.binary_path, binary_path);
        assert!(process.websocket_port > 1024);
        assert!(process.process.is_none());
    }

    #[test]
    fn test_process_manager_builder() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let process = CamillaDSPProcess::new(binary_path)
            .with_port(5678)
            .with_health_check_interval(Duration::from_secs(10));
        assert_eq!(process.websocket_port, 5678);
        assert_eq!(process.health_check_interval, Duration::from_secs(10));
    }

    #[test]
    fn test_websocket_url_generation() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let process = CamillaDSPProcess::new(binary_path).with_port(9999);
        assert_eq!(process.websocket_url(), "ws://127.0.0.1:9999");
    }

    #[test]
    fn test_process_not_running_initially() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let mut process = CamillaDSPProcess::new(binary_path);
        assert!(!process.is_running());
        assert!(process.pid().is_none());
    }

    #[test]
    fn test_generate_playback_config() {
        let audio_file = PathBuf::from("/tmp/test.wav");
        let filters = vec![
            FilterParams::new(100.0, 1.0, 3.0),
            FilterParams::new(1000.0, 1.5, -2.0),
            FilterParams::new(10000.0, 2.0, 1.5),
        ];

        let config = generate_playback_config(
            &audio_file,
            None,
            48000,
            2,
            &filters,
            ChannelMapMode::Normal,
            None,
            None,
        )
        .unwrap();

        assert_eq!(config.devices.samplerate, 48000);
        assert_eq!(
            config.devices.playback.channels,
            Some(ChannelsSetting::Count(2))
        );
        assert!(config.devices.capture.is_some());
        assert!(config.filters.is_some());
        assert!(config.mixers.is_some());
        assert!(config.pipeline.is_some());
    }

    #[test]
    fn test_generate_playback_config_no_filters() {
        let audio_file = PathBuf::from("/tmp/test.wav");
        let filters = vec![];

        let config = generate_playback_config(
            &audio_file,
            None,
            44100,
            2,
            &filters,
            ChannelMapMode::Normal,
            None,
            None,
        )
        .unwrap();

        assert_eq!(config.devices.samplerate, 44100);
        assert!(config.filters.is_none());
        // Pipeline should include at least the mixer now
        assert!(config.pipeline.is_some());
        assert!(config.mixers.is_some());
    }

    #[test]
    fn test_generate_recording_config() {
        let output_file = PathBuf::from("/tmp/recording.wav");
        let config = generate_recording_config(&output_file, None, 48000, 2, None).unwrap();

        assert_eq!(config.devices.samplerate, 48000);
        assert_eq!(
            config.devices.playback.channels,
            Some(ChannelsSetting::Count(2))
        );
        assert!(config.devices.capture.is_some());
        assert_eq!(config.devices.playback.device_type, "File");
    }

    #[test]
    fn test_config_serialization() {
        let audio_file = PathBuf::from("/tmp/test.wav");
        let filters = vec![FilterParams::new(1000.0, 1.0, 3.0)];

        let config = generate_playback_config(
            &audio_file,
            None,
            48000,
            2,
            &filters,
            ChannelMapMode::Normal,
            None,
            None,
        )
        .unwrap();
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Verify YAML contains expected fields
        assert!(yaml.contains("devices"));
        assert!(yaml.contains("samplerate: 48000"));
        assert!(yaml.contains("mixers"));
        assert!(yaml.contains("matrix_mixer"));
        assert!(yaml.contains("filters"));
        assert!(yaml.contains("peq1"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_map_output_device_macos() {
        let (device_type, device_name) = map_output_device(None).unwrap();
        assert_eq!(device_type, "CoreAudio");
        assert!(device_name.is_none());

        let (device_type, device_name) = map_output_device(Some("Built-in Output")).unwrap();
        assert_eq!(device_type, "CoreAudio");
        assert_eq!(device_name.unwrap(), "Built-in Output");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_map_output_device_linux() {
        let (device_type, device_name) = map_output_device(None).unwrap();
        assert_eq!(device_type, "Alsa");
        assert_eq!(device_name.unwrap(), "default");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_map_output_device_windows() {
        let (device_type, device_name) = map_output_device(None).unwrap();
        assert_eq!(device_type, "Wasapi");
        assert!(device_name.is_none());
    }

    #[test]
    fn test_websocket_client_creation() {
        let client = CamillaWebSocketClient::new("ws://127.0.0.1:1234".to_string());
        assert_eq!(client.url, "ws://127.0.0.1:1234");
        assert_eq!(client.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_websocket_client_with_timeout() {
        let client = CamillaWebSocketClient::new("ws://127.0.0.1:1234".to_string())
            .with_timeout(Duration::from_secs(10));
        assert_eq!(client.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_command_serialization() {
        // Build JSON the same way send_command does
        let cmd = CamillaCommand::GetState;
        let json = match cmd {
            CamillaCommand::GetState => serde_json::json!({"GetState": {}}).to_string(),
            _ => String::new(),
        };
        assert!(json.contains("GetState"));

        let cmd = CamillaCommand::Stop;
        let json = match cmd {
            CamillaCommand::Stop => serde_json::json!({"Stop": {}}).to_string(),
            _ => String::new(),
        };
        assert!(json.contains("Stop"));

        let cmd = CamillaCommand::SetConfig {
            config: "test config".to_string(),
        };
        let json = match cmd {
            CamillaCommand::SetConfig { ref config } => {
                serde_json::json!({"SetConfig": {"config": config}}).to_string()
            }
            _ => String::new(),
        };
        assert!(json.contains("SetConfig"));
        assert!(json.contains("test config"));
    }

    #[test]
    fn test_response_deserialization() {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum CamillaResponse {
            State { state: String },
            Error { error: String },
            SignalPeak { value: f64 },
        }

        // Test State response
        let json = r#"{"state":"Playing"}"#;
        let response: CamillaResponse = serde_json::from_str(json).unwrap();
        match response {
            CamillaResponse::State { state } => assert_eq!(state, "Playing"),
            _ => panic!("Wrong response type"),
        }

        // Test Error response
        let json = r#"{"error":"Something went wrong"}"#;
        let response: CamillaResponse = serde_json::from_str(json).unwrap();
        match response {
            CamillaResponse::Error { error } => assert_eq!(error, "Something went wrong"),
            _ => panic!("Wrong response type"),
        }

        // Test SignalPeak response
        let json = r#"{"value":-12.5}"#;
        let response: CamillaResponse = serde_json::from_str(json).unwrap();
        match response {
            CamillaResponse::SignalPeak { value } => assert_eq!(value, -12.5),
            _ => panic!("Wrong response type"),
        }
    }

    #[test]
    fn test_audio_manager_creation() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let manager = AudioManager::new(binary_path);
        let state = manager.get_state().unwrap();
        assert_eq!(state.state, AudioState::Idle);
        assert_eq!(state.sample_rate, 48000);
        assert_eq!(state.channels, 2);
    }

    #[test]
    fn test_audio_manager_state_access() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let manager = AudioManager::new(binary_path);

        // Get initial state
        let state = manager.get_state().unwrap();
        assert_eq!(state.state, AudioState::Idle);

        // Check playing/recording status
        assert!(!manager.is_playing().unwrap());
        assert!(!manager.is_recording().unwrap());

        // Get shared state
        let shared = manager.shared_state();
        let state_from_shared = shared.lock().unwrap();
        assert_eq!(state_from_shared.state, AudioState::Idle);
    }

    #[test]
    fn test_audio_manager_error_handling() {
        let binary_path = PathBuf::from("/usr/local/bin/camilladsp");
        let manager = AudioManager::new(binary_path);

        // Set error via internal method
        manager.set_error("Test error").unwrap();

        let state = manager.get_state().unwrap();
        assert_eq!(state.state, AudioState::Error);
        assert_eq!(state.error_message, Some("Test error".to_string()));
    }
    #[test]
    fn test_generate_matrix_mixer_16ch_identity() {
        // Identity mapping for 16 channels
        let in_ch = 16u16;
        let out_ch = 16u16;
        let mapping: Vec<u16> = (0..in_ch).collect();
        let v = generate_matrix_mixer_yaml(in_ch, out_ch, &mapping);
        let yaml = serde_yaml::to_string(&v).unwrap();
        assert!(yaml.contains("matrix_mixer"));
        assert!(yaml.contains("in: 16"));
        assert!(yaml.contains("out: 16"));
    }
}
