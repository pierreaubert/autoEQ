use autoeq::{
    Curve, LossType, cli::Args as AutoEQArgs, plot_filters, plot_spin, plot_spin_details,
    plot_spin_tonal,
};
use ndarray::Array1;
use plotly::Plot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_mocks;

// Global cancellation state
pub struct CancellationState {
    pub cancelled: AtomicBool,
}

impl Clone for CancellationState {
    fn clone(&self) -> Self {
        Self {
            cancelled: AtomicBool::new(self.cancelled.load(Ordering::Relaxed)),
        }
    }
}

impl Default for CancellationState {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationState {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OptimizationParams {
    num_filters: usize,
    curve_path: Option<String>,
    target_path: Option<String>,
    sample_rate: f64,
    max_db: f64,
    min_db: f64,
    max_q: f64,
    min_q: f64,
    min_freq: f64,
    max_freq: f64,
    speaker: Option<String>,
    version: Option<String>,
    measurement: Option<String>,
    curve_name: String,
    algo: String,
    population: usize,
    maxeval: usize,
    refine: bool,
    local_algo: String,
    min_spacing_oct: f64,
    spacing_weight: f64,
    smooth: bool,
    smooth_n: usize,
    loss: String,              // "flat", "score", or "mixed"
    peq_model: Option<String>, // New PEQ model system: "pk", "hp-pk", "hp-pk-lp", etc.
    // DE-specific parameters
    strategy: Option<String>,
    de_f: Option<f64>,
    de_cr: Option<f64>,
    adaptive_weight_f: Option<f64>,
    adaptive_weight_cr: Option<f64>,
    // Tolerance parameters
    tolerance: Option<f64>,
    atolerance: Option<f64>,
    // Captured/Target curve data (alternative to file paths)
    captured_frequencies: Option<Vec<f64>>,
    captured_magnitudes: Option<Vec<f64>>,
    target_frequencies: Option<Vec<f64>>,
    target_magnitudes: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize)]
struct OptimizationResult {
    success: bool,
    error_message: Option<String>,
    filter_params: Option<Vec<f64>>,
    objective_value: Option<f64>,
    preference_score_before: Option<f64>,
    preference_score_after: Option<f64>,
    filter_response: Option<PlotData>,
    spin_details: Option<PlotData>,
    filter_plots: Option<PlotData>, // Individual filter responses and sum
    input_curve: Option<PlotData>,  // Original normalized input curve
    deviation_curve: Option<PlotData>, // Target - Input (what needs to be corrected)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProgressUpdate {
    iteration: usize,
    fitness: f64,
    params: Vec<f64>,
    convergence: f64,
}

#[derive(Debug, Clone, Serialize)]
struct PlotData {
    frequencies: Vec<f64>,
    curves: HashMap<String, Vec<f64>>,
    metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlotFiltersParams {
    input_curve: CurveData,
    target_curve: CurveData,
    deviation_curve: CurveData,
    optimized_params: Vec<f64>,
    sample_rate: f64,
    num_filters: usize,
    peq_model: Option<String>, // "pk", "hp-pk", etc.
}

#[derive(Debug, Clone, Deserialize)]
struct PlotSpinParams {
    cea2034_curves: Option<HashMap<String, CurveData>>,
    eq_response: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CurveData {
    freq: Vec<f64>,
    spl: Vec<f64>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_speakers() -> Result<Vec<String>, String> {
    match reqwest::get("https://api.spinorama.org/v1/speakers").await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(speakers) = data.as_array() {
                    let speaker_names: Vec<String> = speakers
                        .iter()
                        .filter_map(|s| s.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    Ok(speaker_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch speakers: {}", e)),
    }
}

#[tauri::command]
async fn get_speaker_versions(speaker: String) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/versions",
        urlencoding::encode(&speaker)
    );
    match reqwest::get(&url).await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(versions) = data.as_array() {
                    let version_names: Vec<String> = versions
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|v| v.to_string())
                        .collect();
                    Ok(version_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch versions: {}", e)),
    }
}

#[tauri::command]
async fn get_speaker_measurements(speaker: String, version: String) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements",
        urlencoding::encode(&speaker),
        urlencoding::encode(&version)
    );
    match reqwest::get(&url).await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(measurements) = data.as_array() {
                    let measurement_names: Vec<String> = measurements
                        .iter()
                        .filter_map(|m| m.as_str())
                        .map(|m| m.to_string())
                        .collect();
                    Ok(measurement_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch measurements: {}", e)),
    }
}

fn validate_params(
    params: &OptimizationParams,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate number of filters
    if params.num_filters == 0 {
        return Err("Number of filters must be at least 1".into());
    }
    if params.num_filters > 50 {
        return Err(format!(
            "Number of filters must be between 1 and 50 (got: {})",
            params.num_filters
        )
        .into());
    }

    // Validate tolerance (lower bound 1e-12, no upper bound)
    if let Some(tol) = params.tolerance
        && tol < 1e-12
    {
        return Err(format!("Tolerance must be >= 1e-12 (got: {})", tol).into());
    }

    // Validate absolute tolerance (lower bound 0, no upper bound)
    if let Some(atol) = params.atolerance
        && atol < 1e-15
    {
        return Err(format!("Absolute tolerance must be >= 1e-15 (got: {})", atol).into());
    }

    // Validate frequency range
    if params.min_freq >= params.max_freq {
        return Err(format!(
            "Minimum frequency ({} Hz) must be less than maximum frequency ({} Hz)",
            params.min_freq, params.max_freq
        )
        .into());
    }
    if params.min_freq < 20.0 {
        return Err(format!(
            "Minimum frequency must be >= 20 Hz (got: {} Hz)",
            params.min_freq
        )
        .into());
    }
    if params.max_freq > 20000.0 {
        return Err(format!(
            "Maximum frequency must be <= 20,000 Hz (got: {} Hz)",
            params.max_freq
        )
        .into());
    }

    // Validate Q range
    if params.min_q >= params.max_q {
        return Err(format!(
            "Minimum Q ({}) must be less than maximum Q ({})",
            params.min_q, params.max_q
        )
        .into());
    }
    if params.min_q < 0.1 {
        return Err(format!("Minimum Q must be >= 0.1 (got: {})", params.min_q).into());
    }
    if params.max_q > 20.0 {
        return Err(format!("Maximum Q must be <= 100 (got: {})", params.max_q).into());
    }

    // Validate dB range
    if params.min_db > params.max_db {
        return Err(format!(
            "Minimum dB ({}) must be <= maximum dB ({})",
            params.min_db, params.max_db
        )
        .into());
    }
    if params.min_db < 0.25 {
        return Err(format!("Minimum dB must be >= 0.25 (got: {})", params.min_db).into());
    }
    if params.max_db > 20.0 {
        return Err(format!("Maximum dB must be <= 20 (got: {})", params.max_db).into());
    }

    // Validate sample rate
    if params.sample_rate < 8000.0 || params.sample_rate > 192000.0 {
        return Err(format!(
            "Sample rate must be between 8,000 and 192,000 Hz (got: {} Hz)",
            params.sample_rate
        )
        .into());
    }

    // Validate population size
    if params.population == 0 {
        return Err("Population size must be at least 1".into());
    }
    if params.population > 10000 {
        return Err(format!(
            "Population size must be between 1 and 10,000 (got: {})",
            params.population
        )
        .into());
    }

    // Validate max evaluations
    if params.maxeval == 0 {
        return Err("Maximum evaluations must be at least 1".into());
    }

    // Validate smoothing N
    if params.smooth_n < 1 || params.smooth_n > 24 {
        return Err(format!(
            "Smoothing N must be between 1 and 24 (got: {})",
            params.smooth_n
        )
        .into());
    }

    // Validate DE parameters if present
    if let Some(de_f) = params.de_f
        && (!(0.0..=2.0).contains(&de_f))
    {
        return Err(format!(
            "Mutation factor (F) must be between 0 and 2 (got: {})",
            de_f
        )
        .into());
    }

    if let Some(de_cr) = params.de_cr
        && (!(0.0..=1.0).contains(&de_cr))
    {
        return Err(format!(
            "Recombination probability (CR) must be between 0 and 1 (got: {})",
            de_cr
        )
        .into());
    }

    // Validate adaptive weights
    if let Some(w) = params.adaptive_weight_f
        && (!(0.0..=1.0).contains(&w))
    {
        return Err(format!("Adaptive weight F must be between 0 and 1 (got: {})", w).into());
    }

    if let Some(w) = params.adaptive_weight_cr
        && (!(0.0..=1.0).contains(&w))
    {
        return Err(format!("Adaptive weight CR must be between 0 and 1 (got: {})", w).into());
    }

    Ok(())
}

#[tauri::command]
async fn run_optimization(
    params: OptimizationParams,
    app_handle: AppHandle,
    cancellation_state: State<'_, CancellationState>,
) -> Result<OptimizationResult, String> {
    println!(
        "[RUST DEBUG] run_optimization called with algo: {}",
        params.algo
    );
    println!(
        "[RUST DEBUG] Parameters: num_filters={}, population={}, maxeval={}",
        params.num_filters, params.population, params.maxeval
    );

    // Reset cancellation state at the start of optimization
    cancellation_state.reset();

    let result =
        run_optimization_internal(params, app_handle, Arc::new((*cancellation_state).clone()))
            .await;
    match result {
        Ok(res) => {
            println!("[RUST DEBUG] Optimization completed successfully");
            Ok(res)
        }
        Err(e) => {
            println!("[RUST DEBUG] Optimization failed with error: {}", e);
            Ok(OptimizationResult {
                success: false,
                error_message: Some(e.to_string()),
                filter_params: None,
                objective_value: None,
                preference_score_before: None,
                preference_score_after: None,
                filter_response: None,
                spin_details: None,
                filter_plots: None,
                input_curve: None,
                deviation_curve: None,
            })
        }
    }
}

async fn run_optimization_internal(
    params: OptimizationParams,
    app_handle: AppHandle,
    cancellation_state: Arc<CancellationState>,
) -> Result<OptimizationResult, Box<dyn std::error::Error + Send + Sync>> {
    println!("[RUST DEBUG] run_optimization_internal started");

    // Check for cancellation at start
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled before start".into());
    }

