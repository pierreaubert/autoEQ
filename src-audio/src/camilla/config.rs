// ============================================================================
// Config Generation
// ============================================================================

use serde_yaml::{Mapping, Value};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

use super::errors::{CamillaError, CamillaResult};
use super::types::{
    CamillaDSPConfig, CaptureDevice, ChannelsSetting, DeviceConfig, PlaybackDevice,
};
use crate::filters::FilterParams;
use crate::loudness_compensation::LoudnessCompensation;

/// Generate a CamillaDSP config for streaming playback with stdin input
pub fn generate_streaming_config(
    output_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    filters: &[FilterParams],
    map_mode: super::types::ChannelMapMode,
    output_map: Option<&[u16]>,
    loudness: Option<&LoudnessCompensation>,
) -> CamillaResult<CamillaDSPConfig> {
    // Validate all filters
    for filter in filters {
        filter.validate()?;
    }

    // Detect device native sample rate for proper resampling
    let device_sample_rate = get_device_native_sample_rate(output_device).unwrap_or(sample_rate); // Fallback to file rate if detection fails

    let needs_resampling = sample_rate != device_sample_rate;

    if needs_resampling {
        println!(
            "[CamillaDSP] Sample rate mismatch detected: file={}Hz, device={}Hz. Adding resampler.",
            sample_rate, device_sample_rate
        );
    }

    // Create capture device (stdin input)
    let capture = CaptureDevice {
        device_type: "Stdin".to_string(),
        device: None,
        filename: None,
        channels: None, // Stdin capture doesn't need channels - it's in the stream format
        format: Some("FLOAT32LE".to_string()), // Our decoder outputs f32 samples
    };

    // Create playback device
    let (playback_type, device_name) = map_output_device(output_device)?;
    // Prepare output channel_map if provided
    let effective_output_map: Option<Vec<u16>> = if let Some(map) = output_map {
        if map.len() as u16 >= channels {
            // Use the last `channels` entries to select L/R, as often used for dedicated output pairs
            let start = map.len() - channels as usize;
            Some(map[start..].to_vec())
        } else {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Output channel_map length ({}) must be >= channels ({})",
                map.len(),
                channels
            )));
        }
    } else {
        None
    };

    // Determine total number of output channels required
    let mixer_out_channels: u16 = if let Some(ref outs) = effective_output_map {
        outs.iter().copied().max().unwrap_or(1) + 1
    } else {
        channels
    };

    let playback = PlaybackDevice {
        device_type: playback_type,
        device: device_name,
        filename: None,
        channels: Some(ChannelsSetting::Count(mixer_out_channels)), // Restore individual device channels
        format: None,     // Let CoreAudio use default format
        wav_header: None, // Not applicable for CoreAudio playback
    };

    // Generate resampler config if needed
    let resampler_config = if needs_resampling {
        // For streaming from Stdin, Camilla doesn't know the source rate.
        // Use Synchronous resampler and provide explicit in/out rates.
        let resampler_yaml = format!(
            r#"
            type: Synchronous
            in_rate: {in_rate}
            out_rate: {out_rate}
            "#,
            in_rate = sample_rate,
            out_rate = device_sample_rate
        );
        Some(
            serde_yaml::from_str::<serde_yaml::Value>(&resampler_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!("Failed to generate resampler: {}", e))
            })?,
        )
    } else {
        None
    };

    let devices = DeviceConfig {
        samplerate: device_sample_rate, // Use device's native rate (output side)
        chunksize: 1024,
        silence_threshold: Some(-60),
        silence_timeout: Some(3.0),
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(false),
        resampler: resampler_config,
    };

    // Generate filters section, optionally including Loudness filter
    let filters_section = {
        let mut fm = serde_yaml::Mapping::new();

        if let Some(lc) = loudness {
            let loud_yaml = format!(
                r#"
                loudness:
                  type: Loudness
                  parameters:
                    fader: Main
                    reference_level: {ref_level}
                    high_boost: {high}
                    low_boost: {low}
                    attenuate_mid: {atten}
                "#,
                ref_level = lc.reference_level,
                high = lc.high_boost,
                low = lc.low_boost,
                atten = if lc.attenuate_mid { "true" } else { "false" }
            );
            let v: serde_yaml::Value = serde_yaml::from_str(&loud_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!(
                    "Failed to generate loudness filter: {}",
                    e
                ))
            })?;
            if let serde_yaml::Value::Mapping(m) = v {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if !filters.is_empty() || loudness.is_some() {
            let peq_val = generate_filters_yaml(filters, loudness)?;
            if let serde_yaml::Value::Mapping(m) = peq_val {
                for (k, val) in m {
                    fm.insert(k, val);
                }
            }
        }

        if fm.is_empty() {
            None
        } else {
            Some(serde_yaml::Value::Mapping(fm))
        }
    };

    // Build destination map per input channel
    let dest_map: Vec<u16> = {
        let mut base: Vec<u16> = (0..channels).collect();
        if let Some(ref outs) = effective_output_map {
            if outs.len() as u16 >= channels {
                let start = outs.len() - channels as usize;
                base = outs[start..].to_vec();
            }
        }
        // Optional swap for first two channels
        if map_mode == super::types::ChannelMapMode::Swap && channels >= 2 {
            let mut swapped = base.clone();
            swapped.swap(0, 1);
            swapped
        } else {
            base
        }
    };

    // Generate mixers section (matrix routing)
    let mixers_section = Some(generate_matrix_mixer_yaml(
        channels,
        mixer_out_channels,
        &dest_map,
    ));

    // Generate pipeline - always include mixer; add filters and resampler if needed
    let pipeline = Some(generate_pipeline(
        mixer_out_channels,
        filters,
        needs_resampling,
        loudness.is_some(),
        loudness,
    ));

    let config = CamillaDSPConfig {
        devices,
        filters: filters_section,
        mixers: mixers_section,
        pipeline,
    };

    // Debug: Print generated config
    let yaml_str = serde_yaml::to_string(&config).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to serialize config: {}", e))
    })?;
    println!("[CamillaDSP] Generated streaming YAML:");
    println!("--- STREAMING CONFIG START ---");
    println!("{}", yaml_str);
    println!("--- STREAMING CONFIG END ---");

    Ok(config)
}

