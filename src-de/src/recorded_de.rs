use ndarray::Array1;
use crate::{DEConfig, DEReport, OptimizationRecorder};
use crate::differential_evolution::differential_evolution;

/// Helper function for running differential evolution with recording
pub fn run_recorded_differential_evolution<F>(
    function_name: &str,
    func: F,
    bounds: &[(f64, f64)],
    mut config: DEConfig,
    output_dir: &str,
) -> Result<(DEReport, String), Box<dyn std::error::Error>>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    // Create the recorder
    let recorder = OptimizationRecorder::new(function_name.to_string());

    // Set up the callback to record progress
    config.callback = Some(recorder.create_callback());

    // Run the optimization
    let result = differential_evolution(&func, bounds, config);

    // Save the recording to CSV
    let csv_path = recorder.save_to_csv(output_dir)?;

    Ok((result, csv_path))
}
