use crate::audio_decoder::error::{AudioDecoderError, AudioDecoderResult};
use std::path::Path;

pub mod symphonia;

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