/// Generate a CamillaDSP config for file playback with EQ filters
pub fn generate_playback_config(
    audio_file: &std::path::Path,
    output_device: Option<&str>,
    sample_rate: u32,
    num_channels: u16,
) -> CamillaResult<CamillaDSPConfig> {
    let capture = CaptureDevice {
        device_type: "WavFile".to_string(), // CamillaDSP v3 uses "WavFile" for WAV file input
        filename: Some(audio_file.to_str().unwrap().to_string()),
        device: None,
        format: None,
        channels: None, // File capture doesn't need channels - it's embedded in the file
    };

    let (playback_type, device_name) = map_output_device(output_device)?;
    let playback = PlaybackDevice {
        device_type: playback_type,
        device: device_name,
        filename: None,
        format: None,
        wav_header: None,
        channels: Some(ChannelsSetting::Count(num_channels)), // Restore individual device channels
    };

    // Detect device native sample rate for file playback too
    let device_sample_rate = get_device_native_sample_rate(output_device).unwrap_or(sample_rate);
    let needs_resampling = sample_rate != device_sample_rate;

    if needs_resampling {
        println!(
            "[CamillaDSP] Sample rate mismatch detected: file={}Hz, device={}Hz. Adding resampler.",
            sample_rate, device_sample_rate
        );
    }

    // Generate resampler config if needed
    let resampler_config = if needs_resampling {
        // CamillaDSP v3 expects explicit parameters per resampler type.
        // Use AsyncPoly with linear interpolation for broad compatibility.
        let resampler_yaml = r#"
            type: AsyncPoly
            interpolation: Linear
        "#;
        Some(
            serde_yaml::from_str::<serde_yaml::Value>(resampler_yaml).map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!("Failed to generate resampler: {}", e))
            })?,
        )
    } else {
        None
    };

    let devices = DeviceConfig {
        // Use the requested playback sample rate in the config; add a resampler if needed
        samplerate: sample_rate,
        chunksize: 1024,
        silence_threshold: Some(-60),
        silence_timeout: Some(3.0),
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
        resampler: resampler_config,
    };

    println!("[CamillaDSP Playback Config]");
    println!("  Input channels: {}", num_channels);
    println!("  Output channels: {}", num_channels);

    let config = CamillaDSPConfig {
        devices,
        filters: None,
        mixers: None,
        pipeline: None,
    };

    // Debug: Print generated config
    let yaml_str = serde_yaml::to_string(&config).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to serialize config: {}", e))
    })?;
    println!("[CamillaDSP] Generated playback YAML:");
    println!("--- PLAYBACK CONFIG START ---");
    println!("{}", yaml_str);
    println!("--- PLAYBACK CONFIG END ---");

    Ok(config)
}

