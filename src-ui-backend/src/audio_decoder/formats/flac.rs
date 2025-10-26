use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_codecs;
use symphonia::default::get_probe;

use crate::audio_decoder::decoder::{AudioDecoder, AudioSpec, DecodedAudio};
use crate::audio_decoder::error::{AudioDecoderError, AudioDecoderResult};
use crate::audio_decoder::formats::AudioFormat;

/// FLAC decoder implementation using Symphonia
pub struct FlacDecoder {
    /// Audio specification
    spec: AudioSpec,
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
    /// Buffer size for decoding chunks
    buffer_frames: usize,
}

impl FlacDecoder {
    /// Create a new FLAC decoder
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

        // Probe the file to determine format
        let probe = get_probe();
        let probed = probe
            .format(
                &hint,
                media_source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| match e {
                SymphoniaError::Unsupported(_) => AudioDecoderError::UnsupportedFormat(
                    "FLAC format not supported by Symphonia".to_string(),
                ),
                _ => AudioDecoderError::from(e),
            })?;

        let format_reader = probed.format;

        // Get the default track (usually the first one)
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioDecoderError::InvalidFile("No valid audio track found".to_string())
            })?;

        let track_id = track.id;
        let codec_params = &track.codec_params;

        // Create decoder for this track
        let decoder_opts = DecoderOptions::default();
        let decoder = get_codecs()
            .make(codec_params, &decoder_opts)
            .map_err(|e| {
                AudioDecoderError::UnsupportedFormat(format!("Cannot create decoder: {:?}", e))
            })?;

        // Extract audio specification
        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| AudioDecoderError::InvalidFile("No sample rate found".to_string()))?;

        let channels = codec_params
            .channels
            .map(|layout| layout.count() as u16)
            .ok_or_else(|| {
                AudioDecoderError::InvalidFile("No channel information found".to_string())
            })?;

        let bits_per_sample = codec_params.bits_per_sample.unwrap_or(16);
        let total_frames = codec_params.n_frames;

        let spec = AudioSpec {
            sample_rate,
            channels,
            bits_per_sample: bits_per_sample as u16,
            total_frames,
        };

        println!(
            "[FlacDecoder] Initialized: {}Hz, {}ch, {}bit, {:?} frames",
            spec.sample_rate, spec.channels, spec.bits_per_sample, spec.total_frames
        );

        Ok(Self {
            spec,
            format_reader,
            decoder,
            position: 0,
            track_id,
            eof: false,
            buffer_frames: 4096, // Default chunk size
        })
    }

    /// Set the buffer size in frames for decoding chunks
    pub fn set_buffer_frames(&mut self, frames: usize) {
        self.buffer_frames = frames;
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

impl AudioDecoder for FlacDecoder {
    fn spec(&self) -> &AudioSpec {
        &self.spec
    }

    fn format(&self) -> AudioFormat {
        AudioFormat::Flac
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
            AudioDecoderError::DecodingFailed(format!("Failed to decode packet: {:?}", e))
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
                "Failed to seek to frame {}: {:?}",
                frame_position, err
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

    // Note: These tests require actual FLAC files to work properly
    // In a real test suite, you'd want to include test fixtures

    #[test]
    fn test_flac_decoder_creation_fails_for_nonexistent() {
        let result = FlacDecoder::new("nonexistent.flac");
        assert!(result.is_err());
    }

    #[test]
    fn test_flac_decoder_format() {
        // This test would need a real FLAC file to work
        // For now, we just test the general structure
        assert_eq!(AudioFormat::Flac.as_str(), "FLAC");
        assert!(AudioFormat::Flac.is_lossless());
    }

    // Integration test would go here with actual FLAC files:
    /*
    #[test]
    fn test_flac_decoder_with_real_file() {
        let decoder = FlacDecoder::new("test_files/test.flac").unwrap();
        assert_eq!(decoder.format(), AudioFormat::Flac);
        assert!(decoder.spec().sample_rate > 0);
        assert!(decoder.spec().channels > 0);
        // Test actual decoding...
    }
    */
}
