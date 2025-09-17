use autoeq::{LossType, cli::Args as AutoEQArgs};
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

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
    // DE-specific parameters
    strategy: Option<String>,
    de_f: Option<f64>,
    de_cr: Option<f64>,
    adaptive_weight_f: Option<f64>,
    adaptive_weight_cr: Option<f64>,
    // Tolerance parameters
    tolerance: Option<f64>,
    atolerance: Option<f64>,
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
}

#[derive(Debug, Clone, Serialize)]
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
async fn get_versions(speaker: String) -> Result<Vec<String>, String> {
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
async fn get_measurements(speaker: String, version: String) -> Result<Vec<String>, String> {
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
async fn run_optimization(params: OptimizationParams, app_handle: AppHandle) -> OptimizationResult {
    let result = run_optimization_internal(params, app_handle).await;
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
            filter_plots: None,
        },
    }
}

async fn run_optimization_internal(
    params: OptimizationParams,
    app_handle: AppHandle,
) -> Result<OptimizationResult, Box<dyn std::error::Error + Send + Sync>> {
    // Validate parameters first
    validate_params(&params)?;

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
        iir_hp_pk: params.iir_hp_pk,
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

    // Load input curve
    let (input_curve, spin_data) = autoeq::workflow::load_input_curve(&args).await.map_err(
        |e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        },
    )?;

    // Build target curve
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);
    let input_curve_normalized =
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &input_curve);
    let target_curve = autoeq::workflow::build_target_curve(&args, &standard_freq, &input_curve);
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: &target_curve.spl - &input_curve_normalized.spl,
    };

    // Setup objective data
    let (objective_data, use_cea) = autoeq::workflow::setup_objective_data(
        &args,
        &input_curve_normalized,
        &deviation_curve,
        &spin_data,
    );

    // Get preference score before optimization if applicable
    let mut pref_score_before: Option<f64> = None;
    if use_cea
        && let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve_normalized.freq,
            spin_data.as_ref().unwrap(),
            None,
        )
        .await
    {
        pref_score_before = Some(metrics.pref_score);
    }

    // Run optimization with progress reporting for autoeq:de
    let filter_params = if args.algo == "autoeq:de" {
        autoeq::workflow::perform_optimization_with_callback(
            &args,
            &objective_data,
            Box::new(move |intermediate| {
                let _ = app_handle.emit(
                    "progress_update",
                    ProgressUpdate {
                        iteration: intermediate.iter,
                        fitness: intermediate.fun,
                        params: intermediate.x.to_vec(),
                        convergence: intermediate.convergence,
                    },
                );
                autoeq::de::CallbackAction::Continue
            }),
        )
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        })?
    } else {
        autoeq::workflow::perform_optimization(&args, &objective_data).map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::other(e.to_string()))
            },
        )?
    };

    // Calculate preference score after optimization
    let mut pref_score_after: Option<f64> = None;
    if use_cea {
        let peq_response = autoeq::iir::compute_peq_response(
            &input_curve_normalized.freq,
            &filter_params,
            args.sample_rate,
            args.iir_hp_pk,
        );
        if let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve_normalized.freq,
            spin_data.as_ref().unwrap(),
            Some(&peq_response),
        )
        .await
        {
            pref_score_after = Some(metrics.pref_score);
        }
    }

    // Generate plot data
    let plot_freqs: Vec<f64> = (0..200)
        .map(|i| 20.0 * (1.0355_f64.powf(i as f64)))
        .collect();
    let plot_freqs_array = Array1::from(plot_freqs.clone());

    // Generate filter response data
    let eq_response = autoeq::iir::compute_peq_response(
        &plot_freqs_array,
        &filter_params,
        args.sample_rate,
        args.iir_hp_pk,
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

        let ftype = if args.iir_hp_pk && orig_i == 0 {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };

        let filter = Biquad::new(ftype, f0, args.sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs_array);
        combined_response = &combined_response + &filter_response;

        let label = if args.iir_hp_pk && orig_i == 0 {
            format!("HPQ {} at {:.0}Hz", orig_i + 1, f0)
        } else {
            format!("PK {} at {:.0}Hz", orig_i + 1, f0)
        };

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
    if let Some(ref spin) = spin_data {
        let mut spin_curves = HashMap::new();
        for (name, curve) in spin {
            let interpolated = autoeq::read::interpolate(&plot_freqs_array, curve);
            spin_curves.insert(name.clone(), interpolated.spl.to_vec());
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
        filter_plots: Some(filter_plots),
    })
}

#[tauri::command]
fn exit_app(window: tauri::Window) {
    window.close().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {}))
        .plugin(tauri_plugin_clipboard_manager::init())
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