/// Convert raw FLOAT32LE audio to WAV format using hound
pub fn convert_raw_to_wav(
    raw_path: &std::path::Path,
    wav_path: &std::path::Path,
    sample_rate: u32,
    channels: u16,
) -> CamillaResult<()> {
    use std::fs::File;
    use std::io::BufReader;

    println!(
        "[CamillaDSP] Converting raw audio to WAV: {:?} -> {:?}",
        raw_path, wav_path
    );

    // Read raw FLOAT32LE data
    let raw_file = File::open(raw_path)
        .map_err(|e| CamillaError::IOError(format!("Failed to open raw file: {}", e)))?;
    let mut reader = BufReader::new(raw_file);
    let mut raw_data = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut raw_data)
        .map_err(|e| CamillaError::IOError(format!("Failed to read raw data: {}", e)))?;

    // Interpret as f32 samples
    let sample_count = raw_data.len() / 4;
    let mut samples = Vec::with_capacity(sample_count);
    for chunk in raw_data.chunks_exact(4) {
        let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
        samples.push(f32::from_le_bytes(bytes));
    }

    // Write WAV file using hound
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut wav_writer = hound::WavWriter::create(wav_path, spec)
        .map_err(|e| CamillaError::IOError(format!("Failed to create WAV writer: {}", e)))?;

    // Write all samples
    for &sample in &samples {
        wav_writer
            .write_sample(sample)
            .map_err(|e| CamillaError::IOError(format!("Failed to write sample: {}", e)))?;
    }

    wav_writer
        .finalize()
        .map_err(|e| CamillaError::IOError(format!("Failed to finalize WAV file: {}", e)))?;

    println!(
        "[CamillaDSP] WAV conversion complete: {} samples, {}Hz, {}ch",
        sample_count, sample_rate, channels
    );
    Ok(())
}

/// Fix RF64 WAV files written by CamillaDSP by updating size fields
/// CamillaDSP writes 0xFFFFFFFF placeholders which hound doesn't accept
pub fn fix_rf64_wav(wav_path: &std::path::Path) -> CamillaResult<()> {
    use std::fs::OpenOptions;
    use std::io::{Read, Seek, SeekFrom, Write};

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(wav_path)
        .map_err(|e| CamillaError::IOError(format!("Failed to open WAV file: {}", e)))?;

    // Get file size
    let file_size = file
        .metadata()
        .map_err(|e| CamillaError::IOError(format!("Failed to get file size: {}", e)))?
        .len() as u32;

    // Read RIFF header
    let mut riff_header = [0u8; 12];
    file.read_exact(&mut riff_header)
        .map_err(|e| CamillaError::IOError(format!("Failed to read RIFF header: {}", e)))?;

    // Check if it's a RIFF/WAVE file
    if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
        return Ok(()); // Not a RIFF WAV, skip
    }

    // Check if RIFF size is 0xFFFFFFFF (RF64 placeholder)
    let riff_size = u32::from_le_bytes([
        riff_header[4],
        riff_header[5],
        riff_header[6],
        riff_header[7],
    ]);
    if riff_size == 0xFFFFFFFF {
        // Fix RIFF chunk size (file_size - 8)
        let correct_riff_size = file_size - 8;
        file.seek(SeekFrom::Start(4))
            .map_err(|e| CamillaError::IOError(format!("Failed to seek: {}", e)))?;
        file.write_all(&correct_riff_size.to_le_bytes())
            .map_err(|e| CamillaError::IOError(format!("Failed to write RIFF size: {}", e)))?;

        println!(
            "[CamillaDSP] Fixed RF64 RIFF size: {} -> {}",
            riff_size, correct_riff_size
        );
    }

    // Seek to find data chunk
    file.seek(SeekFrom::Start(12))
        .map_err(|e| CamillaError::IOError(format!("Failed to seek to chunks: {}", e)))?;

    loop {
        let mut chunk_header = [0u8; 8];
        if file.read_exact(&mut chunk_header).is_err() {
            break; // End of file
        }

        let chunk_id = &chunk_header[0..4];
        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]);

        if chunk_id == b"data" && chunk_size == 0xFFFFFFFF {
            // Fix data chunk size
            let current_pos = file
                .stream_position()
                .map_err(|e| CamillaError::IOError(format!("Failed to get position: {}", e)))?;
            let correct_data_size = file_size - current_pos as u32;

            file.seek(SeekFrom::Start(current_pos - 4)).map_err(|e| {
                CamillaError::IOError(format!("Failed to seek to data size: {}", e))
            })?;
            file.write_all(&correct_data_size.to_le_bytes())
                .map_err(|e| CamillaError::IOError(format!("Failed to write data size: {}", e)))?;

            println!(
                "[CamillaDSP] Fixed RF64 data size: {} -> {}",
                chunk_size, correct_data_size
            );
            break;
        }

        // Skip to next chunk
        if chunk_size != 0xFFFFFFFF {
            file.seek(SeekFrom::Current(chunk_size as i64))
                .map_err(|e| CamillaError::IOError(format!("Failed to skip chunk: {}", e)))?;
        }
    }

    Ok(())
}

