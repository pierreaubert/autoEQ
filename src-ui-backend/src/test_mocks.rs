#[cfg(test)]
#[allow(dead_code)]
pub mod mocks {
    use crate::OptimizationParams;
    use std::collections::HashMap;

    // Mock HTTP client for testing API calls
    pub struct MockHttpClient {
        pub responses: HashMap<String, Result<serde_json::Value, String>>,
    }

    impl MockHttpClient {
        pub fn new() -> Self {
            Self {
                responses: HashMap::new(),
            }
        }

        pub fn add_response(&mut self, url: &str, response: Result<serde_json::Value, String>) {
            self.responses.insert(url.to_string(), response);
        }

        pub async fn get(&self, url: &str) -> Result<serde_json::Value, String> {
            self.responses
                .get(url)
                .cloned()
                .unwrap_or_else(|| Err("URL not mocked".to_string()))
        }
    }

    // Mock functions for testing without network dependencies
    pub async fn mock_get_speakers() -> Result<Vec<String>, String> {
        Ok(vec![
            "KEF LS50".to_string(),
            "JBL M2".to_string(),
            "Genelec 8030A".to_string(),
        ])
    }

    pub async fn mock_get_speaker_versions(speaker: String) -> Result<Vec<String>, String> {
        if speaker.is_empty() {
            return Err("Speaker name cannot be empty".to_string());
        }

        Ok(vec![
            "v1.0".to_string(),
            "v1.1".to_string(),
            "v2.0".to_string(),
        ])
    }

    pub async fn mock_get_speaker_measurements(
        speaker: String,
        version: String,
    ) -> Result<Vec<String>, String> {
        if speaker.is_empty() || version.is_empty() {
            return Err("Speaker name and version cannot be empty".to_string());
        }

        Ok(vec![
            "On Axis".to_string(),
            "Listening Window".to_string(),
            "Early Reflections".to_string(),
            "Sound Power".to_string(),
        ])
    }

    // Helper to create test data for optimization
    pub fn create_minimal_optimization_params() -> OptimizationParams {
        OptimizationParams {
            num_filters: 3,
            curve_path: None,
            target_path: None,
            sample_rate: 48000.0,
            max_db: 3.0,
            min_db: 1.0,
            max_q: 5.0,
            min_q: 0.5,
            min_freq: 100.0,
            max_freq: 10000.0,
            speaker: Some("Test Speaker".to_string()),
            version: Some("v1.0".to_string()),
            measurement: Some("On Axis".to_string()),
            curve_name: "Listening Window".to_string(),
            algo: "nlopt:cobyla".to_string(),
            population: 10,
            maxeval: 50,
            refine: false,
            local_algo: "cobyla".to_string(),
            min_spacing_oct: 0.5,
            spacing_weight: 1.0,
            smooth: false,
            smooth_n: 1,
            loss: "speaker-flat".to_string(),
            peq_model: Some("pk".to_string()),
            strategy: None,
            de_f: None,
            de_cr: None,
            adaptive_weight_f: None,
            adaptive_weight_cr: None,
            tolerance: Some(1e-2),
            atolerance: Some(1e-3),
            captured_frequencies: None,
            captured_magnitudes: None,
            target_frequencies: None,
            target_magnitudes: None,
        }
    }

    // Helper to create edge case parameters for testing validation
    pub fn create_edge_case_params() -> Vec<(String, OptimizationParams)> {
        let base = create_minimal_optimization_params();

        vec![
            (
                "max_filters".to_string(),
                OptimizationParams {
                    num_filters: 50,
                    ..base.clone()
                },
            ),
            (
                "min_freq_boundary".to_string(),
                OptimizationParams {
                    min_freq: 20.0,
                    ..base.clone()
                },
            ),
            (
                "max_freq_boundary".to_string(),
                OptimizationParams {
                    max_freq: 20000.0,
                    ..base.clone()
                },
            ),
            (
                "min_q_boundary".to_string(),
                OptimizationParams {
                    min_q: 0.1,
                    ..base.clone()
                },
            ),
            (
                "max_q_boundary".to_string(),
                OptimizationParams {
                    max_q: 20.0,
                    ..base.clone()
                },
            ),
            (
                "min_db_boundary".to_string(),
                OptimizationParams {
                    min_db: 0.25,
                    ..base.clone()
                },
            ),
            (
                "max_db_boundary".to_string(),
                OptimizationParams {
                    max_db: 20.0,
                    ..base.clone()
                },
            ),
            (
                "min_sample_rate".to_string(),
                OptimizationParams {
                    sample_rate: 8000.0,
                    ..base.clone()
                },
            ),
            (
                "max_sample_rate".to_string(),
                OptimizationParams {
                    sample_rate: 192000.0,
                    ..base.clone()
                },
            ),
            (
                "max_population".to_string(),
                OptimizationParams {
                    population: 10000,
                    ..base.clone()
                },
            ),
            (
                "max_smooth_n".to_string(),
                OptimizationParams {
                    smooth_n: 24,
                    ..base.clone()
                },
            ),
            (
                "valid_de_f".to_string(),
                OptimizationParams {
                    de_f: Some(0.8),
                    ..base.clone()
                },
            ),
            (
                "valid_de_cr".to_string(),
                OptimizationParams {
                    de_cr: Some(0.9),
                    ..base.clone()
                },
            ),
            (
                "min_tolerance".to_string(),
                OptimizationParams {
                    tolerance: Some(1e-12),
                    ..base.clone()
                },
            ),
            (
                "min_atolerance".to_string(),
                OptimizationParams {
                    atolerance: Some(1e-15),
                    ..base.clone()
                },
            ),
        ]
    }