    // Validate parameters first
    println!("[RUST DEBUG] Validating parameters...");
    validate_params(&params)?;
    println!("[RUST DEBUG] Parameters validated successfully");

    // Convert parameters to AutoEQ Args structure
    let args = AutoEQArgs {
        num_filters: params.num_filters,
        curve: params.curve_path.map(PathBuf::from),
        target: params.target_path.map(PathBuf::from),
        sample_rate: params.sample_rate,
        max_db: params.max_db,
        min_db: params.min_db,
        max_q: params.max_q,
        min_q: params.min_q,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        output: None, // We'll handle plotting in the frontend
        speaker: params.speaker,
        version: params.version,
        measurement: params.measurement,
        curve_name: params.curve_name,
        algo: params.algo,
        population: params.population,
        maxeval: params.maxeval,
        refine: params.refine,
        local_algo: params.local_algo,
        min_spacing_oct: params.min_spacing_oct,
        spacing_weight: params.spacing_weight,
        smooth: params.smooth,
        smooth_n: params.smooth_n,
        loss: match params.loss.as_str() {
            "flat" => LossType::SpeakerFlat,
            "score" => LossType::SpeakerScore,
            "speaker-flat" => LossType::SpeakerFlat,
            "speaker-score" => LossType::SpeakerScore,
            "headphone-flat" => LossType::HeadphoneFlat,
            "headphone-score" => LossType::HeadphoneScore,
            _ => LossType::SpeakerFlat,
        },
        peq_model: match params.peq_model.as_deref() {
            Some("hp-pk") => autoeq::cli::PeqModel::HpPk,
            Some("hp-pk-lp") => autoeq::cli::PeqModel::HpPkLp,
            Some("free-pk-free") => autoeq::cli::PeqModel::FreePkFree,
            Some("free") => autoeq::cli::PeqModel::Free,
            Some("pk") | _ => autoeq::cli::PeqModel::Pk, // Default to Pk
        },
        peq_model_list: false,
        algo_list: false, // UI doesn't need to list algorithms
        tolerance: params.tolerance.unwrap_or(1e-3), // Use provided tolerance or default
        atolerance: params.atolerance.unwrap_or(1e-4), // Use provided atolerance or default
        recombination: params.de_cr.unwrap_or(0.9), // DE crossover probability
        strategy: params
            .strategy
            .unwrap_or_else(|| "currenttobest1bin".to_string()), // DE strategy
        strategy_list: false, // UI doesn't need to list strategies
        adaptive_weight_f: params.adaptive_weight_f.unwrap_or(0.8), // Adaptive weight for F
        adaptive_weight_cr: params.adaptive_weight_cr.unwrap_or(0.7), // Adaptive weight for CR
        no_parallel: false,
        parallel_threads: 0,
    };

