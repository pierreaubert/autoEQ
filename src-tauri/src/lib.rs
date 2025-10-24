use autoeq::{
    Curve, LossType, cli::Args as AutoEQArgs, plot_filters, plot_spin, plot_spin_details,
    plot_spin_tonal,
};
use ndarray::Array1;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// Import from sotf_backend
use sotf_backend::camilla::ChannelMapMode;
use sotf_backend::optim::{ProgressCallback, ProgressUpdate, run_optimization_internal};
use sotf_backend::plot::{PlotFiltersParams, PlotSpinParams, plot_to_json};
use sotf_backend::{
    AudioManager, CancellationState, OptimizationParams, OptimizationResult, SharedAudioState,
    audio, curve_data_to_curve,
};
use tokio::sync::Mutex;

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

// Tauri-specific ProgressCallback implementation
struct TauriProgressCallback {
    app_handle: AppHandle,
}

impl ProgressCallback for TauriProgressCallback {
    fn on_progress(&self, update: ProgressUpdate) -> bool {
        // Emit progress update to frontend
        match self.app_handle.emit("progress_update", &update) {
            Ok(_) => true, // Continue optimization
            Err(e) => {
                eprintln!("Failed to emit progress update: {}", e);
                true // Still continue even if emit fails
            }
        }
    }
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

    // Create progress callback
    let progress_callback = Arc::new(TauriProgressCallback { app_handle });

    let result = run_optimization_internal(
        params,
        progress_callback,
        Arc::new((*cancellation_state).clone()),
    )
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
        seed: None, // Random seed for deterministic optimization (None = random)
        qa: None,   // Quality assurance mode disabled for UI (None = disabled)
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

// ============================================================================
// Audio Control Commands
// ============================================================================

use sotf_backend::audio::{AudioConfig, AudioDevice};
use sotf_backend::{AudioState, AudioStreamState, FilterParams};
use std::path::PathBuf;

// ============================================================================
// Audio Event Payloads
// ============================================================================

/// Audio state change event payload
#[derive(Clone, serde::Serialize)]
struct AudioStateChanged {
    state: String,
    file: Option<String>,
    output_device: Option<String>,
    input_device: Option<String>,
}

/// Audio position update event payload
#[derive(Clone, serde::Serialize)]
struct AudioPositionUpdate {
    position_seconds: f64,
    duration_seconds: Option<f64>,
}

/// Audio error event payload
#[derive(Clone, serde::Serialize)]
struct AudioError {
    error: String,
}

/// Audio signal peak event payload (for VU meter)
#[derive(Clone, serde::Serialize)]
struct AudioSignalPeak {
    peak: f32,
}

/// Convert AudioState enum to string for events
fn audio_state_to_string(state: AudioState) -> String {
    match state {
        AudioState::Idle => "idle".to_string(),
        AudioState::Playing => "playing".to_string(),
        AudioState::Paused => "paused".to_string(),
        AudioState::Recording => "recording".to_string(),
        AudioState::Error => "error".to_string(),
    }
}

#[tauri::command]
async fn audio_start_playback(
    file_path: String,
    output_device: Option<String>,
    sample_rate: u32,
    channels: u16,
    filters: Vec<FilterParams>,
    audio_manager: State<'_, Mutex<AudioManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!(
        "[AUDIO] Starting playback: {} ({}Hz, {}ch, {} filters)",
        file_path,
        sample_rate,
        channels,
        filters.len()
    );

    let manager = audio_manager.lock().await;
    let result = manager
        .start_playback(
            PathBuf::from(&file_path),
            output_device.clone(),
            sample_rate,
            channels,
            filters,
            ChannelMapMode::Normal,
            None,
        )
        .await;

    match result {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "audio:state-changed",
                AudioStateChanged {
                    state: "playing".to_string(),
                    file: Some(file_path),
                    output_device,
                    input_device: None,
                },
            );
            Ok(())
        }
        Err(e) => {
            // Emit error event
            let _ = app_handle.emit(
                "audio:error",
                AudioError {
                    error: e.to_string(),
                },
            );
            Err(format!("{}", e))
        }
    }
}

