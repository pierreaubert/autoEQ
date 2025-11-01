pub mod audio;
pub use audio::SharedAudioState;

pub mod audio_decoder;
pub use audio_decoder::{
    AudioDecoder, AudioDecoderError, AudioDecoderResult, AudioFormat, AudioStream, DecodedAudio,
    StreamConfig, create_decoder, probe_file,
};
// Re-export specific types with their full paths
pub use audio_decoder::decoder::AudioSpec;
pub use audio_decoder::stream::{StreamEvent, StreamPosition, StreamState};

pub mod audio_streaming;
pub use audio_streaming::{
    AudioFileInfo, AudioStreamingManager, StreamingCommand, StreamingEvent, StreamingState,
};

pub mod camilla;
pub use camilla::{
    AudioManager, AudioState, AudioStreamState, CamillaError, CamillaResult, FilterParams,
    SharedAudioStreamState,
};

pub mod eq_response;
pub mod export;
pub mod loudness_monitor;
pub mod optim;
pub mod plot;
pub mod replaygain;
pub mod spinorama_api;

// Re-export commonly used types and helpers for easier access in tests and consumers
pub use eq_response::{EqResponseResult, FilterParam as EqFilterParam, FilterResponse, compute_eq_response};
pub use export::{ExportFormat, FilterParam as ExportFilterParam};
pub use loudness_monitor::{LoudnessInfo, LoudnessMonitor};
pub use optim::{
    CancellationState, OptimizationParams, OptimizationResult, ProgressUpdate, validate_params,
};
pub use plot::{CurveData, PlotData, curve_data_to_curve};
pub use replaygain::{ReplayGainInfo, analyze_file};
pub use spinorama_api::{
    Cea2034Data, FrequencyResponse, MeasurementInfo, SpeakerInfo, SpinAudioClient,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_mocks;
