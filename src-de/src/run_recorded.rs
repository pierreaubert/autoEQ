//! Recording wrapper for differential evolution for testing purposes

use std::fs::create_dir_all;
use ndarray::Array1;
use crate::{differential_evolution, DEConfig, DEReport};
use crate::recorder::OptimizationRecorder;

/// Run differential evolution with recording of evaluations to CSV
///
/// This wrapper function is primarily used for testing and analysis.
/// It records every function evaluation to CSV files for later analysis.
pub fn run_recorded_differential_evolution<F>(
    function_name: &str,
    func: F,
    bounds: &[(f64, f64)],
    config: DEConfig,
    output_dir: &str,
) -> Result<(DEReport, String), Box<dyn std::error::Error>>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync + 'static,
{
    // Create output directory if it doesn't exist
    create_dir_all(output_dir)?;
    
    // Create recorder for this optimization run
    let recorder = std::sync::Arc::new(OptimizationRecorder::with_output_dir(
        function_name.to_string(),
        output_dir.to_string(),
    ));
    
    // Create wrapped objective function that records evaluations
    let recorder_clone = recorder.clone();
    let recorded_func = move |x: &Array1<f64>| -> f64 {
        let f_value = func(x);
        recorder_clone.record_evaluation(x, f_value);
        f_value
    };
    
    // Run differential evolution with the wrapped function
    let result = differential_evolution(&recorded_func, bounds, config);
    
    // Finalize recording and get CSV file paths
    let csv_files = recorder.finalize()?;
    
    // Return the DE result and the primary CSV file path
    let csv_path = if !csv_files.is_empty() {
        csv_files[0].clone()
    } else {
        format!("{}/{}.csv", output_dir, function_name)
    };
    
    Ok((result, csv_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEConfigBuilder;
    
    #[test]
    fn test_run_recorded_basic() {
        // Simple quadratic function for testing
        let quadratic = |x: &Array1<f64>| -> f64 {
            x.iter().map(|&xi| xi * xi).sum()
        };
        
        let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(20)
            .popsize(10)
            .build();
        
        let result = run_recorded_differential_evolution(
            "test_quadratic",
            quadratic,
            &bounds,
            config,
            "./data_generated/test_records",
        );
        
        assert!(result.is_ok());
        let (report, csv_path) = result.unwrap();
        
        // Should find minimum near origin
        println!("Result: f = {:.6e}, x = {:?}", report.fun, report.x);
        assert!(report.fun < 1e-3, "Function value too high: {}", report.fun);
        for &xi in report.x.iter() {
            assert!(xi.abs() < 1e-1, "Variable too far from 0: {}", xi);
        }
        
        // CSV file should be created
        println!("CSV saved to: {}", csv_path);
    }
}
