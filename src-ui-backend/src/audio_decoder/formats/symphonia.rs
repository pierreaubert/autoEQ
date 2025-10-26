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
use crate::audio_decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::audio_decoder::formats::AudioFormat;

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

        let (final_format_reader, final_decoder, channels) = if channels_opt.is_none() {
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
            match format_reader.next_packet() {
                Ok(packet) => {
                    if packet.track_id() == track_id {
                        match temp_decoder.decode(&packet) {
                            Ok(decoded) => {
                                detected_channels = Some(decoded.spec().channels.count() as u16);
                            }
                            Err(_) => {}
                        }
                    }
                }
                Err(_) => {}
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
                .map_err(|e| AudioDecoderError::from(e))?;
            let new_format_reader = probe_result.format;

            // Recreate decoder for fresh state
            let new_decoder = codec_registry
                .make(&codec_params, &decoder_opts)
                .map_err(|e| {
                    AudioDecoderError::UnsupportedFormat(format!(
                        "Cannot create decoder for codec: {:?}",
                        e
                    ))
                })?;

            (new_format_reader, new_decoder, channels)
        } else {
            // Channel info is available, use as is
            let decoder = codec_registry
                .make(&codec_params, &decoder_opts)
                .map_err(|e| {
                    AudioDecoderError::UnsupportedFormat(format!(
                        "Cannot create decoder for codec: {:?}",
                        e
                    ))
                })?;
            (format_reader, decoder, channels_opt.unwrap())
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
        let timestamp = frame_position;

        match self.format_reader.seek(
            symphonia::core::formats::SeekMode::Accurate,
            symphonia::core::formats::SeekTo::Time {
                time: symphonia::core::units::Time::from(timestamp),
                track_id: Some(self.track_id),
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
mod tests {
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
