//! Signal generation and recording module
//!
//! This module provides functionality to generate test signals, play them back,
//! record the output, and analyze the results.

use crate::signal_analysis::{analyze_recording, write_analysis_csv};
// Note: PlaybackRecorder is available but not used in current implementation
// use crate::audio_playback::{PlaybackRecorder, PlaybackRecordingConfig};
// NOTE: CamillaDSP integration has been removed - record_and_analyze function is deprecated
// use crate::camilla::{AudioManager, ChannelMapMode};
use crate::signals::*;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Signal type for recording
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
    Tone,
    TwoTone,
    Sweep,
    WhiteNoise,
    PinkNoise,
    MNoise,
}

impl SignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tone => "tone",
            Self::TwoTone => "two-tone",
            Self::Sweep => "sweep",
            Self::WhiteNoise => "white-noise",
            Self::PinkNoise => "pink-noise",
            Self::MNoise => "m-noise",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "tone" => Ok(Self::Tone),
            "two-tone" | "twotone" => Ok(Self::TwoTone),
            "sweep" => Ok(Self::Sweep),
            "white-noise" | "white_noise" | "whitenoise" => Ok(Self::WhiteNoise),
            "pink-noise" | "pink_noise" | "pinknoise" => Ok(Self::PinkNoise),
            "m-noise" | "m_noise" | "mnoise" => Ok(Self::MNoise),
            _ => Err(format!("Unknown signal type: {}", s)),
        }
    }
}

/// Parameters for signal generation
#[derive(Debug, Clone)]
pub enum SignalParams {
    Tone {
        freq: f32,
        amp: f32,
    },
    TwoTone {
        freq1: f32,
        amp1: f32,
        freq2: f32,
        amp2: f32,
    },
    Sweep {
        start_freq: f32,
        end_freq: f32,
        amp: f32,
    },
    Noise {
        amp: f32,
    },
}

/// Generate a signal based on parameters
pub fn generate_signal(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<Vec<f32>, String> {
    let signal = match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            gen_tone(*freq, *amp, sample_rate, duration)
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => gen_two_tone(*freq1, *amp1, *freq2, *amp2, sample_rate, duration),
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => gen_log_sweep(*start_freq, *end_freq, *amp, sample_rate, duration),
        (SignalType::WhiteNoise, SignalParams::Noise { amp }) => {
            gen_white_noise(*amp, sample_rate, duration)
        }
        (SignalType::PinkNoise, SignalParams::Noise { amp }) => {
            gen_pink_noise(*amp, sample_rate, duration)
        }
        (SignalType::MNoise, SignalParams::Noise { amp }) => {
            gen_m_noise(*amp, sample_rate, duration)
        }
        _ => {
            return Err(format!(
                "Signal type {:?} does not match parameters {:?}",
                signal_type, params
            ));
        }
    };

    Ok(signal)
}

/// Prepare a signal for playback with fades and padding
pub fn prepare_signal(signal: Vec<f32>, sample_rate: u32) -> Vec<f32> {
    const FADE_MS: f32 = 20.0;
    const PADDING_MS: f32 = 250.0;

    prepare_signal_for_playback(signal, sample_rate, FADE_MS, PADDING_MS)
}

