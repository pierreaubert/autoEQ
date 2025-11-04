// ============================================================================
// CamillaDSP Integration Module
// ============================================================================

// Re-export all submodules
pub mod errors;
pub mod types;
pub mod process;
pub mod websocket;
pub mod manager;
pub mod config;
pub mod utils;

// Re-export the main public API items for convenience
pub use errors::{CamillaError, CamillaResult};
pub use types::{
    AudioState, AudioStreamState, SharedAudioStreamState, CamillaDSPConfig,
    DeviceConfig, CaptureDevice, PlaybackDevice, PipelineStep, ChannelsSetting,
    ChannelMapMode
};
pub use process::CamillaDSPProcess;
pub use websocket::{CamillaWebSocketClient, CamillaCommand};
pub use manager::AudioManager;
pub use config::{
    generate_streaming_config, generate_playback_config, generate_recording_config,
    generate_filters_yaml, generate_matrix_mixer_yaml, generate_pipeline,
    write_config_to_temp, write_config_to_file, get_device_native_sample_rate,
    map_output_device, map_input_device, convert_raw_to_wav
};
pub use utils::find_camilladsp_binary;
