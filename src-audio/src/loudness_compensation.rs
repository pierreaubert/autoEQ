use crate::camilla::{CamillaError, CamillaResult};
use serde::{Deserialize, Serialize};

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