    // Helper to create invalid parameters for testing validation errors
    pub fn create_invalid_params() -> Vec<(String, OptimizationParams, &'static str)> {
        let base = create_minimal_optimization_params();

        vec![
            (
                "zero_filters".to_string(),
                OptimizationParams {
                    num_filters: 0,
                    ..base.clone()
                },
                "Number of filters must be at least 1",
            ),
            (
                "too_many_filters".to_string(),
                OptimizationParams {
                    num_filters: 51,
                    ..base.clone()
                },
                "Number of filters must be between 1 and 50",
            ),
            (
                "invalid_freq_range".to_string(),
                OptimizationParams {
                    min_freq: 1000.0,
                    max_freq: 500.0,
                    ..base.clone()
                },
                "Minimum frequency",
            ),
            (
                "freq_too_low".to_string(),
                OptimizationParams {
                    min_freq: 10.0,
                    ..base.clone()
                },
                "Minimum frequency must be >= 20 Hz",
            ),
            (
                "freq_too_high".to_string(),
                OptimizationParams {
                    max_freq: 25000.0,
                    ..base.clone()
                },
                "Maximum frequency must be <= 20,000 Hz",
            ),
            (
                "invalid_q_range".to_string(),
                OptimizationParams {
                    min_q: 5.0,
                    max_q: 2.0,
                    ..base.clone()
                },
                "Minimum Q",
            ),
            (
                "q_too_low".to_string(),
                OptimizationParams {
                    min_q: 0.05,
                    ..base.clone()
                },
                "Minimum Q must be >= 0.1",
            ),
            (
                "q_too_high".to_string(),
                OptimizationParams {
                    max_q: 25.0,
                    ..base.clone()
                },
                "Maximum Q must be <= 100",
            ),
            (
                "invalid_db_range".to_string(),
                OptimizationParams {
                    min_db: 10.0,
                    max_db: 5.0,
                    ..base.clone()
                },
                "Minimum dB",
            ),
            (
                "db_too_low".to_string(),
                OptimizationParams {
                    min_db: 0.1,
                    ..base.clone()
                },
                "Minimum dB must be >= 0.25",
            ),
            (
                "db_too_high".to_string(),
                OptimizationParams {
                    max_db: 25.0,
                    ..base.clone()
                },
                "Maximum dB must be <= 20",
            ),
            (
                "sample_rate_too_low".to_string(),
                OptimizationParams {
                    sample_rate: 4000.0,
                    ..base.clone()
                },
                "Sample rate must be between",
            ),
            (
                "sample_rate_too_high".to_string(),
                OptimizationParams {
                    sample_rate: 200000.0,
                    ..base.clone()
                },
                "Sample rate must be between",
            ),
            (
                "zero_population".to_string(),
                OptimizationParams {
                    population: 0,
                    ..base.clone()
                },
                "Population size must be at least 1",
            ),
            (
                "population_too_high".to_string(),
                OptimizationParams {
                    population: 15000,
                    ..base.clone()
                },
                "Population size must be between",
            ),
            (
                "zero_maxeval".to_string(),
                OptimizationParams {
                    maxeval: 0,
                    ..base.clone()
                },
                "Maximum evaluations must be at least 1",
            ),
            (
                "smooth_n_too_low".to_string(),
                OptimizationParams {
                    smooth_n: 0,
                    ..base.clone()
                },
                "Smoothing N must be between",
            ),
            (
                "smooth_n_too_high".to_string(),
                OptimizationParams {
                    smooth_n: 30,
                    ..base.clone()
                },
                "Smoothing N must be between",
            ),
            (
                "de_f_too_low".to_string(),
                OptimizationParams {
                    de_f: Some(-0.1),
                    ..base.clone()
                },
                "Mutation factor",
            ),
            (
                "de_f_too_high".to_string(),
                OptimizationParams {
                    de_f: Some(2.5),
                    ..base.clone()
                },
                "Mutation factor",
            ),
            (
                "de_cr_too_low".to_string(),
                OptimizationParams {
                    de_cr: Some(-0.1),
                    ..base.clone()
                },
                "Recombination probability",
            ),
            (
                "de_cr_too_high".to_string(),
                OptimizationParams {
                    de_cr: Some(1.5),
                    ..base.clone()
                },
                "Recombination probability",
            ),
            (
                "tolerance_too_low".to_string(),
                OptimizationParams {
                    tolerance: Some(1e-15),
                    ..base.clone()
                },
                "Tolerance must be >= 1e-12",
            ),
            (
                "atolerance_too_low".to_string(),
                OptimizationParams {
                    atolerance: Some(1e-20),
                    ..base.clone()
                },
                "Absolute tolerance must be >= 1e-15",
            ),
        ]
    }
}
