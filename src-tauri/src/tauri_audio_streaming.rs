// ============================================================================
// Streaming Audio Commands
// ============================================================================

use crate::tauri_audio_recording::AudioError;
use sotf_audio::{AudioFileInfo, AudioStreamingManager, LoudnessCompensation, PluginConfig, StreamingState};
use autoeq::iir::Biquad;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

/// Audio file information for the frontend
#[derive(Clone, serde::Serialize)]
pub struct AudioFileInfoPayload {
    path: String,
    format: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    duration_seconds: Option<f64>,
}

fn convert_audio_file_info(info: &AudioFileInfo) -> AudioFileInfoPayload {
    AudioFileInfoPayload {
        path: info.path.to_string_lossy().to_string(),
        format: info.format.to_string(),
        sample_rate: info.spec.sample_rate,
        channels: info.spec.channels,
        bits_per_sample: info.spec.bits_per_sample,
        duration_seconds: info.duration_seconds,
    }
}

#[tauri::command]
pub async fn stream_load_file(
    file_path: String,
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<AudioFileInfoPayload, String> {
    println!("[TAURI] Loading file for streaming: {}", file_path);

    let mut manager = streaming_manager.lock().await;
    match manager.load_file(&file_path).await {
        Ok(audio_info) => {
            let payload = convert_audio_file_info(&audio_info);

            // Emit file loaded event
            let _ = app_handle.emit("stream:file-loaded", &payload);

            Ok(payload)
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:error",
                AudioError {
                    error: error_msg.clone(),
                },
            );
            Err(error_msg)
        }
    }
}

/// Helper to create EQ plugin config from biquad filters
fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use autoeq::iir::BiquadFilterType;

    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            // Use long_name() from BiquadFilterType
            let filter_type = match f.filter_type {
                BiquadFilterType::HighpassVariableQ => "highpass".to_string(),
                _ => f.filter_type.long_name().to_lowercase(),
            };

            Ok(serde_json::json!({
                "filter_type": filter_type,
                "freq": f.freq,
                "q": f.q,
                "db_gain": f.db_gain,
            }))
        })
        .collect();

    let filter_configs = filter_configs?;

    let parameters = serde_json::json!({
        "filters": filter_configs,
    });

    Ok(PluginConfig {
        plugin_type: "eq".to_string(),
        parameters,
    })
}

#[tauri::command]
pub async fn stream_start_playback(
    output_device: Option<String>,
    filters: Vec<Biquad>,
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Starting playback with {} filters", filters.len());

    // Build plugin chain
    let mut plugins = Vec::new();

    // Add EQ plugin if filters are present
    if !filters.is_empty() {
        plugins.push(create_eq_plugin_config(&filters)?);
    }

    // Get audio info to determine output channels
    let manager = streaming_manager.lock().await;
    let audio_info = manager
        .get_audio_info()
        .ok_or_else(|| "No audio file loaded".to_string())?;
    let output_channels = audio_info.spec.channels as usize;
    drop(manager);

    // Start playback
    let mut manager = streaming_manager.lock().await;
    match manager
        .start_playback(output_device.clone(), plugins, output_channels)
        .await
    {
        Ok(_) => {
            // Emit state change event
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "playing",
                    "output_device": output_device,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:error",
                AudioError {
                    error: error_msg.clone(),
                },
            );
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub async fn stream_pause_playback(
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Pausing playback");

    let manager = streaming_manager.lock().await;
    match manager.pause().await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "paused",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_resume_playback(
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Resuming playback");

    let manager = streaming_manager.lock().await;
    match manager.resume().await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "playing",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_stop_playback(
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Stopping playback");

    let mut manager = streaming_manager.lock().await;
    match manager.stop().await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:state-changed",
                serde_json::json!({
                    "state": "idle",
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_seek(
    seconds: f64,
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Seeking to {}s", seconds);

    let manager = streaming_manager.lock().await;
    match manager.seek(seconds).await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:position-changed",
                serde_json::json!({
                    "position_seconds": seconds,
                }),
            );
            Ok(())
        }
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
pub async fn stream_get_state(
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
) -> Result<String, String> {
    let manager = streaming_manager.lock().await;
    let state = manager.get_state();

    let state_str = match state {
        StreamingState::Idle => "idle",
        StreamingState::Loading => "loading",
        StreamingState::Ready => "ready",
        StreamingState::Playing => "playing",
        StreamingState::Paused => "paused",
        StreamingState::Seeking => "seeking",
        StreamingState::Error => "error",
    };

    Ok(state_str.to_string())
}

#[tauri::command]
pub async fn stream_get_file_info(
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
) -> Result<Option<AudioFileInfoPayload>, String> {
    let manager = streaming_manager.lock().await;
    match manager.get_audio_info() {
        Some(info) => Ok(Some(convert_audio_file_info(&info))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn stream_update_filters(
    _filters: Vec<Biquad>,
    _loudness: Option<LoudnessCompensation>,
    _streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // println!(
    //     "[TAURI] Updating filters: {} filters{}",
    //     _filters.len(),
    //     if _loudness.is_some() {
    //         " with loudness"
    //     } else {
    //         ""
    //     }
    // );

    // TODO: Phase 3 - Convert filters to plugins and update plugin chain
    // For now, return an error indicating this feature is not yet implemented
    let error_msg = "Filter updates not yet implemented in native engine (coming in Phase 3)".to_string();
    let _ = app_handle.emit(
        "stream:error",
        AudioError {
            error: error_msg.clone(),
        },
    );
    return Err(error_msg);

    /*
    let manager = streaming_manager.lock().await;
    match manager.update_filters(filters, loudness).await {
        Ok(_) => {
            let _ = app_handle.emit(
                "stream:filters-updated",
                serde_json::json!({
                    "ok": true,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = app_handle.emit(
                "stream:filters-updated",
                serde_json::json!({
                    "ok": false,
                    "error": error_msg.clone(),
                }),
            );
            Err(error_msg)
        }
    }
    */
}
