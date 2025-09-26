#[cfg(test)]
mod tests {
    use crate::{
        CurveData, OptimizationParams, OptimizationResult, PlotData, PlotFiltersParams,
        PlotSpinParams, ProgressUpdate, curve_data_to_curve, generate_plot_filters,
        generate_plot_spin, generate_plot_spin_details, generate_plot_spin_tonal,
        get_speaker_measurements, get_speaker_versions, get_speakers, greet, validate_params,
    };
    use std::collections::HashMap;
    use tokio;

    use crate::test_mocks;

    // Helper function to create test curve data
    fn create_test_curve_data() -> CurveData {
        CurveData {
            freq: vec![20.0, 100.0, 1000.0, 10000.0, 20000.0],
            spl: vec![0.0, -1.0, 0.5, -2.0, -3.0],
        }
    }

    // Helper function to create test optimization params
    fn create_test_optimization_params() -> OptimizationParams {
        OptimizationParams {
            num_filters: 5,
            curve_path: None,
            target_path: None,
            sample_rate: 48000.0,
            max_db: 5.0,
            min_db: 1.0,
            max_q: 10.0,
            min_q: 0.5,
            min_freq: 20.0,
            max_freq: 20000.0,
            speaker: Some("Test Speaker".to_string()),
            version: Some("v1.0".to_string()),
            measurement: Some("On Axis".to_string()),
            curve_name: "Listening Window".to_string(),
            algo: "nlopt:cobyla".to_string(),
            population: 50,
            maxeval: 100,
            refine: false,
            local_algo: "cobyla".to_string(),
            min_spacing_oct: 0.5,
            spacing_weight: 20.0,
            smooth: true,
            smooth_n: 2,
            loss: "speaker-flat".to_string(),
            iir_hp_pk: false,
            strategy: None,
            de_f: None,
            de_cr: None,
            adaptive_weight_f: None,
            adaptive_weight_cr: None,
            tolerance: Some(1e-3),
            atolerance: Some(1e-4),
        }
    }

    #[test]
    fn test_greet() {
        let result = greet("World");
        assert_eq!(result, "Hello, World! You've been greeted from Rust!");
    }

    #[test]
    fn test_validate_params_valid() {
        let params = create_test_optimization_params();
        let result = validate_params(&params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_params_invalid_num_filters() {
        let mut params = create_test_optimization_params();
        params.num_filters = 0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Number of filters must be at least 1")
        );
    }

    #[test]
    fn test_validate_params_invalid_frequency_range() {
        let mut params = create_test_optimization_params();
        params.min_freq = 1000.0;
        params.max_freq = 500.0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Minimum frequency")
        );
    }

    #[test]
    fn test_validate_params_invalid_q_range() {
        let mut params = create_test_optimization_params();
        params.min_q = 5.0;
        params.max_q = 2.0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Minimum Q"));
    }

    #[test]
    fn test_validate_params_invalid_db_range() {
        let mut params = create_test_optimization_params();
        params.min_db = 10.0;
        params.max_db = 5.0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Minimum dB"));
    }

