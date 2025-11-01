use crate::audio_decoder::error::{AudioDecoderError, AudioDecoderResult};
use std::fs::File;
use std::path::Path;

use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CodecRegistry, Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint, Probe};

use crate::audio_decoder::decoder::{AudioDecoder, AudioSpec, DecodedAudio};

/// Create a custom probe with all supported format readers registered
fn create_probe() -> Probe {
    let mut probe = Probe::default();

    // Register all format readers
    // Note: AAC is supported both in MP4/M4A containers (via IsoMp4Reader)
    // and as raw ADTS AAC files (via AdtsReader)
    probe.register_all::<symphonia_bundle_flac::FlacReader>();
    probe.register_all::<symphonia_bundle_mp3::MpaReader>();
    probe.register_all::<symphonia_format_riff::WavReader>();
    probe.register_all::<symphonia_format_ogg::OggReader>();
    probe.register_all::<symphonia_format_isomp4::IsoMp4Reader>();
    probe.register_all::<symphonia_codec_aac::AdtsReader>();

    probe
}

/// Create a custom codec registry with all supported codecs
fn create_codec_registry() -> CodecRegistry {
    let mut registry = CodecRegistry::new();

    // Register all codecs
    registry.register_all::<symphonia_bundle_flac::FlacDecoder>();
    registry.register_all::<symphonia_bundle_mp3::MpaDecoder>();
    registry.register_all::<symphonia_codec_pcm::PcmDecoder>();
    registry.register_all::<symphonia_codec_aac::AacDecoder>();
    registry.register_all::<symphonia_codec_vorbis::VorbisDecoder>();

    registry
}

/// Unified Symphonia decoder implementation supporting all audio formats
pub struct SymphoniaDecoder {
    /// Audio specification
    spec: AudioSpec,
    /// Detected audio format
    format: AudioFormat,
    /// Symphonia format reader
    format_reader: Box<dyn FormatReader>,
    /// Symphonia decoder
    decoder: Box<dyn Decoder>,
    /// Current position in frames
    position: u64,
    /// Track ID in the format
    track_id: u32,
    /// End of stream flag
    eof: bool,
}

impl SymphoniaDecoder {
    /// Create a new Symphonia decoder for any supported format
    pub fn new<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let path = path.as_ref();

