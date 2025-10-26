use crate::audio_decoder::error::{AudioDecoderError, AudioDecoderResult};
use std::path::Path;

pub mod flac;

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Flac,
    // Future formats to be added:
    // Mp3,
    // Ogg,
    // Wav,
    // M4a,
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
            // Future format support:
            // "mp3" => Ok(AudioFormat::Mp3),
            // "ogg" => Ok(AudioFormat::Ogg),
            // "wav" => Ok(AudioFormat::Wav),
            // "m4a" => Ok(AudioFormat::M4a),
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
            // AudioFormat::Mp3 => "MP3",
            // AudioFormat::Ogg => "OGG",
            // AudioFormat::Wav => "WAV",
            // AudioFormat::M4a => "M4A",
        }
    }

    /// Get the file extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Flac => "flac",
            // AudioFormat::Mp3 => "mp3",
            // AudioFormat::Ogg => "ogg",
            // AudioFormat::Wav => "wav",
            // AudioFormat::M4a => "m4a",
        }
    }

    /// Check if the format is lossless
    pub fn is_lossless(&self) -> bool {
        match self {
            AudioFormat::Flac => true,
            // AudioFormat::Mp3 => false,
            // AudioFormat::Ogg => false,
            // AudioFormat::Wav => true,
            // AudioFormat::M4a => false, // Usually not, could be ALAC
        }
    }

    /// Get all supported formats
    pub fn supported_formats() -> Vec<AudioFormat> {
        vec![AudioFormat::Flac]
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
        assert_eq!(
            AudioFormat::from_path("test.flac").unwrap(),
            AudioFormat::Flac
        );
        assert_eq!(
            AudioFormat::from_path("test.FLAC").unwrap(),
            AudioFormat::Flac
        );
        assert_eq!(
            AudioFormat::from_path(PathBuf::from("path/to/music.flac")).unwrap(),
            AudioFormat::Flac
        );

        // Test unsupported format
        assert!(AudioFormat::from_path("test.mp3").is_err());
        assert!(AudioFormat::from_path("test").is_err());
    }

    #[test]
    fn test_format_properties() {
        let flac = AudioFormat::Flac;
        assert_eq!(flac.as_str(), "FLAC");
        assert_eq!(flac.extension(), "flac");
        assert!(flac.is_lossless());
    }

    #[test]
    fn test_supported_formats() {
        let formats = AudioFormat::supported_formats();
        assert!(!formats.is_empty());
        assert!(formats.contains(&AudioFormat::Flac));

        let formats_string = AudioFormat::supported_formats_string();
        assert!(formats_string.contains("FLAC"));
    }
}
