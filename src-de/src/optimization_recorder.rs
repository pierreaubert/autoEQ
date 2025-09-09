use std::sync::{Arc, Mutex};
use std::fs::{create_dir_all, File};
use std::io::Write;
use crate::{DEIntermediate, CallbackAction};

/// Records optimization progress via DE callbacks
#[derive(Debug)]
pub struct OptimizationRecorder {
    /// Function name (used for CSV filename)
    function_name: String,
    /// Shared records storage
    records: Arc<Mutex<Vec<OptimizationRecord>>>,
    /// Best function value seen so far
    best_value: Arc<Mutex<Option<f64>>>,
}

/// A single optimization iteration record
#[derive(Debug, Clone)]
pub struct OptimizationRecord {
    /// Iteration number
    pub iteration: usize,
    /// Best x found so far
    pub x: Vec<f64>,
    /// Best function result so far
    pub best_result: f64,
    /// Convergence measure (standard deviation of population)
    pub convergence: f64,
    /// Whether this iteration improved the best known result
    pub is_improvement: bool,
}

impl OptimizationRecorder {
    /// Create a new optimization recorder for the given function
    pub fn new(function_name: String) -> Self {
        Self {
            function_name,
            records: Arc::new(Mutex::new(Vec::new())),
            best_value: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a callback function that records optimization progress
    pub fn create_callback(&self) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
        let records = self.records.clone();
        let best_value = self.best_value.clone();

        Box::new(move |intermediate: &DEIntermediate| -> CallbackAction {
            let mut best_guard = best_value.lock().unwrap();
            let is_improvement = match *best_guard {
                Some(best) => intermediate.fun < best,
                None => true,
            };

            if is_improvement {
                *best_guard = Some(intermediate.fun);
            }
            drop(best_guard);

            // Record the iteration
            let mut records_guard = records.lock().unwrap();
            records_guard.push(OptimizationRecord {
                iteration: intermediate.iter,
                x: intermediate.x.to_vec(),
                best_result: intermediate.fun,
                convergence: intermediate.convergence,
                is_improvement,
            });
            drop(records_guard);

            CallbackAction::Continue
        })
    }

    /// Save all recorded iterations to a CSV file
    pub fn save_to_csv(&self, output_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Create output directory if it doesn't exist
        create_dir_all(output_dir)?;

        let filename = format!("{}/{}.csv", output_dir, self.function_name);
        let mut file = File::create(&filename)?;

        let records_guard = self.records.lock().unwrap();

        if records_guard.is_empty() {
            return Ok(filename);
        }

        // Write CSV header
        let num_dimensions = records_guard[0].x.len();
        write!(file, "iteration,")?;
        for i in 0..num_dimensions {
            write!(file, "x{},", i)?;
        }
        writeln!(file, "best_result,convergence,is_improvement")?;

        // Write data rows
        for record in records_guard.iter() {
            write!(file, "{},", record.iteration)?;
            for &xi in &record.x {
                write!(file, "{:.16},", xi)?;
            }
            writeln!(
                file,
                "{:.16},{:.16},{}",
                record.best_result, record.convergence, record.is_improvement
            )?;
        }

        Ok(filename)
    }

    /// Get a copy of all recorded iterations
    pub fn get_records(&self) -> Vec<OptimizationRecord> {
        self.records.lock().unwrap().clone()
    }

    /// Get the number of iterations recorded
    pub fn num_iterations(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    /// Clear all recorded iterations
    pub fn clear(&self) {
        self.records.lock().unwrap().clear();
        *self.best_value.lock().unwrap() = None;
    }

    /// Get the final best solution if any iterations were recorded
    pub fn get_best_solution(&self) -> Option<(Vec<f64>, f64)> {
        let records_guard = self.records.lock().unwrap();
        if let Some(last_record) = records_guard.last() {
            Some((last_record.x.clone(), last_record.best_result))
        } else {
            None
        }
    }
}
