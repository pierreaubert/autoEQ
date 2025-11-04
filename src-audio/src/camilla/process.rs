// ============================================================================
// CamillaDSP Process Management
// ============================================================================

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::errors::{CamillaError, CamillaResult};

/// CamillaDSP subprocess manager
pub struct CamillaDSPProcess {
    /// Child process handle
    process: Option<Child>,
    /// Stdin handle for streaming audio data
    pub stdin: Option<ChildStdin>,
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
