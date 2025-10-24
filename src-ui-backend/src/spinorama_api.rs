use serde::{Deserialize, Serialize};
use reqwest;
use crate::plot::CurveData;

const SPINORAMA_API_BASE: &str = "https://api.spinorama.org";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerInfo {
    pub brand: String,
    pub model: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementInfo {
    pub measurement_id: String,
    pub measurement_type: String,
    pub reviewer: String,
}

#[derive(Debug, Clone)]
pub struct SpinAudioClient {
    client: reqwest::Client,
}

impl SpinAudioClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// List all available speakers
    pub async fn list_speakers(&self) -> Result<Vec<SpeakerInfo>, String> {
        let url = format!("{}/speakers", SPINORAMA_API_BASE);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch speakers: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API returned error: {}", response.status()));
        }

        response
            .json::<Vec<SpeakerInfo>>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get measurements for a specific speaker
    pub async fn get_measurements(&self, brand: &str, model: &str) -> Result<Vec<MeasurementInfo>, String> {
        let url = format!("{}/speakers/{}/{}/measurements", SPINORAMA_API_BASE, brand, model);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch measurements: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API returned error: {}", response.status()));
        }

        response
            .json::<Vec<MeasurementInfo>>()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Get CEA2034 data for a specific measurement
    pub async fn get_cea2034_data(
        &self,
        brand: &str,
        model: &str,
        measurement_id: &str,
    ) -> Result<Cea2034Data, String> {
        let url = format!(
            "{}/speakers/{}/{}/measurements/{}/cea2034",
            SPINORAMA_API_BASE, brand, model, measurement_id
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch CEA2034 data: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API returned error: {}", response.status()));
        }

        response
            .json::<Cea2034Data>()
            .await
            .map_err(|e| format!("Failed to parse CEA2034 data: {}", e))
    }

    /// Get frequency response curve for a specific measurement type
    pub async fn get_frequency_response(
        &self,
        brand: &str,
        model: &str,
        measurement_id: &str,
        curve_name: &str,
    ) -> Result<FrequencyResponse, String> {
        let url = format!(
            "{}/speakers/{}/{}/measurements/{}/curves/{}",
            SPINORAMA_API_BASE, brand, model, measurement_id, curve_name
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch frequency response: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API returned error: {}", response.status()));
        }

        response
            .json::<FrequencyResponse>()
            .await
            .map_err(|e| format!("Failed to parse frequency response: {}", e))
    }
}

impl Default for SpinAudioClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cea2034Data {
    pub on_axis: FrequencyResponse,
    pub listening_window: FrequencyResponse,
    pub early_reflections: FrequencyResponse,
    pub sound_power: FrequencyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyResponse {
    pub frequencies: Vec<f64>,
    pub magnitudes: Vec<f64>,
}

impl From<FrequencyResponse> for CurveData {
    fn from(fr: FrequencyResponse) -> Self {
        CurveData {
            freq: fr.frequencies,
            spl: fr.magnitudes,
        }
    }
}