    // Load input data (following autoeq.rs pattern)
    println!("[RUST DEBUG] Loading input curve...");
    let (input_curve_raw, spin_data_raw) = if let (Some(captured_freqs), Some(captured_mags)) =
        (&params.captured_frequencies, &params.captured_magnitudes)
    {
        // Use captured audio data
        println!(
            "[RUST DEBUG] Using captured audio data with {} points",
            captured_freqs.len()
        );
        let captured_curve = autoeq::Curve {
            freq: Array1::from_vec(captured_freqs.clone()),
            spl: Array1::from_vec(captured_mags.clone()),
        };
        (captured_curve, None)
    } else {
        // Load from file or API
        autoeq::workflow::load_input_curve(&args).await.map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                println!("[RUST DEBUG] Failed to load input curve: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
            },
        )?
    };
    println!(
        "[RUST DEBUG] Input curve loaded successfully, {} frequency points",
        input_curve_raw.freq.len()
    );

    // Check for cancellation after data loading
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled during data loading".into());
    }

    // Resample everything to standard frequency grid
    println!("[RUST DEBUG] Creating standard frequency grid...");
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Build/Get target curve
    let target_curve = if let (Some(target_freqs), Some(target_mags)) =
        (&params.target_frequencies, &params.target_magnitudes)
    {
        // Use provided target curve data
        println!(
            "[RUST DEBUG] Using provided target curve data with {} points",
            target_freqs.len()
        );
        let target_curve_raw = autoeq::Curve {
            freq: Array1::from_vec(target_freqs.clone()),
            spl: Array1::from_vec(target_mags.clone()),
        };
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &target_curve_raw)
    } else {
        // Build target using RAW input curve (before normalization)
        println!("[RUST DEBUG] Building target curve using raw input...");
        autoeq::workflow::build_target_curve(&args, &standard_freq, &input_curve_raw)
    };

    // Normalize input curve AFTER building target
    println!("[RUST DEBUG] Normalizing and interpolating input curve...");
    let input_curve =
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &input_curve_raw);

    // Create deviation curve
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: target_curve.spl.clone() - &input_curve.spl,
    };

    // Process spin data if available (following autoeq.rs pattern)
    let spin_data = spin_data_raw.map(|spin_data| {
        println!(
            "[RUST DEBUG] Processing spin data with {} curves",
            spin_data.len()
        );
        spin_data
            .into_iter()
            .map(|(name, curve)| {
                let interpolated = autoeq::read::interpolate_log_space(&standard_freq, &curve);
                (name, interpolated)
            })
            .collect()
    });

    println!("[RUST DEBUG] Target curve and data processing completed");

    // Setup objective data
    println!("[RUST DEBUG] Setting up objective data...");
    let (objective_data, use_cea) = autoeq::workflow::setup_objective_data(
        &args,
        &input_curve,
        &target_curve,
        &deviation_curve,
        &spin_data,
    );
    println!(
        "[RUST DEBUG] Objective data setup complete, use_cea: {}",
        use_cea
    );

    // Get preference score before optimization if applicable
    let mut pref_score_before: Option<f64> = None;
    if use_cea
        && let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve.freq,
            spin_data.as_ref().unwrap(),
            None,
        )
        .await
    {
        pref_score_before = Some(metrics.pref_score);
    } else if args.loss == LossType::HeadphoneFlat || args.loss == LossType::HeadphoneScore {
        // Calculate headphone preference score using Olive et al. model
        println!("[RUST DEBUG] Calculating headphone preference score before optimization");
        let headphone_data = autoeq::loss::HeadphoneLossData::new(args.smooth, args.smooth_n);
        let loss_value =
            autoeq::loss::headphone_loss_with_target(&headphone_data, &input_curve, &target_curve);
        // Negate the loss value to convert to preference score (higher is better)
        pref_score_before = Some(-loss_value);
        println!(
            "[RUST DEBUG] Headphone preference score before: {:.2}",
            pref_score_before.unwrap()
        );
    }

    // Check for cancellation before optimization
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled before starting".into());
    }

    // Run optimization with progress reporting for autoeq:de
    println!(
        "[RUST DEBUG] Starting optimization with algorithm: {}",
        args.algo
    );
    let filter_params = if args.algo == "autoeq:de" {
        println!("[RUST DEBUG] Using DE algorithm with progress reporting");
        let mut progress_count = 0;
        let cancellation_state_clone = Arc::clone(&cancellation_state);
        autoeq::workflow::perform_optimization_with_callback(
            &args,
            &objective_data,
            Box::new(move |intermediate| {
                // Check for cancellation in callback
                if cancellation_state_clone.is_cancelled() {
                    println!("[RUST DEBUG] Optimization cancelled during iteration {}", intermediate.iter);
                    return autoeq::de::CallbackAction::Stop;
                }
                progress_count += 1;
                if progress_count % 10 == 0 || progress_count <= 5 {
                    println!("[RUST DEBUG] Progress update #{}: iter={}, fitness={:.6}, convergence={:.4}",
                             progress_count, intermediate.iter, intermediate.fun, intermediate.convergence);
                }
                let emit_result = app_handle.emit(
                    "progress_update",
                    ProgressUpdate {
                        iteration: intermediate.iter,
                        fitness: intermediate.fun,
                        params: intermediate.x.to_vec(),
                        convergence: intermediate.convergence,
                    },
                );
                if let Err(e) = emit_result {
                    println!("[RUST DEBUG] Failed to emit progress update: {}", e);
                } else if progress_count % 50 == 0 {
                    println!("[RUST DEBUG] Progress event emitted successfully (count: {})", progress_count);
                }
                autoeq::de::CallbackAction::Continue
            }),
        )
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            println!("[RUST DEBUG] DE optimization failed: {}", e);
            Box::new(std::io::Error::other(e.to_string()))
        })?
    } else {
        println!("[RUST DEBUG] Using non-DE algorithm: {}", args.algo);
        autoeq::workflow::perform_optimization(&args, &objective_data).map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                println!("[RUST DEBUG] Non-DE optimization failed: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
            },
        )?
    };
    println!(
        "[RUST DEBUG] Optimization completed, got {} filter parameters",
        filter_params.len()
    );

    // Calculate preference score after optimization
    let mut pref_score_after: Option<f64> = None;
    if use_cea {
        let peq_response = autoeq::x2peq::compute_peq_response_from_x(
            &input_curve.freq,
            &filter_params,
            args.sample_rate,
            args.peq_model,
        );
        if let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve.freq,
            spin_data.as_ref().unwrap(),
            Some(&peq_response),
        )
        .await
        {
            pref_score_after = Some(metrics.pref_score);
        }
    } else if args.loss == LossType::HeadphoneFlat || args.loss == LossType::HeadphoneScore {
        // Calculate headphone preference score after applying EQ
        println!("[RUST DEBUG] Calculating headphone preference score after optimization");
        let peq_response = autoeq::x2peq::compute_peq_response_from_x(
            &input_curve.freq,
            &filter_params,
            args.sample_rate,
            args.peq_model,
        );
        // Create corrected curve by adding PEQ response to input
        let corrected_curve = autoeq::Curve {
            freq: input_curve.freq.clone(),
            spl: &input_curve.spl + &peq_response,
        };
        let headphone_data = autoeq::loss::HeadphoneLossData::new(args.smooth, args.smooth_n);
        let loss_value = autoeq::loss::headphone_loss_with_target(
            &headphone_data,
            &corrected_curve,
            &target_curve,
        );
        // Negate the loss value to convert to preference score (higher is better)
        pref_score_after = Some(-loss_value);
        println!(
            "[RUST DEBUG] Headphone preference score after: {:.2}",
            pref_score_after.unwrap()
        );
    }

    // Generate plot data
    let plot_freqs: Vec<f64> = (0..200)
        .map(|i| 20.0 * (1.0355_f64.powf(i as f64)))
        .collect();
    let plot_freqs_array = Array1::from(plot_freqs.clone());

    // Generate filter response data
    let eq_response = autoeq::x2peq::compute_peq_response_from_x(
        &plot_freqs_array,
        &filter_params,
        args.sample_rate,
        args.peq_model,
    );

    let mut filter_curves = HashMap::new();
    filter_curves.insert("EQ Response".to_string(), eq_response.to_vec());
    let target_curve_for_plot = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: target_curve.spl.clone(),
    };
    let target_interpolated = autoeq::read::interpolate(&plot_freqs_array, &target_curve_for_plot);
    filter_curves.insert("Target".to_string(), target_interpolated.spl.to_vec());

    let filter_response = PlotData {
        frequencies: plot_freqs.clone(),
        curves: filter_curves,
        metadata: HashMap::new(),
    };

    // Generate individual filter plots
    let mut individual_filter_curves = HashMap::new();
    let mut combined_response = Array1::zeros(plot_freqs_array.len());

    // Sort filters by frequency for consistent display
    let mut filters: Vec<(usize, f64, f64, f64)> = (0..args.num_filters)
        .map(|i| {
            (
                i,
                10f64.powf(filter_params[i * 3]), // Convert from log to linear frequency
                filter_params[i * 3 + 1],         // Q
                filter_params[i * 3 + 2],         // Gain
            )
        })
        .collect();
    filters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Generate response for each filter
    for (orig_i, f0, q, gain) in filters.into_iter() {
        use autoeq::iir::{Biquad, BiquadFilterType};

        let (ftype, label_prefix) = match args.peq_model {
            autoeq::cli::PeqModel::HpPk if orig_i == 0 => (BiquadFilterType::Highpass, "HPQ"),
            autoeq::cli::PeqModel::HpPkLp if orig_i == 0 => (BiquadFilterType::Highpass, "HPQ"),
            autoeq::cli::PeqModel::HpPkLp if orig_i == args.num_filters - 1 => {
                (BiquadFilterType::Lowpass, "LP")
            }
            _ => (BiquadFilterType::Peak, "PK"),
        };

        let filter = Biquad::new(ftype, f0, args.sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs_array);
        combined_response = &combined_response + &filter_response;

        let label = format!("{} {} at {:.0}Hz", label_prefix, orig_i + 1, f0);

        individual_filter_curves.insert(label, filter_response.to_vec());
    }

    // Add the combined sum
    individual_filter_curves.insert("Sum".to_string(), combined_response.to_vec());

    let filter_plots = PlotData {
        frequencies: plot_freqs.clone(),
        curves: individual_filter_curves,
        metadata: HashMap::new(),
    };

    // Generate spin details data if available
    let mut spin_details = None;
    if let Some(spin) = spin_data {
        let mut spin_curves = HashMap::new();
        for (name, curve) in spin {
            let interpolated = autoeq::read::interpolate(&plot_freqs_array, &curve);
            spin_curves.insert(name.clone(), interpolated.spl.to_vec());
        }
        spin_details = Some(PlotData {
            frequencies: plot_freqs.clone(),
            curves: spin_curves,
            metadata: HashMap::new(),
        });
    }

    // Add input curve data
    let input_curve_plot = PlotData {
        frequencies: plot_freqs.clone(),
        curves: {
            let mut curves = HashMap::new();
            let input_interpolated = autoeq::read::interpolate(&plot_freqs_array, &input_curve);
            curves.insert("Input".to_string(), input_interpolated.spl.to_vec());
            curves
        },
        metadata: HashMap::new(),
    };

    // Add deviation curve data (target - input, this is what needs to be corrected)
    let deviation_curve_plot = PlotData {
        frequencies: plot_freqs.clone(),
        curves: {
            let mut curves = HashMap::new();
            let deviation_interpolated =
                autoeq::read::interpolate(&plot_freqs_array, &deviation_curve);
            curves.insert("Deviation".to_string(), deviation_interpolated.spl.to_vec());
            curves
        },
        metadata: HashMap::new(),
    };

    Ok(OptimizationResult {
        success: true,
        error_message: None,
        filter_params: Some(filter_params),
        objective_value: None, // We could calculate this if needed
        preference_score_before: pref_score_before,
        preference_score_after: pref_score_after,
        filter_response: Some(filter_response),
        spin_details,
        filter_plots: Some(filter_plots),
        input_curve: Some(input_curve_plot),
        deviation_curve: Some(deviation_curve_plot),
    })
}

