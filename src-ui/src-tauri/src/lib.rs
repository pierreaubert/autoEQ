use autoeq::{cli::Args as AutoEQArgs, LossType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use ndarray::Array1;

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
    loss: String, // "flat", "score", or "mixed"
    iir_hp_pk: bool,
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
}

#[derive(Debug, Clone, Serialize)]
struct PlotData {
    frequencies: Vec<f64>,
    curves: HashMap<String, Vec<f64>>,
    metadata: HashMap<String, serde_json::Value>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn get_speakers() -> Result<Vec<String>, String> {
    match reqwest::get("https://api.spinorama.org/v1/speakers").await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
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
                Err(e) => Err(format!("Failed to parse response: {}", e))
            }
        }
        Err(e) => Err(format!("Failed to fetch speakers: {}", e))
    }
}

#[tauri::command]
async fn get_versions(speaker: String) -> Result<Vec<String>, String> {
    let url = format!("https://api.spinorama.org/v1/speaker/{}/versions", urlencoding::encode(&speaker));
    match reqwest::get(&url).await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
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
                Err(e) => Err(format!("Failed to parse response: {}", e))
            }
        }
        Err(e) => Err(format!("Failed to fetch versions: {}", e))
    }
}

#[tauri::command]
async fn get_measurements(speaker: String, version: String) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements", 
        urlencoding::encode(&speaker),
        urlencoding::encode(&version)
    );
    match reqwest::get(&url).await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
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
                Err(e) => Err(format!("Failed to parse response: {}", e))
            }
        }
        Err(e) => Err(format!("Failed to fetch measurements: {}", e))
    }
}

#[tauri::command]
async fn run_optimization(params: OptimizationParams) -> OptimizationResult {
    let result = run_optimization_internal(params).await;
    match result {
        Ok(res) => res,
        Err(e) => OptimizationResult {
            success: false,
            error_message: Some(e.to_string()),
            filter_params: None,
            objective_value: None,
            preference_score_before: None,
            preference_score_after: None,
            filter_response: None,
            spin_details: None,
        },
    }
}

async fn run_optimization_internal(params: OptimizationParams) -> Result<OptimizationResult, Box<dyn std::error::Error + Send + Sync>> {
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
            "flat" => LossType::Flat,
            "score" => LossType::Score,
            "mixed" => LossType::Mixed,
            _ => LossType::Flat,
        },
        iir_hp_pk: params.iir_hp_pk,
        algo_list: false, // UI doesn't need to list algorithms
        tolerance: 1e-3, // Default DE tolerance
        atolerance: 1e-4, // Default DE absolute tolerance
        recombination: 0.9, // Default DE recombination probability
        strategy: "currenttobest1bin".to_string(), // Default DE strategy
        strategy_list: false, // UI doesn't need to list strategies
        adaptive_weight_f: 0.9, // Default adaptive weight for F
        adaptive_weight_cr: 0.9, // Default adaptive weight for CR
    };

    // Load input curve
    let (input_curve, spin_data) = autoeq::workflow::load_input_curve(&args).await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { 
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;
    
    // Build target curve
    let (inverted_curve, smoothed_curve) = autoeq::workflow::build_target_curve(&args, &input_curve);
    let target_curve = smoothed_curve.as_ref().unwrap_or(&inverted_curve);
    
    // Setup objective data
    let (objective_data, use_cea) = autoeq::workflow::setup_objective_data(&args, &input_curve, target_curve, &spin_data);

    // Get preference score before optimization if applicable
    let mut pref_score_before: Option<f64> = None;
    if use_cea {
        if let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(&input_curve.freq, spin_data.as_ref().unwrap(), None).await {
            pref_score_before = Some(metrics.pref_score);
        }
    }

    // Run optimization
    let filter_params = autoeq::workflow::perform_optimization(&args, &objective_data)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { 
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        })?;

    // Calculate preference score after optimization
    let mut pref_score_after: Option<f64> = None;
    if use_cea {
        let peq_response = autoeq::iir::compute_peq_response(&input_curve.freq, &filter_params, args.sample_rate, args.iir_hp_pk);
        if let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(&input_curve.freq, spin_data.as_ref().unwrap(), Some(&peq_response)).await {
            pref_score_after = Some(metrics.pref_score);
        }
    }

    // Generate plot data
    let plot_freqs: Vec<f64> = (0..200).map(|i| 20.0 * (1.0355_f64.powf(i as f64))).collect();
    let plot_freqs_array = Array1::from(plot_freqs.clone());
    
    // Generate filter response data
    let eq_response = autoeq::iir::compute_peq_response(&plot_freqs_array, &filter_params, args.sample_rate, args.iir_hp_pk);
    
    let mut filter_curves = HashMap::new();
    filter_curves.insert("EQ Response".to_string(), eq_response.to_vec());
    filter_curves.insert("Target".to_string(), 
        autoeq::read::interpolate(&plot_freqs_array, &input_curve.freq, target_curve).to_vec());
    
    let filter_response = PlotData {
        frequencies: plot_freqs.clone(),
        curves: filter_curves,
        metadata: HashMap::new(),
    };

    // Generate spin details data if available
    let mut spin_details = None;
    if let Some(ref spin) = spin_data {
        let mut spin_curves = HashMap::new();
        for (name, curve) in spin {
            spin_curves.insert(name.clone(), 
                autoeq::read::interpolate(&plot_freqs_array, &curve.freq, &curve.spl).to_vec());
        }
        spin_details = Some(PlotData {
            frequencies: plot_freqs,
            curves: spin_curves,
            metadata: HashMap::new(),
        });
    }

    Ok(OptimizationResult {
        success: true,
        error_message: None,
        filter_params: Some(filter_params),
        objective_value: None, // We could calculate this if needed
        preference_score_before: pref_score_before,
        preference_score_after: pref_score_after,
        filter_response: Some(filter_response),
        spin_details,
    })
}

#[tauri::command]
fn exit_app(window: tauri::Window) {
    window.close().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            run_optimization, 
            get_speakers, 
            get_versions, 
            get_measurements,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
