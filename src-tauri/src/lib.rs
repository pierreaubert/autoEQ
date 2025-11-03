use ndarray::Array1;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

// Import audio streaming types
use sotf_audio::AudioManager;
use sotf_audio::AudioStreamingManager;
use sotf_audio::SharedAudioState;

// Declare modules
mod tauri_speakers;
mod tauri_optim;
mod tauri_plots;
mod tauri_audio_devices;
mod tauri_audio_recording;
mod tauri_audio_streaming;
mod tauri_loudness;
mod tauri_spectrum;
mod tauri_replay_gain;
mod tauri_compute_eq;
mod tauri_generate_eq;

pub use tauri_speakers::{
    get_speakers,
    get_speaker_versions,
    get_speaker_measurements,
};

pub use tauri_optim::{
    CancellationState,
    run_optimization,
    cancel_optimization,
};

pub use tauri_plots::{
    generate_plot_filters,
    generate_plot_spin,
    generate_plot_spin_details,
    generate_plot_spin_tonal,
};

pub use tauri_audio_devices::{
    get_audio_devices,
    set_audio_device,
    get_audio_config,
    get_device_properties,
};

pub use tauri_audio_recording::{
    audio_start_recording,
    audio_stop_recording,
    audio_get_signal_peak,
    audio_get_recording_spl,
};

pub use tauri_audio_streaming::{
    stream_load_file,
    stream_start_playback,
    stream_pause_playback,
    stream_resume_playback,
    stream_stop_playback,
    stream_seek,
    stream_get_state,
    stream_get_file_info,
};
pub use sotf_audio::StreamingState;

pub use tauri_loudness::{
    stream_enable_loudness_monitoring,
    stream_disable_loudness_monitoring,
    stream_get_loudness,
};

pub use tauri_spectrum::{
    stream_enable_spectrum_monitoring,
    stream_disable_spectrum_monitoring,
    stream_get_spectrum,
};

pub use tauri_replay_gain::analyze_replaygain;

pub use tauri_compute_eq::compute_eq_response;

pub use tauri_generate_eq::{
    generate_apo_format,
    generate_aupreset_format,
    generate_rme_format,
    generate_rme_room_format,
};


#[tauri::command]
fn exit_app(window: tauri::Window) {
    window.close().unwrap();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Find CamillaDSP binary
    let camilla_binary = sotf_audio::camilla::find_camilladsp_binary().unwrap_or_else(|e| {
        eprintln!("[TAURI] Warning: CamillaDSP binary not found: {}", e);
        eprintln!("[TAURI] Audio playback features will not be available.");
        std::path::PathBuf::from("/usr/local/bin/camilladsp")
    });

    // Create AudioManager (wrapped in Mutex for Tauri state)
    let audio_manager = Mutex::new(AudioManager::new(camilla_binary.clone()));

    // Create AudioStreamingManager for all audio format playback (WAV, FLAC, MP3, etc.)
    let streaming_manager = Mutex::new(AudioStreamingManager::new(camilla_binary));

    let mut builder = tauri::Builder::default();

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_devtools::init());
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancellationState::new())
        .manage(SharedAudioState::default())
        .manage(audio_manager)
        .manage(streaming_manager)
        .setup(|app| {
            // Spawn background task to monitor streaming events and forward them to frontend
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    // Poll for events from the decoder thread
                    let streaming_mgr = app_handle.state::<Mutex<AudioStreamingManager>>();
                    if let Ok(manager) = streaming_mgr.try_lock() {
                        for event in manager.drain_events() {
                            let _ = match event {
                                sotf_audio::StreamingEvent::EndOfStream => {
                                    println!("[EVENT MONITOR] End of stream detected");
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": "ended" }),
                                    )
                                }
                                sotf_audio::StreamingEvent::StateChanged(state) => {
                                    let state_str = match state {
                                        StreamingState::Idle => "idle",
                                        StreamingState::Loading => "loading",
                                        StreamingState::Ready => "ready",
                                        StreamingState::Playing => "playing",
                                        StreamingState::Paused => "paused",
                                        StreamingState::Seeking => "seeking",
                                        StreamingState::Error => "error",
                                    };
                                    println!("[TAURI] State changed to: {}", state_str);
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": state_str }),
                                    )
                                }
                                sotf_audio::StreamingEvent::Error(msg) => {
                                    println!("[TAURI] Error: {}", msg);
                                    app_handle.emit(
                                        "stream:state-changed",
                                        serde_json::json!({ "state": "error", "message": msg }),
                                    )
                                }
                            };
                        }
                    }

                    // Sleep to avoid busy-waiting
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            // Audio recording commands
            audio_start_recording,
            audio_stop_recording,
            audio_get_signal_peak,
            audio_get_recording_spl,
            // Streaming playback commands (supports all audio formats)
            stream_load_file,
            stream_start_playback,
            stream_pause_playback,
            stream_resume_playback,
            stream_stop_playback,
            stream_seek,
            stream_get_state,
            stream_get_file_info,
            stream_enable_loudness_monitoring,
            stream_disable_loudness_monitoring,
            stream_get_loudness,
            stream_enable_spectrum_monitoring,
            stream_disable_spectrum_monitoring,
            stream_get_spectrum,
            // Export format commands
            generate_apo_format,
            generate_aupreset_format,
            generate_rme_format,
            generate_rme_room_format,
            // ReplayGain analysis
            analyze_replaygain,
            // EQ response computation
            compute_eq_response
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
