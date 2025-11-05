// ============================================================================
// WebSocket Communication
// ============================================================================

use futures_util::{SinkExt, StreamExt};
use serde_json;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::errors::{CamillaError, CamillaResult};

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
    SetVolume { volume: f32 },
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
                // CamillaDSP expects: {"SetConfig": "<yaml_string>"}
                let command_json = serde_json::json!({ "SetConfig": config }).to_string();
                println!("[WebSocket] Sending SetConfig command");
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
            CamillaCommand::SetVolume { ref volume } => {
                println!("[WebSocket] Sending command: SetVolume");
                let command_json = serde_json::json!({ "SetVolume": volume }).to_string();
                write
                    .send(Message::Text(command_json))
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

        // Debug: print the full response to help diagnose the issue
        println!("[WebSocket] SetConfig response JSON: {:?}", v);

        let set_config = v.get("SetConfig").ok_or_else(|| {
            CamillaError::WebSocketError(format!(
                "Unexpected response format: missing SetConfig field. Got: {}",
                serde_json::to_string(&v).unwrap_or_else(|_| "<invalid>".to_string())
            ))
        })?;

        let result = set_config
            .get("result")
            .and_then(|x| x.as_str())
            .unwrap_or("");

        if result == "Ok" {
            Ok(())
        } else {
            // Extract the error message from the "value" field if available
            let error_msg = set_config
                .get("value")
                .and_then(|x| x.as_str())
                .unwrap_or("SetConfig failed with unknown error");

            Err(CamillaError::ProcessCommunicationFailed(format!(
                "SetConfig failed: {}",
                error_msg
            )))
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

    /// Set volume for all channels
    pub async fn set_volume(&self, volume: f32) -> CamillaResult<()> {
        let text = self
            .send_command(CamillaCommand::SetVolume { volume })
            .await?;
        let v: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| CamillaError::WebSocketError(format!("JSON parse error: {}", e)))?;
        let result = v
            .get("SetVolume")
            .and_then(|x| x.get("result"))
            .and_then(|x| x.as_str())
            .ok_or_else(|| {
                CamillaError::WebSocketError("Unexpected response format".to_string())
            })?;

        if result != "Ok" {
            return Err(CamillaError::WebSocketError(format!(
                "SetVolume failed: {}",
                text
            )));
        }

        Ok(())
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