        // Open the file
        let file = File::open(path)?;
        let media_source = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint for the format
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }

        // Probe the file to determine format using our custom probe
        let probe = create_probe();
        let probe_result = probe
            .format(
                &hint,
                media_source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| match e {
                SymphoniaError::Unsupported(_) => AudioDecoderError::UnsupportedFormat(
                    "Audio format not supported by Symphonia".to_string(),
                ),
                _ => AudioDecoderError::from(e),
            })?;

        let mut format_reader = probe_result.format;

        // Get the default track (usually the first one)
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioDecoderError::InvalidFile("No valid audio track found".to_string())
            })?;

        let track_id = track.id;
        let codec_params = track.codec_params.clone();

        // Create decoder for this track using our custom codec registry
        let decoder_opts = DecoderOptions::default();
        let codec_registry = create_codec_registry();

        // Extract audio specification
        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| AudioDecoderError::InvalidFile("No sample rate found".to_string()))?;

        // For AAC and some other codecs, channel information may not be available
        // until the first packet is decoded. Try to get it from codec params first,
        // and if that fails, decode the first packet.
        let channels_opt = codec_params.channels.map(|layout| layout.count() as u16);

        let (final_format_reader, final_decoder, channels) = match channels_opt {
            None => {
                // Need to probe for channels - create temporary decoder
                let mut temp_decoder =
                    codec_registry
                        .make(&codec_params, &decoder_opts)
                        .map_err(|e| {
                            AudioDecoderError::UnsupportedFormat(format!(
                                "Cannot create decoder for codec: {:?}",
                                e
                            ))
                        })?;

                // Decode first packet to get channel info
                let mut detected_channels = None;
                if let Ok(packet) = format_reader.next_packet()
                    && packet.track_id() == track_id
                    && let Ok(decoded) = temp_decoder.decode(&packet)
                {
                    detected_channels = Some(decoded.spec().channels.count() as u16);
                }

                // If we still don't have channel info, fail
                let channels = detected_channels.ok_or_else(|| {
                    AudioDecoderError::InvalidFile(
                        "No channel information found even after decoding first packet".to_string(),
                    )
                })?;

                // Reset the format reader by creating a new one
                let file = File::open(path)?;
                let media_source = MediaSourceStream::new(Box::new(file), Default::default());
                let mut hint = Hint::new();
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    hint.with_extension(extension);
                }
                let probe = create_probe();
                let probe_result = probe
                    .format(
                        &hint,
                        media_source,
                        &FormatOptions::default(),
                        &MetadataOptions::default(),
                    )
                    .map_err(AudioDecoderError::from)?;
                let new_format_reader = probe_result.format;

                // Recreate decoder for fresh state
                let new_decoder =
                    codec_registry
                        .make(&codec_params, &decoder_opts)
                        .map_err(|e| {
                            AudioDecoderError::UnsupportedFormat(format!(
                                "Cannot create decoder for codec: {:?}",
                                e
                            ))
                        })?;

                (new_format_reader, new_decoder, channels)
            }
            Some(channels) => {
                // Channel info is available, use as is
                let decoder = codec_registry
                    .make(&codec_params, &decoder_opts)
                    .map_err(|e| {
                        AudioDecoderError::UnsupportedFormat(format!(
                            "Cannot create decoder for codec: {:?}",
                            e
                        ))
                    })?;
                (format_reader, decoder, channels)
            }
        };

        let bits_per_sample = codec_params.bits_per_sample.unwrap_or(16);
        let total_frames = codec_params.n_frames;

        let spec = AudioSpec {
            sample_rate,
            channels,
            bits_per_sample: bits_per_sample as u16,
            total_frames,
        };

        // Detect the format from the file extension
        let format = AudioFormat::from_path(path)?;

        println!(
            "[SymphoniaDecoder] Initialized {}: {}Hz, {}ch, {}bit, {:?} frames",
            format.as_str(),
            spec.sample_rate,
            spec.channels,
            spec.bits_per_sample,
            spec.total_frames
        );

        Ok(Self {
            spec,
            format,
            format_reader: final_format_reader,
            decoder: final_decoder,
            position: 0,
            track_id,
            eof: false,
        })
    }

    /// Convert audio buffer to normalized f32 samples
    fn convert_audio_buffer_static(audio_buf: AudioBufferRef) -> AudioDecoderResult<Vec<f32>> {
        let mut samples = Vec::new();
        let channels_count = audio_buf.spec().channels.count();
        let duration = audio_buf.frames();

        match audio_buf {
            AudioBufferRef::U8(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 128.0 - 1.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::U16(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 32768.0 - 1.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::U24(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = (buf.chan(ch)[frame].inner() as f32) / 8388608.0 - 1.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::U32(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 2147483648.0 - 1.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::S8(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 128.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 32768.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::S24(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = (buf.chan(ch)[frame].inner() as f32) / 8388608.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        let sample = buf.chan(ch)[frame] as f32 / 2147483648.0;
                        samples.push(sample);
                    }
                }
            }
            AudioBufferRef::F32(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        samples.push(buf.chan(ch)[frame]);
                    }
                }
            }
            AudioBufferRef::F64(buf) => {
                for frame in 0..duration {
                    for ch in 0..channels_count {
                        samples.push(buf.chan(ch)[frame] as f32);
                    }
                }
            }
        }

        Ok(samples)
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        self.format
    }

    fn decode_next(&mut self) -> AudioDecoderResult<Option<DecodedAudio>> {
        if self.eof {
            return Ok(None);
        }

        // Read the next packet
        let packet = match self.format_reader.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(ref err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                self.eof = true;
                return Ok(None);
            }
            Err(err) => {
                self.eof = true;
                return Err(AudioDecoderError::from(err));
            }
        };

        // Skip packets that don't belong to our track
        if packet.track_id() != self.track_id {
            return self.decode_next(); // Recursively try next packet
        }

        // Decode the packet
        let decoded_audio_buf = self.decoder.decode(&packet).map_err(|e| {
            AudioDecoderError::DecodingFailed(format!(
                "Failed to decode {} packet: {:?}",
                self.format.as_str(),
                e
            ))
        })?;

        let frame_count = decoded_audio_buf.frames() as u64;
        let samples = Self::convert_audio_buffer_static(decoded_audio_buf)?;

        let mut decoded = DecodedAudio::new(self.spec.clone());
        decoded.samples = samples;
        decoded.frame_position = self.position;

        self.position += frame_count;

        Ok(Some(decoded))
    }

    fn seek(&mut self, frame_position: u64) -> AudioDecoderResult<()> {
        // Use Symphonia's seek functionality
        // frame_position is in PCM frames (track time-base). Use TimeStamp, not Time-in-seconds.
        match self.format_reader.seek(
            symphonia::core::formats::SeekMode::Accurate,
            symphonia::core::formats::SeekTo::TimeStamp {
                ts: frame_position,
                track_id: self.track_id,
            },
        ) {
            Ok(seeked) => {
                self.position = seeked.actual_ts;
                self.eof = false;
                Ok(())
            }
            Err(err) => Err(AudioDecoderError::SeekFailed(format!(
                "Failed to seek to frame {} in {}: {:?}",
                frame_position,
                self.format.as_str(),
                err
            ))),
        }
    }

    fn position(&self) -> u64 {
        self.position
    }

    fn is_eof(&self) -> bool {
        self.eof
    }
}