/// Recording output types
#[derive(Debug, Clone)]
pub enum RecordingOutputType {
    WavFile(std::path::PathBuf),
    StdOut,
    RawFile(std::path::PathBuf),
}

/// Generate a CamillaDSP config for recording with WAV file output
pub fn generate_recording_config(
    input_device: Option<&str>,
    output_file: &std::path::Path,
    sample_rate: u32,
    channels: u16,
    input_map: Option<&[u16]>,
) -> CamillaResult<CamillaDSPConfig> {
    generate_recording_config_with_output_type(
        RecordingOutputType::WavFile(output_file.to_path_buf()),
        input_device,
        sample_rate,
        channels,
        input_map,
    )
}

/// Generate a CamillaDSP config for recording with flexible output types
pub fn generate_recording_config_with_output_type(
    output_type: RecordingOutputType,
    input_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    input_map: Option<&[u16]>,
) -> CamillaResult<CamillaDSPConfig> {
    // Create capture device (audio input)
    let (capture_type, device_name) = map_input_device(input_device)?;
    println!(
        "[CamillaDSP Recording] Input device: type={}, name={:?}",
        capture_type, device_name
    );

    let capture = CaptureDevice {
        device_type: capture_type,
        device: device_name,
        channels: Some(ChannelsSetting::Count(channels)), // Restore individual device channels
        filename: None,
        format: None,
    };

    // Create playback device based on output type
    let playback = match output_type {
        RecordingOutputType::WavFile(ref path) => PlaybackDevice {
            device_type: "File".to_string(), // CamillaDSP v3 uses "File" for file output
            device: None,
            filename: Some(path.to_string_lossy().to_string()),
            channels: Some(ChannelsSetting::Count(channels)), // Restore individual device channels
            format: Some("S32LE".to_string()), // Use S32LE instead of FLOAT32LE to avoid CamillaDSP double-header bug
            wav_header: Some(true),            // Enable WAV header for WAV file output
        },
        RecordingOutputType::StdOut => PlaybackDevice {
            device_type: "Stdout".to_string(), // "Stdout" is correct for stdout output
            device: None,
            filename: None,
            channels: Some(ChannelsSetting::Count(channels)), // Restore individual device channels
            format: Some("FLOAT32LE".to_string()),
            wav_header: None, // No WAV header for stdout
        },
        RecordingOutputType::RawFile(ref path) => PlaybackDevice {
            device_type: "File".to_string(), // CamillaDSP v3 uses "File" for file output
            device: None,
            filename: Some(path.to_string_lossy().to_string()),
            channels: Some(ChannelsSetting::Count(channels)), // Restore individual device channels
            format: Some("FLOAT32LE".to_string()),
            wav_header: Some(false), // No WAV header for raw file
        },
    };

    let devices = DeviceConfig {
        samplerate: sample_rate,
        chunksize: 1024,
        silence_threshold: Some(-60), // More sensitive threshold for recording
        silence_timeout: Some(3.0),
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
        resampler: None,
    };

    println!("[CamillaDSP Recording] Configuration:");
    println!("  Input device: {:?}", input_device);
    println!("  Output type: {:?}", output_type);
    println!("  Sample rate: {}", sample_rate);
    println!("  Channels: {}", channels);
    println!("  Input map: {:?}", input_map);

    let config = CamillaDSPConfig {
        devices,
        filters: None,
        mixers: None,
        pipeline: None,
    };

    // Debug: Print generated config
    let yaml_str = serde_yaml::to_string(&config).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to serialize config: {}", e))
    })?;
    println!("[CamillaDSP] Generated recording YAML:");
    println!("--- RECORDING CONFIG START ---");
    println!("{}", yaml_str);
    println!("--- RECORDING CONFIG END ---");

    Ok(config)
}

