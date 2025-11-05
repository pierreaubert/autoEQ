// ============================================================================
// Filter Parameters
// ============================================================================

use crate::camilla::{CamillaError, CamillaResult};
use serde::{Deserialize, Serialize};

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
