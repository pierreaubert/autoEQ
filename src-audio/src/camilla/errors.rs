// ============================================================================
// Error Types
// ============================================================================

use serde::{Deserialize, Serialize};

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