#[cfg(test)]
mod tests_decoder {
    use super::*;

    #[test]
    fn test_symphonia_decoder_creation_fails_for_nonexistent() {
        let result = SymphoniaDecoder::new("nonexistent.flac");
        assert!(result.is_err());
    }

    #[test]
    fn test_symphonia_decoder_format_detection() {
        // Test that the decoder can detect supported formats from extensions
        // This would require actual audio files for proper testing
        assert_eq!(AudioFormat::Flac.as_str(), "FLAC");
        assert_eq!(AudioFormat::Mp3.as_str(), "MP3");
        assert_eq!(AudioFormat::Aac.as_str(), "AAC");
        assert_eq!(AudioFormat::Wav.as_str(), "WAV");
        assert_eq!(AudioFormat::Vorbis.as_str(), "Vorbis");
        assert_eq!(AudioFormat::Aiff.as_str(), "AIFF");
    }

    // Integration tests would go here with actual audio files:
    /*
    #[test]
    fn test_symphonia_decoder_with_real_files() {
        // Test with actual FLAC file
        let flac_decoder = SymphoniaDecoder::new("test_files/test.flac").unwrap();
        assert_eq!(flac_decoder.format(), AudioFormat::Flac);

        // Test with actual MP3 file
        let mp3_decoder = SymphoniaDecoder::new("test_files/test.mp3").unwrap();
        assert_eq!(mp3_decoder.format(), AudioFormat::Mp3);

        // Add more format tests...
    }
    */
}

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Flac,
    Mp3,
    Aac,
    Wav,
    Vorbis,
    Aiff,
}

impl AudioFormat {
    /// Detect audio format from file extension
    pub fn from_path<P: AsRef<Path>>(path: P) -> AudioDecoderResult<Self> {
        let path = path.as_ref();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .ok_or_else(|| {
                AudioDecoderError::UnsupportedFormat("No file extension found".to_string())
            })?;

        match extension.as_str() {
            "flac" => Ok(AudioFormat::Flac),
            "mp3" => Ok(AudioFormat::Mp3),
            // AAC in MP4/M4A containers and raw ADTS AAC (.aac) are both supported
            "aac" | "m4a" | "mp4" => Ok(AudioFormat::Aac),
            "wav" => Ok(AudioFormat::Wav),
            "ogg" | "oga" => Ok(AudioFormat::Vorbis),
            "aiff" | "aif" => Ok(AudioFormat::Aiff),
            _ => Err(AudioDecoderError::UnsupportedFormat(format!(
                "Unsupported file extension: {}",
                extension
            ))),
        }
    }

    /// Get the format name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "FLAC",
            AudioFormat::Mp3 => "MP3",
            AudioFormat::Aac => "AAC",
            AudioFormat::Wav => "WAV",
            AudioFormat::Vorbis => "Vorbis",
            AudioFormat::Aiff => "AIFF",
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "flac",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Aac => "m4a",
            AudioFormat::Wav => "wav",
            AudioFormat::Vorbis => "ogg",
            AudioFormat::Aiff => "aiff",
        }
    }

    /// Check if the format is lossless
    pub fn is_lossless(&self) -> bool {
        match self {
            AudioFormat::Flac => true,
            AudioFormat::Mp3 => false,
            AudioFormat::Aac => false, // Usually not, could be ALAC but we'll assume lossy
            AudioFormat::Wav => true,
            AudioFormat::Vorbis => false,
            AudioFormat::Aiff => true,
        }
    }

    /// Get all supported formats
    pub fn supported_formats() -> Vec<AudioFormat> {
        vec![
            AudioFormat::Flac,
            AudioFormat::Mp3,
            AudioFormat::Aac,
            AudioFormat::Wav,
            AudioFormat::Vorbis,
            AudioFormat::Aiff,
        ]
    }

    /// Get a user-friendly description of supported formats
    pub fn supported_formats_string() -> String {
        let formats: Vec<&str> = Self::supported_formats()
            .iter()
            .map(|f| f.as_str())
            .collect();
        formats.join(", ")
    }
}