// Helper function to convert CurveData to autoeq::Curve
fn curve_data_to_curve(curve_data: &CurveData) -> Curve {
    Curve {
        freq: Array1::from_vec(curve_data.freq.clone()),
        spl: Array1::from_vec(curve_data.spl.clone()),
    }
}

// Helper function to convert plotly::Plot to JSON
fn plot_to_json(plot: Plot) -> Result<serde_json::Value, String> {
    match serde_json::to_value(plot) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Failed to serialize plot: {}", e)),
    }
}

#[tauri::command]
async fn generate_plot_filters(params: PlotFiltersParams) -> Result<serde_json::Value, String> {
    // Convert CurveData to autoeq::Curve
    let input_curve = curve_data_to_curve(&params.input_curve);
    let target_curve = curve_data_to_curve(&params.target_curve);
    let deviation_curve = curve_data_to_curve(&params.deviation_curve);

    // Create a minimal Args struct for the plot function
    let args = AutoEQArgs {
        num_filters: params.num_filters,
        curve: None,
        target: None,
        sample_rate: params.sample_rate,
        max_db: 3.0,
        min_db: 1.0,
        max_q: 3.0,
        min_q: 1.0,
        min_freq: 60.0,
        max_freq: 16000.0,
        output: None,
        speaker: None,
        version: None,
        measurement: None,
        curve_name: "Listening Window".to_string(),
        algo: "nlopt:cobyla".to_string(),
        population: 300,
        maxeval: 2000,
        refine: false,
        local_algo: "cobyla".to_string(),
        min_spacing_oct: 0.5,
        spacing_weight: 20.0,
        smooth: true,
        smooth_n: 2,
        loss: LossType::SpeakerFlat,
        peq_model: match params.peq_model.as_deref() {
            Some("hp-pk") => autoeq::cli::PeqModel::HpPk,
            Some("hp-pk-lp") => autoeq::cli::PeqModel::HpPkLp,
            Some("free-pk-free") => autoeq::cli::PeqModel::FreePkFree,
            Some("free") => autoeq::cli::PeqModel::Free,
            Some("pk") | _ => autoeq::cli::PeqModel::Pk,
        },
        peq_model_list: false,
        algo_list: false,
        tolerance: 1e-3,
        atolerance: 1e-4,
        recombination: 0.9,
        strategy: "currenttobest1bin".to_string(),
        strategy_list: false,
        adaptive_weight_f: 0.9,
        adaptive_weight_cr: 0.9,
        no_parallel: false,
        parallel_threads: 0,
    };

    // Generate the plot
    let plot = plot_filters(
        &args,
        &input_curve,
        &target_curve,
        &deviation_curve,
        &params.optimized_params,
    );

    // Convert to JSON
    plot_to_json(plot)
}

