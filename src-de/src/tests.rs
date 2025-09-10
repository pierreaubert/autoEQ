#[cfg(test)]
mod tests {
    use ndarray::Array1;
    use crate::{
        OptimizationRecorder,
        run_recorded_differential_evolution, DEConfigBuilder,
    };
    use autoeq_testfunctions::quadratic;

    #[test]
    fn test_optimization_recorder() {
        let recorder = OptimizationRecorder::new("test_function".to_string());

        // Test recording evaluations directly
        let x1 = Array1::from(vec![1.0, 2.0]);
        recorder.set_generation(0);
        recorder.record_evaluation(&x1, 5.0);

        let x2 = Array1::from(vec![0.5, 1.0]);
        recorder.set_generation(1);
        recorder.record_evaluation(&x2, 1.25);

        // Check records using test method
        let records = recorder.get_test_records();
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].iteration, 0);
        assert_eq!(records[0].x, vec![1.0, 2.0]);
        assert_eq!(records[0].best_result, 5.0);
        assert!(records[0].is_improvement);

        assert_eq!(records[1].iteration, 1);
        assert_eq!(records[1].x, vec![0.5, 1.0]);
        assert_eq!(records[1].best_result, 1.25);
        assert!(records[1].is_improvement);
    }

    #[test]
    fn test_recorded_optimization() {
        // Test recording with simple quadratic function
        let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50) // Keep it short for testing
            .popsize(10)
            .build();

        let result = run_recorded_differential_evolution(
            "quadratic",
            quadratic,
            &bounds,
            config,
            "./data_generated/records",
        );

        assert!(result.is_ok());
        let (_de_report, csv_path) = result.unwrap();

        // Check that CSV file was created
        assert!(std::path::Path::new(&csv_path).exists());
        println!("CSV saved to: {}", csv_path);

        // Read and verify CSV content
        let csv_content = std::fs::read_to_string(&csv_path).expect("Failed to read CSV");
        let lines: Vec<&str> = csv_content.trim().split('\n').collect();

        // Should have header plus at least a few iterations
        assert!(lines.len() > 1, "CSV should have header plus data rows");

        // Check header format
        let header = lines[0];
        assert!(header.starts_with("eval_id,generation,x0,x1,f_value,best_so_far,is_improvement"));

        println!(
            "Recording test passed - {} iterations recorded",
            lines.len() - 1
        );

    }
}