/// Get the native sample rate of an output device
pub fn get_device_native_sample_rate(device_name: Option<&str>) -> Option<u32> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();

    let device = if let Some(name) = device_name {
        // Find device by name
        host.output_devices()
            .ok()?
            .find(|d| d.name().ok().as_deref() == Some(name))
    } else {
        // Use default device
        host.default_output_device()
    };

    let device = device?;
    let config = device.default_output_config().ok()?;
    Some(config.sample_rate().0)
}

/// Map output device name to CamillaDSP format
pub fn map_output_device(device: Option<&str>) -> CamillaResult<(String, Option<String>)> {
    match device {
        None => {
            // Use default device for the platform
            #[cfg(target_os = "macos")]
            return Ok(("CoreAudio".to_string(), None));

            #[cfg(target_os = "linux")]
            return Ok(("Alsa".to_string(), Some("default".to_string())));

            #[cfg(target_os = "windows")]
            return Ok(("Wasapi".to_string(), None));

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            return Err(CamillaError::InvalidConfiguration(
                "Unsupported platform".to_string(),
            ));
        }
        Some(name) => {
            #[cfg(target_os = "macos")]
            return Ok(("CoreAudio".to_string(), Some(name.to_string())));

            #[cfg(target_os = "linux")]
            return Ok(("Alsa".to_string(), Some(name.to_string())));

            #[cfg(target_os = "windows")]
            return Ok(("Wasapi".to_string(), Some(name.to_string())));

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            return Err(CamillaError::InvalidConfiguration(
                "Unsupported platform".to_string(),
            ));
        }
    }
}

/// Map input device name to CamillaDSP format
pub fn map_input_device(device: Option<&str>) -> CamillaResult<(String, Option<String>)> {
    // Same logic as output device for now
    map_output_device(device)
}