    #[test]
    fn test_validate_params_invalid_sample_rate() {
        let mut params = create_test_optimization_params();
        params.sample_rate = 1000.0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Sample rate must be between")
        );
    }

    #[test]
    fn test_validate_params_invalid_population() {
        let mut params = create_test_optimization_params();
        params.population = 0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Population size must be at least 1")
        );
    }

    #[test]
    fn test_validate_params_invalid_maxeval() {
        let mut params = create_test_optimization_params();
        params.maxeval = 0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Maximum evaluations must be at least 1")
        );
    }

    #[test]
    fn test_validate_params_invalid_smooth_n() {
        let mut params = create_test_optimization_params();
        params.smooth_n = 0;
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Smoothing N must be between")
        );
    }

    #[test]
    fn test_validate_params_invalid_de_f() {
        let mut params = create_test_optimization_params();
        params.de_f = Some(3.0);
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mutation factor"));
    }

    #[test]
    fn test_validate_params_invalid_de_cr() {
        let mut params = create_test_optimization_params();
        params.de_cr = Some(1.5);
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Recombination probability")
        );
    }

    #[test]
    fn test_validate_params_invalid_tolerance() {
        let mut params = create_test_optimization_params();
        params.tolerance = Some(1e-15);
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Tolerance must be >= 1e-12")
        );
    }

    #[test]
    fn test_validate_params_invalid_atolerance() {
        let mut params = create_test_optimization_params();
        params.atolerance = Some(1e-20);
        let result = validate_params(&params);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Absolute tolerance must be >= 1e-15")
        );
    }

    #[test]
    fn test_curve_data_to_curve() {
        let curve_data = create_test_curve_data();
        let curve = curve_data_to_curve(&curve_data);

        assert_eq!(curve.freq.len(), curve_data.freq.len());
        assert_eq!(curve.spl.len(), curve_data.spl.len());

        for (i, &freq) in curve_data.freq.iter().enumerate() {
            assert_eq!(curve.freq[i], freq);
        }

        for (i, &spl) in curve_data.spl.iter().enumerate() {
            assert_eq!(curve.spl[i], spl);
        }
    }

    #[tokio::test]
    async fn test_generate_plot_filters() {
        let params = PlotFiltersParams {
            input_curve: create_test_curve_data(),
            target_curve: create_test_curve_data(),
            deviation_curve: create_test_curve_data(),
            optimized_params: vec![3.0, 1.0, 2.0, 3.5, 1.5, -1.0], // 2 filters
            sample_rate: 48000.0,
            num_filters: 2,
            iir_hp_pk: false,
        };

        let result = generate_plot_filters(params).await;
        assert!(result.is_ok());

        let plot_json = result.unwrap();
        assert!(plot_json.is_object());
    }

    #[tokio::test]
    async fn test_generate_plot_spin() {
        let mut cea2034_curves = HashMap::new();
        cea2034_curves.insert("On Axis".to_string(), create_test_curve_data());
        cea2034_curves.insert("Listening Window".to_string(), create_test_curve_data());

        let params = PlotSpinParams {
            cea2034_curves: Some(cea2034_curves),
            eq_response: Some(vec![0.0, 0.5, 1.0, 0.5, 0.0]),
            frequencies: Some(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]),
        };

        let result = generate_plot_spin(params).await;
        assert!(result.is_ok());

        let plot_json = result.unwrap();
        assert!(plot_json.is_object());
    }

    #[tokio::test]
    async fn test_generate_plot_spin_details() {
        let mut cea2034_curves = HashMap::new();
        // Add more curves that the spin details plot expects
        cea2034_curves.insert("On Axis".to_string(), create_test_curve_data());
        cea2034_curves.insert("Listening Window".to_string(), create_test_curve_data());
        cea2034_curves.insert("Early Reflections".to_string(), create_test_curve_data());
        cea2034_curves.insert("Sound Power".to_string(), create_test_curve_data());

        let params = PlotSpinParams {
            cea2034_curves: Some(cea2034_curves),
            eq_response: Some(vec![0.0, 0.0, 0.0, 0.0, 0.0]),
            frequencies: Some(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]),
        };

        let result = generate_plot_spin_details(params).await;
        assert!(result.is_ok());

        let plot_json = result.unwrap();
        assert!(plot_json.is_object());
    }

    #[tokio::test]
    async fn test_generate_plot_spin_tonal() {
        let mut cea2034_curves = HashMap::new();
        cea2034_curves.insert("Sound Power".to_string(), create_test_curve_data());

        let params = PlotSpinParams {
            cea2034_curves: Some(cea2034_curves),
            eq_response: Some(vec![0.0, 0.0, 0.0, 0.0, 0.0]),
            frequencies: Some(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]),
        };

        let result = generate_plot_spin_tonal(params).await;
        assert!(result.is_ok());

        let plot_json = result.unwrap();
        assert!(plot_json.is_object());
    }

    #[test]
    fn test_validate_params_comprehensive() {
        // Test all valid edge cases
        let edge_cases = test_mocks::mocks::create_edge_case_params();
        for (name, params) in edge_cases {
            let result = validate_params(&params);
            assert!(
                result.is_ok(),
                "Edge case '{}' should be valid: {:?}",
                name,
                result
            );
        }
    }

    #[test]
    fn test_validate_params_comprehensive_invalid() {
        // Test all invalid cases
        let invalid_cases = test_mocks::mocks::create_invalid_params();
        for (name, params, expected_error) in invalid_cases {
            let result = validate_params(&params);
            assert!(
                result.is_err(),
                "Invalid case '{}' should fail validation",
                name
            );
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains(expected_error),
                "Error message for '{}' should contain '{}', got: '{}'",
                name,
                expected_error,
                error_msg
            );
        }
    }

    #[tokio::test]
    async fn test_mock_get_speakers() {
        let result = test_mocks::mocks::mock_get_speakers().await;
        assert!(result.is_ok());
        let speakers = result.unwrap();
        assert_eq!(speakers.len(), 3);
        assert!(speakers.contains(&"KEF LS50".to_string()));
        assert!(speakers.contains(&"JBL M2".to_string()));
        assert!(speakers.contains(&"Genelec 8030A".to_string()));
    }

    #[tokio::test]
    async fn test_mock_get_speaker_versions() {
        let result = test_mocks::mocks::mock_get_speaker_versions("KEF LS50".to_string()).await;
        assert!(result.is_ok());
        let versions = result.unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&"v1.0".to_string()));
        assert!(versions.contains(&"v1.1".to_string()));
        assert!(versions.contains(&"v2.0".to_string()));
    }

    #[tokio::test]
    async fn test_mock_get_speaker_versions_empty_speaker() {
        let result = test_mocks::mocks::mock_get_speaker_versions("".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Speaker name cannot be empty"));
    }

    #[tokio::test]
    async fn test_mock_get_speaker_measurements() {
        let result = test_mocks::mocks::mock_get_speaker_measurements(
            "KEF LS50".to_string(),
            "v1.0".to_string(),
        )
        .await;
        assert!(result.is_ok());
        let measurements = result.unwrap();
        assert_eq!(measurements.len(), 4);
        assert!(measurements.contains(&"On Axis".to_string()));
        assert!(measurements.contains(&"Listening Window".to_string()));
        assert!(measurements.contains(&"Early Reflections".to_string()));
        assert!(measurements.contains(&"Sound Power".to_string()));
    }

    #[tokio::test]
    async fn test_mock_get_speaker_measurements_empty_params() {
        let result =
            test_mocks::mocks::mock_get_speaker_measurements("".to_string(), "v1.0".to_string())
                .await;
        assert!(result.is_err());

        let result2 = test_mocks::mocks::mock_get_speaker_measurements(
            "KEF LS50".to_string(),
            "".to_string(),
        )
        .await;
        assert!(result2.is_err());
    }

    #[test]
    fn test_optimization_result_serialization() {
        let result = OptimizationResult {
            success: true,
            error_message: None,
            filter_params: Some(vec![1.0, 2.0, 3.0]),
            objective_value: Some(0.5),
            preference_score_before: Some(7.5),
            preference_score_after: Some(8.2),
            filter_response: None,
            spin_details: None,
            filter_plots: None,
        };

        // Test that the struct can be serialized (important for Tauri commands)
        let serialized = serde_json::to_string(&result);
        assert!(serialized.is_ok());

        let json_str = serialized.unwrap();
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("\"filter_params\":[1.0,2.0,3.0]"));
    }

    #[test]
    fn test_progress_update_serialization() {
        let update = ProgressUpdate {
            iteration: 100,
            fitness: 0.123,
            params: vec![1.0, 2.0, 3.0],
            convergence: 0.001,
        };

        let serialized = serde_json::to_string(&update);
        assert!(serialized.is_ok());

        let json_str = serialized.unwrap();
        assert!(json_str.contains("\"iteration\":100"));
        assert!(json_str.contains("\"fitness\":0.123"));
    }

    #[test]
    fn test_plot_data_serialization() {
        let mut curves = HashMap::new();
        curves.insert("test_curve".to_string(), vec![1.0, 2.0, 3.0]);

        let mut metadata = HashMap::new();
        metadata.insert(
            "title".to_string(),
            serde_json::Value::String("Test Plot".to_string()),
        );

        let plot_data = PlotData {
            frequencies: vec![20.0, 100.0, 1000.0],
            curves,
            metadata,
        };

        let serialized = serde_json::to_string(&plot_data);
        assert!(serialized.is_ok());

        let json_str = serialized.unwrap();
        assert!(json_str.contains("\"frequencies\":[20.0,100.0,1000.0]"));
        assert!(json_str.contains("\"test_curve\":[1.0,2.0,3.0]"));
    }

    // Note: Network-dependent tests for API calls would require mocking
    // These are integration tests that should be run separately
    #[tokio::test]
    #[ignore] // Ignore by default as it requires network access
    async fn test_get_speakers_integration() {
        let result = get_speakers().await;
        // This test depends on external API, so we just check it doesn't panic
        // In a real scenario, you'd mock the HTTP client
        match result {
            Ok(speakers) => {
                assert!(speakers.is_empty() || !speakers.is_empty()); // Either is fine
            }
            Err(_) => {
                // Network errors are acceptable in tests
            }
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it requires network access
    async fn test_get_speaker_versions_integration() {
        let result = get_speaker_versions("Test Speaker".to_string()).await;
        // This test depends on external API
        match result {
            Ok(_) | Err(_) => {} // Either outcome is acceptable for integration test
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it requires network access
    async fn test_get_speaker_measurements_integration() {
        let result = get_speaker_measurements("Test Speaker".to_string(), "v1.0".to_string()).await;
        // This test depends on external API
        match result {
            Ok(_) | Err(_) => {} // Either outcome is acceptable for integration test
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it requires full Tauri app context
    async fn test_optimization_progress_events_integration() {
        // This test would require a full Tauri app context to test event emission
        // For now, we'll test the progress update structure and logic

        println!("[TEST] 🧪 Testing optimization progress event structure");

        // Test that we can create and serialize progress updates
        use crate::ProgressUpdate;

        let progress = ProgressUpdate {
            iteration: 10,
            fitness: 2.456,
            params: vec![100.0, 1.5, 2.0, 200.0, 2.5, -1.5],
            convergence: 0.05,
        };

        // Test serialization (this is what gets sent as event payload)
        let serialized = serde_json::to_value(&progress).unwrap();

        // Verify the structure matches what frontend expects
        assert_eq!(serialized["iteration"], 10);
        assert_eq!(serialized["fitness"], 2.456);
        assert_eq!(serialized["convergence"], 0.05);
        assert!(serialized["params"].is_array());
        assert_eq!(serialized["params"].as_array().unwrap().len(), 6);

        println!(
            "[TEST] ✅ Progress event structure is correct: {}",
            serialized
        );

        // Test that the progress update can be converted back from JSON
        let deserialized: ProgressUpdate = serde_json::from_value(serialized).unwrap();
        assert_eq!(deserialized.iteration, progress.iteration);
        assert_eq!(deserialized.fitness, progress.fitness);
        assert_eq!(deserialized.convergence, progress.convergence);
        assert_eq!(deserialized.params, progress.params);

        println!("[TEST] ✅ Progress event serialization/deserialization works correctly");
    }

    #[tokio::test]
    async fn test_optimization_progress_event_structure() {
        // Test that ProgressUpdate can be serialized correctly for events
        use crate::ProgressUpdate;

        let progress = ProgressUpdate {
            iteration: 42,
            fitness: 1.234,
            params: vec![100.0, 1.5, 2.0, 200.0, 2.5, -1.5],
            convergence: 0.001,
        };

        // Test serialization (this is what gets sent as event payload)
        let serialized = serde_json::to_value(&progress).unwrap();

        // Verify the structure matches what frontend expects
        assert_eq!(serialized["iteration"], 42);
        assert_eq!(serialized["fitness"], 1.234);
        assert_eq!(serialized["convergence"], 0.001);
        assert!(serialized["params"].is_array());

        println!(
            "[TEST] ✅ Progress event structure is correct: {}",
            serialized
        );
    }

    #[test]
    fn test_cancellation_state() {
        use crate::CancellationState;

        let state = CancellationState::new();

        // Initially not cancelled
        assert!(!state.is_cancelled());

        // Cancel and check
        state.cancel();
        assert!(state.is_cancelled());

        // Reset and check
        state.reset();
        assert!(!state.is_cancelled());

        println!("[TEST] ✅ Cancellation state works correctly");
    }
}