#[tauri::command]
async fn generate_plot_spin(params: PlotSpinParams) -> Result<serde_json::Value, String> {
    // Convert CurveData HashMap to autoeq::Curve HashMap if provided
    let cea2034_curves = params.cea2034_curves.as_ref().map(|curves| {
        curves
            .iter()
            .map(|(name, curve_data)| (name.clone(), curve_data_to_curve(curve_data)))
            .collect::<HashMap<String, Curve>>()
    });

    // Convert eq_response to Array1 if provided
    let eq_response = params
        .eq_response
        .as_ref()
        .map(|response| Array1::from_vec(response.clone()));

    // Generate the plot
    let plot = plot_spin(cea2034_curves.as_ref(), eq_response.as_ref());

    // Convert to JSON
    plot_to_json(plot)
}

#[tauri::command]
async fn generate_plot_spin_details(params: PlotSpinParams) -> Result<serde_json::Value, String> {
    // Convert CurveData HashMap to autoeq::Curve HashMap if provided
    let cea2034_curves = params.cea2034_curves.as_ref().map(|curves| {
        curves
            .iter()
            .map(|(name, curve_data)| (name.clone(), curve_data_to_curve(curve_data)))
            .collect::<HashMap<String, Curve>>()
    });

    // Convert eq_response to Array1 if provided
    let eq_response = params
        .eq_response
        .as_ref()
        .map(|response| Array1::from_vec(response.clone()));

    // Generate the plot
    let plot = plot_spin_details(cea2034_curves.as_ref(), eq_response.as_ref());

    // Convert to JSON
    plot_to_json(plot)
}