/// Generate the filters section as YAML
pub fn generate_filters_yaml(
    filters: &[FilterParams],
    loudness: Option<&LoudnessCompensation>,
) -> CamillaResult<serde_yaml::Value> {
    let mut filters_map = serde_yaml::Mapping::new();

    // Add volume gain filter if loudness compensation is active
    // This reduces the overall volume to prevent clipping when loudness boosts frequencies
    if let Some(lc) = loudness {
        // Calculate attenuation based on the amount of loudness compensation
        // Use a more precise calculation: attenuate by about 70% of the boost amount
        // This accounts for the fact that loudness affects frequency ranges, not the entire signal
        let max_boost = lc.low_boost.max(lc.high_boost);
        // Apply -7dB of attenuation for every 10dB of boost to prevent clipping
        // More conservative to handle various music content
        let attenuation_db = -(max_boost * 0.7);

        let volume_filter_name = "volume_attenuation".to_string();

        // Use a Biquad filter with Peaking type for overall gain
        let mut params = serde_yaml::Mapping::new();
        params.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Peaking".to_string()),
        );
        params.insert(
            serde_yaml::Value::String("freq".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(1000i64)),
        );
        params.insert(
            serde_yaml::Value::String("gain".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(attenuation_db as i64)),
        );
        params.insert(
            serde_yaml::Value::String("q".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(0.1f64)),
        );

        let mut filter_config = serde_yaml::Mapping::new();
        filter_config.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Biquad".to_string()),
        );
        filter_config.insert(
            serde_yaml::Value::String("parameters".to_string()),
            serde_yaml::Value::Mapping(params),
        );

        filters_map.insert(
            serde_yaml::Value::String(volume_filter_name),
            serde_yaml::Value::Mapping(filter_config),
        );

        println!(
            "[CamillaDSP] Added volume attenuation filter: {:.1}dB (compensation: {:.1}/{:.1})",
            attenuation_db, lc.low_boost, lc.high_boost
        );
    }

    for (idx, filter) in filters.iter().enumerate() {
        let filter_name = format!("peq{}", idx + 1);

        let mut params = serde_yaml::Mapping::new();
        params.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String(filter.filter_type.clone()),
        );
        params.insert(
            serde_yaml::Value::String("freq".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.frequency as i64)),
        );
        params.insert(
            serde_yaml::Value::String("gain".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.gain as i64)),
        );
        params.insert(
            serde_yaml::Value::String("q".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(filter.q as i64)),
        );

        let mut filter_config = serde_yaml::Mapping::new();
        filter_config.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Biquad".to_string()),
        );
        filter_config.insert(
            serde_yaml::Value::String("parameters".to_string()),
            serde_yaml::Value::Mapping(params),
        );

        filters_map.insert(
            serde_yaml::Value::String(filter_name),
            serde_yaml::Value::Mapping(filter_config),
        );
    }

    Ok(serde_yaml::Value::Mapping(filters_map))
}

/// Generate a stereo mixer configuration
/// Generate a generic N-in / M-out matrix mixer configuration
/// mapping[i] gives the destination output channel index for input channel i
pub fn generate_matrix_mixer_yaml(
    in_channels: u16,
    out_channels: u16,
    mapping: &[u16],
) -> serde_yaml::Value {
    // Build YAML mapping entries
    // For each destination output channel, collect sources referencing it
    let mut dest_sources: Vec<Vec<u16>> = vec![Vec::new(); out_channels as usize];
    for (src, &dst) in mapping.iter().enumerate() {
        if (dst as usize) < dest_sources.len() {
            dest_sources[dst as usize].push(src as u16);
        }
    }

    // Compose YAML string manually to keep dependency surface small
    let mut s = String::new();
    s.push_str("matrix_mixer:\n");
    s.push_str("  channels:\n");
    s.push_str(&format!("    in: {}\n", in_channels));
    s.push_str(&format!("    out: {}\n", out_channels));
    s.push_str("  mapping:\n");

    for (dest, sources) in dest_sources.iter().enumerate() {
        s.push_str(&format!("    - dest: {}\n", dest));
        s.push_str("      sources:\n");
        if sources.is_empty() {
            // Leave empty sources list (silence on this destination)
            s.push_str("        []\n");
        } else {
            for &src in sources {
                s.push_str(&format!(
                    "        - channel: {}\n          gain: 1.0\n          inverted: false\n",
                    src
                ));
            }
        }
    }

    serde_yaml::from_str::<serde_yaml::Value>(&s).unwrap()
}

/// Generate the pipeline
/// Note: Resampler is NOT added to pipeline when using devices.resampler - it's automatic
pub fn generate_pipeline(
    _channels: u16,
    filters: &[FilterParams],
    _needs_resampling: bool,
    include_loudness: bool,
    loudness: Option<&LoudnessCompensation>,
) -> Vec<serde_yaml::Value> {
    let mut pipeline: Vec<Value> = Vec::new();

    // Always add mixer with singular `name` as required by CamillaDSP v3
    let mut mixer_map = Mapping::new();
    mixer_map.insert(
        Value::String("type".to_string()),
        Value::String("Mixer".to_string()),
    );
    mixer_map.insert(
        Value::String("name".to_string()),
        Value::String("matrix_mixer".to_string()),
    );
    pipeline.push(Value::Mapping(mixer_map));

    let mut filter_names: Vec<Value> = Vec::new();

    // Optional Loudness filter
    if include_loudness {
        filter_names.push(Value::String("loudness".to_string()));
    }

    // Add volume attenuation filter if loudness compensation is active
    if loudness.is_some() {
        filter_names.push(Value::String("volume_attenuation".to_string()));
    }

    // Add all PEQ filters
    for (idx, _filter) in filters.iter().enumerate() {
        filter_names.push(Value::String(format!("peq{}", idx + 1)));
    }

    // Add a single Filter step with all the filter names
    if !filter_names.is_empty() {
        let mut filter_map = Mapping::new();
        filter_map.insert(
            Value::String("type".to_string()),
            Value::String("Filter".to_string()),
        );
        filter_map.insert(
            Value::String("names".to_string()),
            Value::Sequence(filter_names),
        );
        pipeline.push(Value::Mapping(filter_map));
    }

    pipeline
}

