// ============================================================================
// CamillaDSP Integration Module
// ============================================================================

// Re-export all submodules
pub mod config;
pub mod errors;
pub mod manager;
pub mod process;
pub mod types;
pub mod utils;
pub mod websocket;

// Re-export the main public API items for convenience
pub use config::{
    convert_raw_to_wav, fix_rf64_wav, generate_filters_yaml, generate_matrix_mixer_yaml,
    generate_pipeline, generate_playback_config, generate_recording_config,
    generate_streaming_config, get_device_native_sample_rate, map_input_device, map_output_device,
    write_config_to_file, write_config_to_temp,
};
pub use errors::{CamillaError, CamillaResult};
pub use manager::AudioManager;
pub use process::CamillaDSPProcess;
pub use types::{
    AudioState, AudioStreamState, CamillaDSPConfig, CaptureDevice, ChannelMapMode, ChannelsSetting,
    DeviceConfig, PipelineStep, PlaybackDevice, SharedAudioStreamState,
};
pub use utils::find_camilladsp_binary;
pub use websocket::{CamillaCommand, CamillaWebSocketClient};