#[tauri::command]
async fn generate_plot_spin_tonal(params: PlotSpinParams) -> Result<serde_json::Value, String> {
    // Convert CurveData HashMap to autoeq::Curve HashMap if provided
    let cea2034_curves = params.cea2034_curves.as_ref().map(|curves| {
        curves
            .iter()
            .map(|(name, curve_data)| (name.clone(), curve_data_to_curve(curve_data)))
            .collect::<HashMap<String, Curve>>()
    });

    // Convert eq_response to Array1 if provided
    let eq_response = params
        .eq_response
        .as_ref()
        .map(|response| Array1::from_vec(response.clone()));

    // Generate the plot
    let plot = plot_spin_tonal(cea2034_curves.as_ref(), eq_response.as_ref());

    // Convert to JSON
    plot_to_json(plot)
}

#[tauri::command]
fn exit_app(window: tauri::Window) {
    window.close().unwrap();
}

#[tauri::command]
fn cancel_optimization(cancellation_state: State<CancellationState>) -> Result<(), String> {
    println!("[RUST DEBUG] Cancellation requested");
    cancellation_state.cancel();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancellationState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            run_optimization,
            cancel_optimization,
            get_speakers,
            get_speaker_versions,
            get_speaker_measurements,
            generate_plot_filters,
            generate_plot_spin,
            generate_plot_spin_details,
            generate_plot_spin_tonal,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
