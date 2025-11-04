// ============================================================================
// Streaming Audio Commands
// ============================================================================

use crate::tauri_audio_recording::AudioError;
use sotf_audio::camilla::{ChannelMapMode, LoudnessCompensation};
use sotf_audio::{AudioFileInfo, AudioStreamingManager, FilterParams, StreamingState};
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

#[tauri::command]
pub async fn stream_start_playback(
    output_device: Option<String>,
    filters: Vec<FilterParams>,
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[TAURI] Starting playback with {} filters", filters.len());

    let mut manager = streaming_manager.lock().await;
    match manager
        .start_playback(
            output_device.clone(),
            filters,
            ChannelMapMode::Normal,
            None,
            None,
        )
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
        Some(info) => Ok(Some(convert_audio_file_info(info))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn stream_update_filters(
    filters: Vec<FilterParams>,
    loudness: Option<LoudnessCompensation>,
    streaming_manager: State<'_, Mutex<AudioStreamingManager>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!(
        "[TAURI] Updating filters: {} filters{}",
        filters.len(),
        if loudness.is_some() {
            " with loudness"
        } else {
            ""
        }
    );

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
}
