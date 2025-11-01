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

pub mod loudness_monitor;
pub mod replaygain;

pub use loudness_monitor::{LoudnessInfo, LoudnessMonitor};
