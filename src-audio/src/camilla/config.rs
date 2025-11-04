// ============================================================================
// Config Generation
// ============================================================================

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use serde_yaml::{Mapping, Value};

use super::errors::{CamillaError, CamillaResult};
use super::types::{CamillaDSPConfig, DeviceConfig, CaptureDevice, PlaybackDevice, ChannelsSetting};
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
        channels: Some(ChannelsSetting::Count(channels)),
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
        channels: Some(ChannelsSetting::Count(mixer_out_channels)),
        format: None, // Let CoreAudio use default format
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
        capture: Some(capture),
        playback,
        // With a synchronous resampler, rate adjust is not applicable.
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

    Ok(CamillaDSPConfig {
        devices,
        filters: filters_section,
        mixers: mixers_section,
        pipeline,
    })
}

/// Generate a CamillaDSP config for file playback with EQ filters
pub fn generate_playback_config(
    audio_file: &PathBuf,
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

    // Create capture device (file input)
    // Prefer absolute path if the file exists; otherwise, use the provided path without failing
    let filename_str = if audio_file.exists() {
        audio_file
            .canonicalize()
            .map_err(|e| {
                CamillaError::ConfigGenerationFailed(format!(
                    "Failed to resolve audio file path {:?}: {}",
                    audio_file, e
                ))
            })?
            .to_str()
            .ok_or_else(|| {
                CamillaError::ConfigGenerationFailed("Invalid audio file path encoding".to_string())
            })?
            .to_string()
    } else {
        audio_file.to_string_lossy().to_string()
    };

    let capture = CaptureDevice {
        device_type: "File".to_string(),
        device: None,
        filename: Some(filename_str),
        channels: None, // File input infers channels from file
        format: None,   // File input infers format from file
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
        channels: Some(ChannelsSetting::Count(mixer_out_channels)),
        format: None, // Let CoreAudio use default format
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
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
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

    // Always include pipeline with at least the mixer
    let pipeline = Some(generate_pipeline(
        mixer_out_channels,
        filters,
        needs_resampling,
        loudness.is_some(),
        loudness,
    ));

    Ok(CamillaDSPConfig {
        devices,
        filters: filters_section,
        mixers: mixers_section,
        pipeline,
    })
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

/// Generate a CamillaDSP config for recording
pub fn generate_recording_config(
    output_file: &std::path::Path,
    input_device: Option<&str>,
    sample_rate: u32,
    channels: u16,
    input_map: Option<&[u16]>,
) -> CamillaResult<CamillaDSPConfig> {
    // Create capture device (audio input)
    let (capture_type, device_name) = map_input_device(input_device)?;
    // Prepare input channel_map if provided
    let effective_input_map: Option<Vec<u16>> = if let Some(map) = input_map {
        if map.len() as u16 >= channels {
            // Use the first `channels` entries for input channels
            Some(map[..channels as usize].to_vec())
        } else {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Input channel_map length ({}) must be >= channels ({})",
                map.len(),
                channels
            )));
        }
    } else {
        None
    };

    let capture = CaptureDevice {
        device_type: capture_type.clone(),
        device: device_name,
        filename: None,
        // CamillaDSP v3 CoreAudio expects channels as count (usize), not indices
        // When hardware channel mapping is provided, we capture enough channels to include
        // the highest requested channel index, then use mixer to route correctly
        channels: Some(ChannelsSetting::Count(match &effective_input_map {
            Some(map) if !map.is_empty() && capture_type == "CoreAudio" => {
                // For CoreAudio: Calculate max channel index + 1 to ensure we capture enough channels
                let max_channel = map.iter().max().copied().unwrap_or(0);
                max_channel + 1
            }
            _ => channels,
        })),
        format: None, // Let backend pick a compatible native format
    };

    // Create playback device (file output)
    let playback = PlaybackDevice {
        device_type: "File".to_string(),
        device: None,
        filename: Some(
            output_file
                .to_str()
                .ok_or_else(|| {
                    CamillaError::ConfigGenerationFailed(
                        "Invalid output file path encoding".to_string(),
                    )
                })?
                .to_string(),
        ),
        channels: Some(ChannelsSetting::Count(channels)), // Specify channels for file output
        format: Some("FLOAT32LE".to_string()),
    };

    let devices = DeviceConfig {
        samplerate: sample_rate,
        chunksize: 1024,
        capture: Some(capture),
        playback,
        enable_rate_adjust: Some(true),
        resampler: None,
    };

    // If hardware channel mapping is provided and we're using CoreAudio,
    // we need to add a mixer to route the selected channels to the output
    let (mixers_section, pipeline) = if let Some(ref map) = effective_input_map
        && capture_type == "CoreAudio"
    {
        // Get the actual number of channels we're capturing
        let capture_channels = match devices.capture.as_ref().unwrap().channels {
            Some(ChannelsSetting::Count(n)) => n,
            _ => channels,
        };

        // Build the channel routing map:
        // For each captured input channel (0..capture_channels), specify which output channel it goes to.
        // The hardware channels specified in `map` go to sequential output channels (0..channels).
        // All other input channels are routed to a non-existent output (captured but discarded).
        let mut channel_routing: Vec<u16> = vec![channels; capture_channels as usize]; // Default: route to out-of-bounds (discard)
        for (output_idx, &hardware_channel) in map.iter().enumerate() {
            if (hardware_channel as usize) < channel_routing.len() && (output_idx as u16) < channels
            {
                channel_routing[hardware_channel as usize] = output_idx as u16;
            }
        }

        // Generate mixer that routes hardware channels to output channels
        let mixer = generate_matrix_mixer_yaml(capture_channels, channels, &channel_routing);

        // Generate pipeline with just the mixer
        let mut pipeline_steps: Vec<serde_yaml::Value> = Vec::new();
        let mut mixer_map = serde_yaml::Mapping::new();
        mixer_map.insert(
            serde_yaml::Value::String("type".to_string()),
            serde_yaml::Value::String("Mixer".to_string()),
        );
        mixer_map.insert(
            serde_yaml::Value::String("name".to_string()),
            serde_yaml::Value::String("matrix_mixer".to_string()),
        );
        pipeline_steps.push(serde_yaml::Value::Mapping(mixer_map));

        (Some(mixer), Some(pipeline_steps))
    } else {
        (None, None)
    };

    Ok(CamillaDSPConfig {
        devices,
        filters: None,
        mixers: mixers_section,
        pipeline,
    })
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
pub fn generate_filters_yaml(filters: &[FilterParams], loudness: Option<&LoudnessCompensation>) -> CamillaResult<serde_yaml::Value> {
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

        println!("[CamillaDSP] Added volume attenuation filter: {:.1}dB (compensation: {:.1}/{:.1})",
                 attenuation_db, lc.low_boost, lc.high_boost);
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
                    "        - channel: {}\n          gain: 0\n          inverted: false\n",
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

    println!("[CamillaDSP] Generated config:\n{}", yaml);

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
