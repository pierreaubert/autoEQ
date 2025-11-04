pub mod devices;
pub use devices::SharedAudioState;

pub mod decoder;
pub use decoder::{
    AudioDecoder, AudioDecoderError, AudioDecoderResult, AudioFormat, AudioStream, DecodedAudio,
    StreamConfig, create_decoder, probe_file,
};

pub use decoder::decoder::AudioSpec;
pub use decoder::stream::{StreamEvent, StreamPosition, StreamState};

pub mod manager;
pub use manager::{
    AudioFileInfo, AudioStreamingManager, StreamingCommand, StreamingEvent, StreamingState,
};

pub mod filters;
pub use filters::FilterParams;

pub mod loudness_compensation;
pub use loudness_compensation::LoudnessCompensation;

pub mod camilla;
pub use camilla::{
    AudioManager, AudioState, AudioStreamState, CamillaError, CamillaResult,
    SharedAudioStreamState,
};

pub mod replaygain;
pub mod signals;

pub mod loudness_monitor;
pub use loudness_monitor::{LoudnessInfo, LoudnessMonitor};

pub mod spectrum_analyzer;
pub use spectrum_analyzer::{SpectrumAnalyzer, SpectrumConfig, SpectrumInfo};