/// Write a config to a temporary YAML file
pub fn write_config_to_temp(config: &CamillaDSPConfig) -> CamillaResult<NamedTempFile> {
    let mut temp_file = NamedTempFile::new().map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to create temp file: {}", e))
    })?;

    let yaml = serde_yaml::to_string(config)?;
    temp_file.write_all(yaml.as_bytes()).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to write config: {}", e))
    })?;

    temp_file.flush().map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to flush config: {}", e))
    })?;

    Ok(temp_file)
}

/// Write a config to a specific file path
pub fn write_config_to_file(config: &CamillaDSPConfig, path: &PathBuf) -> CamillaResult<()> {
    let yaml = serde_yaml::to_string(config)?;
    fs::write(path, yaml).map_err(|e| {
        CamillaError::ConfigGenerationFailed(format!("Failed to write config file: {}", e))
    })?;

    println!("[CamillaDSP] Wrote config to: {:?}", path);
    Ok(())
}

/// Validate and debug a CamillaDSP configuration
pub fn validate_config(config: &CamillaDSPConfig, config_type: &str) -> CamillaResult<()> {
    println!(
        "[CamillaDSP Debug] Validating {} configuration",
        config_type
    );

    // Check basic device configuration
    println!("  Sample rate: {}", config.devices.samplerate);
    println!("  Chunk size: {}", config.devices.chunksize);

    // Check capture device
    if let Some(ref capture) = config.devices.capture {
        println!("  Capture device:");
        println!("    Type: {}", capture.device_type);
        println!("    Device: {:?}", capture.device);
        println!("    Filename: {:?}", capture.filename);
        println!("    Channels: {:?}", capture.channels);
        println!("    Format: {:?}", capture.format);
    } else {
        println!("  No capture device configured");
    }

    // Check playback device
    println!("  Playback device:");
    println!("    Type: {}", config.devices.playback.device_type);
    println!("    Device: {:?}", config.devices.playback.device);
    println!("    Filename: {:?}", config.devices.playback.filename);
    println!("    Channels: {:?}", config.devices.playback.channels);
    println!("    Format: {:?}", config.devices.playback.format);
    println!("    WAV header: {:?}", config.devices.playback.wav_header);

    // Check for common issues
    let mut warnings = Vec::new();

    // Recording-specific validations
    if config_type == "recording" {
        if config.devices.playback.device_type == "Stdout"
            && config.devices.playback.wav_header.is_some()
        {
            warnings.push("WAV header should be None for Stdout output");
        }
        if config.devices.playback.device_type == "File"
            && config.devices.playback.filename.is_none()
        {
            warnings.push("Filename required for File output type");
        }
        if config.devices.playback.device_type == "File"
            && config.devices.playback.wav_header.is_none()
        {
            warnings.push("WAV header setting should be specified for File output");
        }
    }

    // Playback-specific validations
    if config_type == "playback" {
        if let Some(ref capture) = config.devices.capture {
            if (capture.device_type == "WavFile" || capture.device_type == "RawFile")
                && capture.filename.is_none()
            {
                warnings.push("Filename required for file input types");
            }
        }
    }

    // Print warnings
    if !warnings.is_empty() {
        println!("  Warnings:");
        for warning in warnings {
            println!("    - {}", warning);
        }
    } else {
        println!("  Configuration looks good!");
    }

    Ok(())
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::camilla::config::{
        RecordingOutputType, generate_recording_config, generate_recording_config_with_output_type,
    };
    use std::path::PathBuf;

    #[test]
    fn test_recording_config_wav_output() {
        let output_file = PathBuf::from("/tmp/record.wav");
        let config = generate_recording_config(
            None, // default input device
            &output_file,
            48000,
            1,
            None,
        )
        .unwrap();

        // Convert to YAML string for verification
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Check that the YAML contains the expected fields
        assert!(yaml.contains("samplerate: 48000"));
        assert!(yaml.contains("chunksize: 1024"));
        assert!(yaml.contains("silence_threshold: -60"));
        assert!(yaml.contains("silence_timeout: 3.0"));
        assert!(yaml.contains("type: CoreAudio"));
        assert!(yaml.contains("type: File"));
        assert!(yaml.contains("filename: /tmp/record.wav"));
        assert!(yaml.contains("channels: 1"));
        assert!(yaml.contains("format: FLOAT32LE"));
        assert!(yaml.contains("wav_header: true"));
    }

    #[test]
    fn test_recording_config_stdout_output() {
        let config = generate_recording_config_with_output_type(
            RecordingOutputType::StdOut,
            None, // default input device
            44100,
            2,
            None,
        )
        .unwrap();

        // Convert to YAML string for verification
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Check that the YAML contains the expected fields
        assert!(yaml.contains("samplerate: 44100"));
        assert!(yaml.contains("chunksize: 1024"));
        assert!(yaml.contains("silence_threshold: -60"));
        assert!(yaml.contains("silence_timeout: 3.0"));
        assert!(yaml.contains("type: CoreAudio"));
        assert!(yaml.contains("type: Stdout"));
        assert!(yaml.contains("channels: 2"));
        assert!(yaml.contains("format: FLOAT32LE"));
        // Should not contain wav_header for StdOut
        assert!(!yaml.contains("wav_header"));
    }

    #[test]
    fn test_recording_config_raw_file_output() {
        let output_file = PathBuf::from("/tmp/record.raw");
        let config = generate_recording_config_with_output_type(
            RecordingOutputType::RawFile(output_file.clone()),
            None, // default input device
            96000,
            1,
            None,
        )
        .unwrap();

        // Convert to YAML string for verification
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Check that the YAML contains the expected fields
        assert!(yaml.contains("samplerate: 96000"));
        assert!(yaml.contains("chunksize: 1024"));
        assert!(yaml.contains("silence_threshold: -60"));
        assert!(yaml.contains("silence_timeout: 3.0"));
        assert!(yaml.contains("type: CoreAudio"));
        assert!(yaml.contains("type: File"));
        assert!(yaml.contains("filename: /tmp/record.raw"));
        assert!(yaml.contains("channels: 1"));
        assert!(yaml.contains("format: FLOAT32LE"));
        assert!(yaml.contains("wav_header: false"));
    }

    #[test]
    fn test_recording_config_with_device_name() {
        let output_file = PathBuf::from("/tmp/record.wav");
        let config =
            generate_recording_config(Some("Built-in Microphone"), &output_file, 48000, 1, None)
                .unwrap();

        // Convert to YAML string for verification
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Check that the YAML contains the device name
        assert!(yaml.contains("device: Built-in Microphone"));
    }

    #[test]
    fn test_recording_config_matches_working_example() {
        let output_file = PathBuf::from("record.wav");
        let config = generate_recording_config(
            None, // default input device
            &output_file,
            48000,
            1,
            None,
        )
        .unwrap();

        // Convert to YAML string for verification
        let yaml = serde_yaml::to_string(&config).unwrap();

        // The generated YAML should match the structure of the working example
        // Check for all required sections in the correct order
        assert!(yaml.contains("devices:"));
        assert!(yaml.contains("samplerate: 48000"));
        assert!(yaml.contains("chunksize: 1024"));
        assert!(yaml.contains("silence_threshold: -60"));
        assert!(yaml.contains("silence_timeout: 3.0"));
        assert!(yaml.contains("capture:"));
        assert!(yaml.contains("type: CoreAudio"));
        assert!(yaml.contains("channels: 1"));
        assert!(yaml.contains("playback:"));
        assert!(yaml.contains("type: File"));
        assert!(yaml.contains("channels: 1"));
        assert!(yaml.contains("format: FLOAT32LE"));
        assert!(yaml.contains("wav_header: true"));
        assert!(yaml.contains("filename: record.wav"));
    }
}