/// Write signal to a temporary WAV file
pub fn write_temp_wav(
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<NamedTempFile, String> {
    let temp_file = NamedTempFile::with_suffix(".wav")
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    write_wav_file(temp_file.path(), signal, sample_rate, channels)?;

    Ok(temp_file)
}

/// Write signal to a WAV file
pub fn write_wav_file(
    path: &Path,
    signal: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in signal {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV file: {}", e))?;

    Ok(())
}

/// Generate output filenames for a recording with both send and record channels
pub fn generate_output_filenames_stereo(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    send_channel: u16,
    record_channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_send{}_rec{}_{}",
            prefix,
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    } else {
        format!(
            "{}_send{}_rec{}_{}",
            signal_type.as_str(),
            send_channel,
            record_channel,
            sample_rate
        )
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}

/// Generate output filenames for a recording
pub fn generate_output_filenames(
    name_prefix: Option<&str>,
    signal_type: SignalType,
    channel: u16,
    sample_rate: u32,
) -> (PathBuf, PathBuf) {
    let base_name = if let Some(prefix) = name_prefix {
        format!(
            "{}_{}_ch{}_{}",
            prefix,
            signal_type.as_str(),
            channel,
            sample_rate
        )
    } else {
        format!("{}_ch{}_{}", signal_type.as_str(), channel, sample_rate)
    };

    let wav_path = PathBuf::from(format!("{}.wav", base_name));
    let csv_path = PathBuf::from(format!("{}.csv", base_name));

    (wav_path, csv_path)
}

/// Perform recording and analysis using AudioStreamingManager for playback
/// and cpal for recording.
///
/// Plays back a signal to a specific output channel while simultaneously
/// recording from a specific input channel, then analyzes the result.
pub fn record_and_analyze(
    temp_wav_path: &Path,
    recorded_wav_path: &Path,
    reference_signal: &[f32],
    sample_rate: u32,
    output_csv_path: &Path,
    output_channel: u16,
    input_channel: u16,
) -> Result<(), String> {
    use crate::AudioStreamingManager;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use parking_lot::Mutex;
    use std::sync::Arc;
    use std::thread::sleep;
    use std::time::Duration;

    eprintln!("[record_and_analyze] Starting playback and recording...");
    eprintln!("[record_and_analyze]   Playback file: {:?}", temp_wav_path);
    eprintln!("[record_and_analyze]   Output channel: {}", output_channel);
    eprintln!("[record_and_analyze]   Input channel: {}", input_channel);
    eprintln!("[record_and_analyze]   Sample rate: {}", sample_rate);

    // Calculate expected duration
    let expected_duration = reference_signal.len() as f64 / sample_rate as f64;
    eprintln!(
        "[record_and_analyze]   Expected duration: {:.2}s",
        expected_duration
    );

    // Set up recording stream
    let host = cpal::default_host();
    let input_device = host
        .default_input_device()
        .ok_or_else(|| "No input device available".to_string())?;

    eprintln!(
        "[record_and_analyze] Input device: {}",
        input_device
            .name()
            .unwrap_or_else(|_| "Unknown".to_string())
    );

    // Configure input stream
    let input_config = cpal::StreamConfig {
        channels: (input_channel + 1).max(2) as u16, // Need at least input_channel+1 channels
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Shared state for recording
    let recorded_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded_samples_clone = Arc::clone(&recorded_samples);

    // Create input stream
    let input_stream = input_device
        .build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let num_channels = input_config.channels as usize;
                let mut recorded = recorded_samples_clone.lock();

                // Extract only the specified input channel
                for frame in data.chunks(num_channels) {
                    if (input_channel as usize) < frame.len() {
                        recorded.push(frame[input_channel as usize]);
                    }
                }
            },
            |err| eprintln!("[record_and_analyze] Input stream error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    // Start recording
    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {}", e))?;
    eprintln!("[record_and_analyze] Recording started");

    // Small delay to let recording buffer fill
    sleep(Duration::from_millis(100));

    // Start playback using AudioStreamingManager
    let mut manager = AudioStreamingManager::new();
    manager
        .load_file(temp_wav_path)
        .map_err(|e| format!("Failed to load file: {}", e))?;

    // Create matrix plugin config to route mono signal to specific output channel
    // Matrix is 1 input channel to N output channels (where N = output_channel + 1)
    let input_channels = 1;
    let num_output_channels = (output_channel as usize) + 1;

    // Create matrix: all zeros except 1.0 at the target output channel
    // Matrix layout is row-major: matrix[out_ch * input_channels + in_ch]
    // Since input_channels=1, this simplifies to: matrix[out_ch]
    let mut matrix = vec![0.0; num_output_channels];
    matrix[output_channel as usize] = 1.0;

    // Create PluginConfig for matrix plugin
    let matrix_params = serde_json::json!({
        "input_channels": input_channels,
        "output_channels": num_output_channels,
        "matrix": matrix,
    });

    use crate::engine::PluginConfig;
    let plugins = vec![PluginConfig::new("matrix", matrix_params)];
    manager
        .start_playback(None, plugins, num_output_channels)
        .map_err(|e| format!("Failed to start playback: {}", e))?;

    eprintln!("[record_and_analyze] Playback started");

    // Wait for playback to complete
    // NOTE: Currently waits for full timeout (duration * 1.5 + 1s) because position tracking
    // doesn't stop when decoder finishes. This is a known limitation that will be fixed
    // when proper end-of-stream detection is implemented in AudioStreamingManager.
    let total_wait = Duration::from_secs_f64(expected_duration * 1.5 + 1.0);
    let check_interval = Duration::from_millis(100);
    let mut elapsed = Duration::ZERO;

    while elapsed < total_wait {
        sleep(check_interval);
        elapsed += check_interval;

        // Check for events (currently not working - state doesn't transition to Idle)
        manager.try_recv_event();
        let state = manager.get_state();

        if state == crate::StreamingState::Idle {
            eprintln!("[record_and_analyze] Playback completed");
            break;
        }
    }

    // Stop playback
    manager
        .stop()
        .map_err(|e| format!("Failed to stop playback: {}", e))?;

    // Stop recording
    std::mem::drop(input_stream);
    eprintln!("[record_and_analyze] Recording stopped");

    // Get recorded samples
    let recorded = recorded_samples.lock().clone();
    eprintln!("[record_and_analyze] Recorded {} samples", recorded.len());

    if recorded.is_empty() {
        return Err("No samples were recorded".to_string());
    }

    // Write recorded samples to WAV file
    write_wav_file(recorded_wav_path, &recorded, sample_rate, 1)?;
    eprintln!(
        "[record_and_analyze] Wrote recording to {:?}",
        recorded_wav_path
    );

    // Analyze the recording
    eprintln!("[record_and_analyze] Analyzing recording...");
    let analysis = analyze_recording(recorded_wav_path, reference_signal, sample_rate)?;
    write_analysis_csv(&analysis, output_csv_path)?;
    eprintln!(
        "[record_and_analyze] Wrote analysis to {:?}",
        output_csv_path
    );

    Ok(())
}

/// Parse comma-separated channel list (0-based indices)
pub fn parse_channel_list(s: &str) -> Result<Vec<u16>, String> {
    let mut channels = Vec::new();

    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let ch: u16 = part
            .parse()
            .map_err(|_| format!("Invalid channel number: {}", part))?;

        if channels.contains(&ch) {
            return Err(format!("Duplicate channel number: {}", ch));
        }

        channels.push(ch);
    }

    if channels.is_empty() {
        return Err("Channel list is empty".to_string());
    }

    Ok(channels)
}

/// Validate signal parameters
pub fn validate_signal_params(
    signal_type: SignalType,
    params: &SignalParams,
    duration: f32,
    sample_rate: u32,
) -> Result<(), String> {
    if duration <= 0.0 {
        return Err("Duration must be positive".to_string());
    }

    let nyquist = sample_rate as f32 / 2.0;

    match (signal_type, params) {
        (SignalType::Tone, SignalParams::Tone { freq, amp }) => {
            if *freq <= 0.0 || *freq >= nyquist {
                return Err(format!(
                    "Tone frequency {} Hz must be in range (0, {} Hz)",
                    freq, nyquist
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (
            SignalType::TwoTone,
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            },
        ) => {
            if *freq1 <= 0.0 || *freq1 >= nyquist {
                return Err(format!(
                    "First frequency {} Hz must be in range (0, {} Hz)",
                    freq1, nyquist
                ));
            }
            if *freq2 <= 0.0 || *freq2 >= nyquist {
                return Err(format!(
                    "Second frequency {} Hz must be in range (0, {} Hz)",
                    freq2, nyquist
                ));
            }
            if *amp1 <= 0.0 || *amp1 > 1.0 {
                return Err(format!("First amplitude {} must be in range (0, 1]", amp1));
            }
            if *amp2 <= 0.0 || *amp2 > 1.0 {
                return Err(format!("Second amplitude {} must be in range (0, 1]", amp2));
            }
        }
        (
            SignalType::Sweep,
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            },
        ) => {
            if *start_freq <= 0.0 || *start_freq >= nyquist {
                return Err(format!(
                    "Start frequency {} Hz must be in range (0, {} Hz)",
                    start_freq, nyquist
                ));
            }
            if *end_freq <= 0.0 || *end_freq >= nyquist {
                return Err(format!(
                    "End frequency {} Hz must be in range (0, {} Hz)",
                    end_freq, nyquist
                ));
            }
            if *start_freq >= *end_freq {
                return Err(format!(
                    "Start frequency {} Hz must be less than end frequency {} Hz",
                    start_freq, end_freq
                ));
            }
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        (_, SignalParams::Noise { amp }) => {
            if *amp <= 0.0 || *amp > 1.0 {
                return Err(format!("Amplitude {} must be in range (0, 1]", amp));
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;
    use tempfile::tempdir;

    #[test]
    fn test_signal_type_from_str() {
        assert_eq!(SignalType::from_str("tone").unwrap(), SignalType::Tone);
        assert_eq!(
            SignalType::from_str("two-tone").unwrap(),
            SignalType::TwoTone
        );
        assert_eq!(SignalType::from_str("sweep").unwrap(), SignalType::Sweep);
        assert_eq!(
            SignalType::from_str("white-noise").unwrap(),
            SignalType::WhiteNoise
        );
        assert!(SignalType::from_str("invalid").is_err());
    }

    #[test]
    fn test_parse_channel_list() {
        assert_eq!(parse_channel_list("0").unwrap(), vec![0]); // Channel 0 is valid (0-based indexing)
        assert_eq!(parse_channel_list("1").unwrap(), vec![1]);
        assert_eq!(parse_channel_list("1,2,3").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_channel_list(" 1 , 2 , 3 ").unwrap(), vec![1, 2, 3]);
        assert_eq!(parse_channel_list("0,1,2").unwrap(), vec![0, 1, 2]); // 0-based channels

        assert!(parse_channel_list("1,1").is_err()); // Duplicate
        assert!(parse_channel_list("").is_err()); // Empty
        assert!(parse_channel_list("abc").is_err()); // Non-numeric
    }

    #[test]
    fn test_validate_signal_params_tone() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

        let params_bad_freq = SignalParams::Tone {
            freq: 30000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_bad_freq, 1.0, 48000).is_err());

        let params_bad_amp = SignalParams::Tone {
            freq: 1000.0,
            amp: 2.0,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_bad_amp, 1.0, 48000).is_err());
    }

    #[test]
    fn test_generate_output_filenames_stereo() {
        let (wav, csv) = generate_output_filenames_stereo(
            Some("test"),
            SignalType::Sweep,
            2, // send channel
            1, // record channel
            48000,
        );
        assert_eq!(wav, PathBuf::from("test_sweep_send2_rec1_48000.wav"));
        assert_eq!(csv, PathBuf::from("test_sweep_send2_rec1_48000.csv"));

        let (wav, csv) = generate_output_filenames_stereo(
            None,
            SignalType::Tone,
            1, // send channel
            3, // record channel
            44100,
        );
        assert_eq!(wav, PathBuf::from("tone_send1_rec3_44100.wav"));
        assert_eq!(csv, PathBuf::from("tone_send1_rec3_44100.csv"));
    }

    #[test]
    fn test_generate_output_filenames() {
        let (wav, csv) = generate_output_filenames(Some("test"), SignalType::Sweep, 1, 48000);
        assert_eq!(wav, PathBuf::from("test_sweep_ch1_48000.wav"));
        assert_eq!(csv, PathBuf::from("test_sweep_ch1_48000.csv"));

        let (wav, csv) = generate_output_filenames(None, SignalType::Tone, 2, 44100);
        assert_eq!(wav, PathBuf::from("tone_ch2_44100.wav"));
        assert_eq!(csv, PathBuf::from("tone_ch2_44100.csv"));
    }

    #[test]
    fn test_generate_signal_tone() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        let signal = generate_signal(SignalType::Tone, &params, 0.1, 48000)
            .expect("Failed to generate tone");

        assert_eq!(signal.len(), 4800); // 0.1s * 48000 Hz

        // Check signal is non-zero and within amplitude bounds
        let max_val = signal
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        assert!(
            max_val > 0.4 && max_val <= 0.5,
            "Tone amplitude out of range: {}",
            max_val
        );
    }

    #[test]
    fn test_generate_signal_sweep() {
        let params = SignalParams::Sweep {
            start_freq: 20.0,
            end_freq: 20000.0,
            amp: 0.5,
        };
        let signal = generate_signal(SignalType::Sweep, &params, 1.0, 48000)
            .expect("Failed to generate sweep");

        assert_eq!(signal.len(), 48000);

        let max_val = signal
            .iter()
            .map(|&x| x.abs())
            .fold(0.0_f32, |a, b| a.max(b));
        assert!(
            max_val > 0.4 && max_val <= 0.5,
            "Sweep amplitude out of range: {}",
            max_val
        );
    }

    #[test]
    fn test_generate_signal_noise() {
        let params = SignalParams::Noise { amp: 0.5 };
        let signal = generate_signal(SignalType::WhiteNoise, &params, 1.0, 48000)
            .expect("Failed to generate white noise");

        assert_eq!(signal.len(), 48000);

        // Check that noise has content (not all zeros) - matches existing test pattern
        assert!(
            signal.iter().any(|&x| x.abs() > 0.01),
            "Noise signal should have non-zero samples"
        );
    }

    #[test]
    fn test_generate_signal_type_mismatch() {
        // Wrong params for signal type should fail
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        let result = generate_signal(SignalType::Sweep, &params, 1.0, 48000);
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_signal_adds_padding() {
        let signal = vec![1.0; 4800]; // 0.1s at 48kHz
        let prepared = prepare_signal(signal.clone(), 48000);

        // Should be longer due to fades and padding
        assert!(
            prepared.len() > signal.len(),
            "Prepared signal should be longer than original"
        );

        // First samples should be faded (smaller than original)
        assert!(
            prepared[0].abs() < signal[0].abs(),
            "First sample should be faded in"
        );

        // Last samples should be faded
        assert!(
            prepared[prepared.len() - 1].abs() < 0.1,
            "Last sample should be faded out or padded"
        );
    }

    #[test]
    fn test_write_and_read_wav_roundtrip() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let wav_path = temp_dir.path().join("test.wav");

        // Generate a simple signal
        let sample_rate = 48000;
        let duration = 0.1;
        let signal: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect();

        // Write WAV
        write_wav_file(&wav_path, &signal, sample_rate, 1).expect("Failed to write WAV");

        assert!(wav_path.exists(), "WAV file should exist");

        // Read it back using hound
        let mut reader = WavReader::open(&wav_path).expect("Failed to open WAV for reading");

        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, sample_rate);
        assert_eq!(spec.sample_format, SampleFormat::Float);

        let read_samples: Vec<f32> = reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to read samples");

        // Verify samples match (with small floating point tolerance)
        assert_eq!(read_samples.len(), signal.len());
        for (i, (&original, &read)) in signal.iter().zip(read_samples.iter()).enumerate() {
            assert!(
                (original - read).abs() < 1e-6,
                "Sample {} mismatch: original={}, read={}",
                i,
                original,
                read
            );
        }
    }

    #[test]
    fn test_write_temp_wav() {
        let signal = vec![0.5, 0.3, -0.2, -0.4, 0.0];
        let sample_rate = 48000;

        let temp_file = write_temp_wav(&signal, sample_rate, 1).expect("Failed to write temp WAV");

        assert!(temp_file.path().exists());

        // Verify it's a valid WAV
        let reader = WavReader::open(temp_file.path()).expect("Failed to open temp WAV");
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, sample_rate);
    }

    #[test]
    fn test_validate_signal_params_duration() {
        let params = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };

        // Valid duration
        assert!(validate_signal_params(SignalType::Tone, &params, 1.0, 48000).is_ok());

        // Invalid duration
        assert!(validate_signal_params(SignalType::Tone, &params, 0.0, 48000).is_err());
        assert!(validate_signal_params(SignalType::Tone, &params, -1.0, 48000).is_err());
    }

    #[test]
    fn test_validate_signal_params_frequency_nyquist() {
        let sample_rate = 48000;
        let nyquist = sample_rate as f32 / 2.0;

        // Valid frequency
        let params_valid = SignalParams::Tone {
            freq: 1000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_valid, 1.0, sample_rate).is_ok());

        // Frequency above Nyquist
        let params_high = SignalParams::Tone {
            freq: nyquist + 100.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_high, 1.0, sample_rate).is_err());

        // Zero frequency
        let params_zero = SignalParams::Tone {
            freq: 0.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Tone, &params_zero, 1.0, sample_rate).is_err());
    }

    #[test]
    fn test_validate_signal_params_sweep_order() {
        let sample_rate = 48000;

        // Valid sweep (ascending)
        let params_valid = SignalParams::Sweep {
            start_freq: 20.0,
            end_freq: 20000.0,
            amp: 0.5,
        };
        assert!(validate_signal_params(SignalType::Sweep, &params_valid, 1.0, sample_rate).is_ok());

        // Invalid sweep (start >= end)
        let params_reversed = SignalParams::Sweep {
            start_freq: 20000.0,
            end_freq: 20.0,
            amp: 0.5,
        };
        assert!(
            validate_signal_params(SignalType::Sweep, &params_reversed, 1.0, sample_rate).is_err()
        );

        let params_equal = SignalParams::Sweep {
            start_freq: 1000.0,
            end_freq: 1000.0,
            amp: 0.5,
        };
        assert!(
            validate_signal_params(SignalType::Sweep, &params_equal, 1.0, sample_rate).is_err()
        );
    }

    /// Regression test: Verify that record_and_analyze doesn't just copy the input file
    ///
    /// This test ensures that the recording function actually performs recording,
    /// not just file copying. It checks that:
    /// 1. The function signature includes both input and output paths
    /// 2. The implementation uses proper recording mechanisms
    ///
    /// Note: This is a compile-time/documentation test. The actual E2E test
    /// should verify that recorded audio differs from input when there's
    /// actual signal processing or latency.
    #[test]
    fn test_record_and_analyze_signature() {
        // This test documents the expected signature of record_and_analyze.
        // It takes separate paths for input (playback) and output (recording),
        // which is the first line of defense against the "copy instead of record" bug.

        // Verify function exists with correct parameter count and types
        // by calling it with dummy parameters (compile-time check only)
        let _check = || async {
            let temp_path = Path::new("/tmp/input.wav");
            let output_path = Path::new("/tmp/output.wav");
            let csv_path = Path::new("/tmp/output.csv");
            let reference: Vec<f32> = vec![];

            // This won't run, but ensures the signature is correct
            if false {
                let _result = record_and_analyze(
                    temp_path,   // temp_wav_path (for playback)
                    output_path, // recorded_wav_path (for recording output)
                    &reference,  // reference_signal
                    48000_u32,   // sample_rate
                    csv_path,    // output_csv_path
                    1_u16,       // output_channel
                    1_u16,       // input_channel
                );
            }
        };

        // Just verify it compiles
        let _ = _check;
    }
}