impl std::fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_detection() {
        // Test FLAC
        assert_eq!(
            AudioFormat::from_path("test.flac").unwrap(),
            AudioFormat::Flac
        );
        assert_eq!(
            AudioFormat::from_path("test.FLAC").unwrap(),
            AudioFormat::Flac
        );

        // Test MP3
        assert_eq!(
            AudioFormat::from_path("test.mp3").unwrap(),
            AudioFormat::Mp3
        );

        // Test AAC/M4A
        assert_eq!(
            AudioFormat::from_path("test.aac").unwrap(),
            AudioFormat::Aac
        );
        assert_eq!(
            AudioFormat::from_path("test.m4a").unwrap(),
            AudioFormat::Aac
        );

        // Test WAV
        assert_eq!(
            AudioFormat::from_path("test.wav").unwrap(),
            AudioFormat::Wav
        );

        // Test Vorbis/OGG
        assert_eq!(
            AudioFormat::from_path("test.ogg").unwrap(),
            AudioFormat::Vorbis
        );

        // Test AIFF
        assert_eq!(
            AudioFormat::from_path("test.aiff").unwrap(),
            AudioFormat::Aiff
        );
        assert_eq!(
            AudioFormat::from_path("test.aif").unwrap(),
            AudioFormat::Aiff
        );

        // Test with path
        assert_eq!(
            AudioFormat::from_path(PathBuf::from("path/to/music.flac")).unwrap(),
            AudioFormat::Flac
        );

        // Test unsupported format
        assert!(AudioFormat::from_path("test.xyz").is_err());
        assert!(AudioFormat::from_path("test").is_err());
    }

    #[test]
    fn test_format_properties() {
        // Test FLAC
        let flac = AudioFormat::Flac;
        assert_eq!(flac.as_str(), "FLAC");
        assert_eq!(flac.extension(), "flac");
        assert!(flac.is_lossless());

        // Test MP3
        let mp3 = AudioFormat::Mp3;
        assert_eq!(mp3.as_str(), "MP3");
        assert_eq!(mp3.extension(), "mp3");
        assert!(!mp3.is_lossless());

        // Test AAC
        let aac = AudioFormat::Aac;
        assert_eq!(aac.as_str(), "AAC");
        assert_eq!(aac.extension(), "m4a");
        assert!(!aac.is_lossless());

        // Test WAV
        let wav = AudioFormat::Wav;
        assert_eq!(wav.as_str(), "WAV");
        assert_eq!(wav.extension(), "wav");
        assert!(wav.is_lossless());

        // Test Vorbis
        let vorbis = AudioFormat::Vorbis;
        assert_eq!(vorbis.as_str(), "Vorbis");
        assert_eq!(vorbis.extension(), "ogg");
        assert!(!vorbis.is_lossless());

        // Test AIFF
        let aiff = AudioFormat::Aiff;
        assert_eq!(aiff.as_str(), "AIFF");
        assert_eq!(aiff.extension(), "aiff");
        assert!(aiff.is_lossless());
    }

    #[test]
    fn test_supported_formats() {
        let formats = AudioFormat::supported_formats();
        assert_eq!(formats.len(), 6);
        assert!(formats.contains(&AudioFormat::Flac));
        assert!(formats.contains(&AudioFormat::Mp3));
        assert!(formats.contains(&AudioFormat::Aac));
        assert!(formats.contains(&AudioFormat::Wav));
        assert!(formats.contains(&AudioFormat::Vorbis));
        assert!(formats.contains(&AudioFormat::Aiff));

        let formats_string = AudioFormat::supported_formats_string();
        assert!(formats_string.contains("FLAC"));
        assert!(formats_string.contains("MP3"));
        assert!(formats_string.contains("AAC"));
        assert!(formats_string.contains("WAV"));
        assert!(formats_string.contains("Vorbis"));
        assert!(formats_string.contains("AIFF"));
    }
}