#[tauri::command]
async fn audio_stop_playback(
    audio_manager: State<'_, Mutex<AudioManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[AUDIO] Stopping playback");

    let manager = audio_manager.lock().await;
    let result = manager.stop_playback().await;

    match result {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "audio:state-changed",
                AudioStateChanged {
                    state: "idle".to_string(),
                    file: None,
                    output_device: None,
                    input_device: None,
                },
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
async fn audio_update_filters(
    filters: Vec<FilterParams>,
    audio_manager: State<'_, Mutex<AudioManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[AUDIO] Updating {} filters", filters.len());

    let manager = audio_manager.lock().await;
    let result = manager.update_filters(filters).await;

    match result {
        Ok(_) => {
            // Optionally emit event to confirm filters updated
            println!("[AUDIO] Filters updated successfully");
            Ok(())
        }
        Err(e) => {
            // Emit error event
            let _ = app_handle.emit(
                "audio:error",
                AudioError {
                    error: e.to_string(),
                },
            );
            Err(format!("{}", e))
        }
    }
}

#[tauri::command]
async fn audio_get_state(
    audio_manager: State<'_, Mutex<AudioManager>>,
) -> Result<AudioStreamState, String> {
    let manager = audio_manager.lock().await;
    manager.get_state().map_err(|e| format!("{}", e))
}

#[tauri::command]
async fn audio_start_recording(
    output_path: String,
    input_device: Option<String>,
    sample_rate: u32,
    channels: u16,
    audio_manager: State<'_, Mutex<AudioManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!(
        "[AUDIO] Starting recording: {} ({}Hz, {}ch)",
        output_path, sample_rate, channels
    );

    let manager = audio_manager.lock().await;
    let result = manager
        .start_recording(
            PathBuf::from(&output_path),
            input_device.clone(),
            sample_rate,
            channels,
            None,
        )
        .await;

    match result {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "audio:state-changed",
                AudioStateChanged {
                    state: "recording".to_string(),
                    file: Some(output_path),
                    output_device: None,
                    input_device,
                },
            );
            Ok(())
        }
        Err(e) => {
            // Emit error event
            let _ = app_handle.emit(
                "audio:error",
                AudioError {
                    error: e.to_string(),
                },
            );
            Err(format!("{}", e))
        }
    }
}

#[tauri::command]
async fn audio_stop_recording(
    audio_manager: State<'_, Mutex<AudioManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[AUDIO] Stopping recording");

    let manager = audio_manager.lock().await;
    let result = manager.stop_recording().await;

    match result {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "audio:state-changed",
                AudioStateChanged {
                    state: "idle".to_string(),
                    file: None,
                    output_device: None,
                    input_device: None,
                },
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
async fn audio_get_signal_peak(
    audio_manager: State<'_, Mutex<AudioManager>>,
) -> Result<f32, String> {
    let manager = audio_manager.lock().await;
    manager
        .get_signal_peak()
        .await
        .map_err(|e| format!("{}", e))
}

// ============================================================================
// Audio Device Management Commands (Tauri wrappers for backend functions)
// ============================================================================

#[tauri::command]
async fn get_audio_devices() -> Result<std::collections::HashMap<String, Vec<AudioDevice>>, String>
{
    sotf_backend::audio::get_audio_devices()
}

#[tauri::command]
async fn set_audio_device(
    device_name: String,
    is_input: bool,
    config: AudioConfig,
    audio_state: State<'_, SharedAudioState>,
) -> Result<String, String> {
    sotf_backend::audio::set_audio_device(device_name, is_input, config, &*audio_state)
}

#[tauri::command]
async fn get_audio_config(
    audio_state: State<'_, SharedAudioState>,
) -> Result<sotf_backend::audio::AudioState, String> {
    sotf_backend::audio::get_audio_config(&*audio_state)
}

#[tauri::command]
async fn get_device_properties(
    device_name: String,
    is_input: bool,
) -> Result<serde_json::Value, String> {
    sotf_backend::audio::get_device_properties(device_name, is_input)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Find CamillaDSP binary
    let camilla_binary = sotf_backend::camilla::find_camilladsp_binary().unwrap_or_else(|e| {
        eprintln!("Warning: CamillaDSP binary not found: {}", e);
        eprintln!("Audio playback features will not be available.");
        std::path::PathBuf::from("/usr/local/bin/camilladsp")
    });

    // Create AudioManager (wrapped in Mutex for Tauri state)
    let audio_manager = Mutex::new(AudioManager::new(camilla_binary));

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancellationState::new())
        .manage(SharedAudioState::default())
        .manage(audio_manager)
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
            exit_app,
            get_audio_devices,
            set_audio_device,
            get_audio_config,
            get_device_properties,
            audio_start_playback,
            audio_stop_playback,
            audio_update_filters,
            audio_get_state,
            audio_start_recording,
            audio_stop_recording,
            audio_get_signal_peak
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
