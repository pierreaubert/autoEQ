use ndarray::Array1;
use crate::{DEConfig, DEReport, OptimizationRecorder};
use crate::differential_evolution::differential_evolution;
use std::sync::Arc;

/// Helper function for running differential evolution with recording
pub fn run_recorded_differential_evolution<F>(
    function_name: &str,
    func: F,
    bounds: &[(f64, f64)],
    config: DEConfig,
    output_dir: &str,
) -> Result<(DEReport, String), Box<dyn std::error::Error>>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    // Create the recorder
    let recorder = Arc::new(OptimizationRecorder::with_output_dir(
        function_name.to_string(),
        output_dir.to_string(),
    ));

    // Create a wrapped function that records evaluations
    let func_with_recording = {
        let recorder = recorder.clone();
        move |x: &Array1<f64>| -> f64 {
            let f_value = func(x);
            recorder.record_evaluation(x, f_value);
            f_value
        }
    };

    // Run the optimization
    let result = differential_evolution(&func_with_recording, bounds, config);

    // Finalize and save any remaining records
    let saved_files = recorder.finalize()?;
    let csv_path = saved_files.first()
        .map(|s| s.clone())
        .unwrap_or_else(|| format!("{}/{}_no_data.csv", output_dir, function_name));

    Ok((result, csv_path))
}
