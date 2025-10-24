use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents information about an audio device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_input: bool,
    pub is_default: bool,
    pub supported_configs: Vec<AudioConfig>,
    pub default_config: Option<AudioConfig>,
}

/// Represents audio configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: Option<u32>,
    pub sample_format: String, // "f32", "i16", "u16"
}

/// State for storing the currently selected audio configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioState {
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub input_config: Option<AudioConfig>,
    pub output_config: Option<AudioConfig>,
}

pub type SharedAudioState = Arc<Mutex<AudioState>>;

/// Helper function to convert cpal sample format to string
fn format_to_string(format: cpal::SampleFormat) -> String {
    match format {
        cpal::SampleFormat::F32 => "f32".to_string(),
        cpal::SampleFormat::I16 => "i16".to_string(),
        cpal::SampleFormat::U16 => "u16".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Get information about all available audio devices
pub fn get_audio_devices() -> Result<HashMap<String, Vec<AudioDevice>>, String> {
    println!("[AUDIO DEBUG] Enumerating audio devices...");
    let host = cpal::default_host();
    let mut devices_map = HashMap::new();

    // Get input devices
    let mut input_devices = Vec::new();
    match host.input_devices() {
        Ok(devices) => {
            let default_input = host.default_input_device();
            let default_input_name = default_input.as_ref().and_then(|d| d.name().ok());
            if let Some(ref name) = default_input_name {
                println!("[AUDIO DEBUG] Default input device: {}", name);
            }

            for device in devices {
                if let Ok(name) = device.name() {
                    let is_default = default_input_name.as_ref() == Some(&name);

                    // Get supported configurations
                    let mut supported_configs = Vec::new();
                    if let Ok(configs) = device.supported_input_configs() {
                        for config in configs {
                            let config_range = config;
                            // Add min and max sample rate configs
                            for sample_rate in [
                                config_range.min_sample_rate().0,
                                config_range.max_sample_rate().0,
                            ] {
                                // Only include valid channel configurations (1 or 2 for input devices)
                                let max_channels = config_range.channels();
                                let channel_configs: Vec<u16> = if max_channels == 1 {
                                    vec![1]
                                } else if max_channels >= 2 {
                                    vec![1, 2] // Most inputs are mono or stereo
                                } else {
                                    vec![max_channels] // Fallback to device max
                                };

                                for &channels in &channel_configs {
                                    supported_configs.push(AudioConfig {
                                        sample_rate,
                                        channels,
                                        buffer_size: None,
                                        sample_format: format_to_string(config.sample_format()),
                                    });
                                }
                            }
                        }
                    }

                    // Get default configuration if available
                    let default_config =
                        device
                            .default_input_config()
                            .ok()
                            .map(|config| AudioConfig {
                                sample_rate: config.sample_rate().0,
                                channels: config.channels(),
                                buffer_size: None,
                                sample_format: format_to_string(config.sample_format()),
                            });

                    // Report what we detected
                    let channel_info = if let Some(ref cfg) = default_config {
                        format!("{} ch", cfg.channels)
                    } else {
                        "unknown".to_string()
                    };

                    input_devices.push(AudioDevice {
                        name: name.clone(),
                        is_input: true,
                        is_default,
                        supported_configs,
                        default_config,
                    });
                    println!(
                        "[AUDIO DEBUG] Found input device: {} (default: {}, channels: {})",
                        name, is_default, channel_info
                    );
                }
            }
            println!(
                "[AUDIO DEBUG] Total input devices found: {}",
                input_devices.len()
            );
        }
        Err(e) => {
            eprintln!("[AUDIO ERROR] Failed to enumerate input devices: {}", e);
            // Continue with empty input devices list rather than failing completely
        }
    }

    // Get output devices
    let mut output_devices = Vec::new();
    match host.output_devices() {
        Ok(devices) => {
            let default_output = host.default_output_device();
            let default_output_name = default_output.as_ref().and_then(|d| d.name().ok());
            if let Some(ref name) = default_output_name {
                println!("[AUDIO DEBUG] Default output device: {}", name);
            }

            for device in devices {
                if let Ok(name) = device.name() {
                    let is_default = default_output_name.as_ref() == Some(&name);

                    // Get supported configurations
                    let mut supported_configs = Vec::new();
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            let config_range = config;
                            // Add common sample rates
                            for sample_rate in [
                                44100,
                                48000,
                                88200,
                                96000,
                                176400,
                                192000,
                                config_range.min_sample_rate().0,
                                config_range.max_sample_rate().0,
                            ] {
                                if sample_rate < config_range.min_sample_rate().0
                                    || sample_rate > config_range.max_sample_rate().0
                                {
                                    continue;
                                }
                                // Common channel configurations
                                for &channels in &[1, 2, config_range.channels()] {
                                    if channels > config_range.channels() {
                                        continue;
                                    }

                                    // Avoid duplicates
                                    let config = AudioConfig {
                                        sample_rate,
                                        channels,
                                        buffer_size: None,
                                        sample_format: format_to_string(config.sample_format()),
                                    };

                                    if !supported_configs.iter().any(|c: &AudioConfig| {
                                        c.sample_rate == config.sample_rate
                                            && c.channels == config.channels
                                            && c.sample_format == config.sample_format
                                    }) {
                                        supported_configs.push(config);
                                    }
                                }
                            }
                        }
                    }

                    // Get default configuration if available
                    let default_config =
                        device
                            .default_output_config()
                            .ok()
                            .map(|config| AudioConfig {
                                sample_rate: config.sample_rate().0,
                                channels: config.channels(),
                                buffer_size: None,
                                sample_format: format_to_string(config.sample_format()),
                            });

                    // Report what we detected - don't make assumptions
                    let channel_info = if let Some(ref cfg) = default_config {
                        format!("{} ch", cfg.channels)
                    } else {
                        "unknown".to_string()
                    };

                    output_devices.push(AudioDevice {
                        name: name.clone(),
                        is_input: false,
                        is_default,
                        supported_configs,
                        default_config,
                    });
                    println!(
                        "[AUDIO DEBUG] Found output device: {} (default: {}, channels: {})",
                        name, is_default, channel_info
                    );
                }
            }
            println!(
                "[AUDIO DEBUG] Total output devices found: {}",
                output_devices.len()
            );
        }
        Err(e) => {
            eprintln!("[AUDIO ERROR] Failed to enumerate output devices: {}", e);
            // Continue with empty output devices list rather than failing completely
        }
    }

    devices_map.insert("input".to_string(), input_devices);
    devices_map.insert("output".to_string(), output_devices);

    // Check if no devices were found at all
    if devices_map.get("input").is_none_or(|v| v.is_empty())
        && devices_map.get("output").is_none_or(|v| v.is_empty())
    {
        eprintln!("[AUDIO WARNING] No audio devices found on the system");
    }

    Ok(devices_map)
}

/// Set the configuration for an audio device
pub fn set_audio_device(
    device_name: String,
    is_input: bool,
    config: AudioConfig,
    audio_state: &SharedAudioState,
) -> Result<String, String> {
    println!(
        "[AUDIO DEBUG] Setting {} device '{}' with config: sample_rate={}, channels={}, format={}",
        if is_input { "input" } else { "output" },
        device_name,
        config.sample_rate,
        config.channels,
        config.sample_format
    );

    let host = cpal::default_host();

    // Find the device by name
    let device = if is_input {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| d.name().ok() == Some(device_name.clone()))
    } else {
        host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .find(|d| d.name().ok() == Some(device_name.clone()))
    };

    let device = device.ok_or_else(|| format!("Device '{}' not found", device_name))?;

    // Validate the configuration against device capabilities
    let config_valid = if is_input {
        match device.supported_input_configs() {
            Ok(configs) => {
                let mut valid = false;
                for supported_config in configs {
                    let sample_rate = cpal::SampleRate(config.sample_rate);
                    if supported_config.min_sample_rate() <= sample_rate
                        && supported_config.max_sample_rate() >= sample_rate
                        && supported_config.channels() >= config.channels
                        && format_to_string(supported_config.sample_format())
                            == config.sample_format
                    {
                        valid = true;
                        break;
                    }
                }
                valid
            }
            Err(e) => {
                return Err(format!("Failed to get input configs: {}", e));
            }
        }
    } else {
        match device.supported_output_configs() {
            Ok(configs) => {
                let mut valid = false;
                for supported_config in configs {
                    let sample_rate = cpal::SampleRate(config.sample_rate);
                    if supported_config.min_sample_rate() <= sample_rate
                        && supported_config.max_sample_rate() >= sample_rate
                        && supported_config.channels() >= config.channels
                        && format_to_string(supported_config.sample_format())
                            == config.sample_format
                    {
                        valid = true;
                        break;
                    }
                }
                valid
            }
            Err(e) => {
                return Err(format!("Failed to get output configs: {}", e));
            }
        }
    };

    if !config_valid {
        eprintln!(
            "[AUDIO ERROR] Invalid configuration for device '{}': sample_rate={}, channels={}, format={}",
            device_name, config.sample_rate, config.channels, config.sample_format
        );
        return Err(format!(
            "Configuration not supported by device '{}': sample_rate={}, channels={}, format={}",
            device_name, config.sample_rate, config.channels, config.sample_format
        ));
    }

    // Store the configuration in the application state
    let mut state = audio_state
        .lock()
        .map_err(|e| format!("Failed to lock audio state: {}", e))?;

    if is_input {
        state.selected_input_device = Some(device_name.clone());
        state.input_config = Some(config.clone());
    } else {
        state.selected_output_device = Some(device_name.clone());
        state.output_config = Some(config.clone());
    }

    let success_msg = format!(
        "Successfully configured {} device '{}' with sample_rate={}, channels={}, format={}",
        if is_input { "input" } else { "output" },
        device_name,
        config.sample_rate,
        config.channels,
        config.sample_format
    );
    println!("[AUDIO DEBUG] {}", success_msg);
    Ok(success_msg)
}

/// Get the current audio configuration
pub fn get_audio_config(
    audio_state: &SharedAudioState,
) -> Result<AudioState, String> {
    println!("[AUDIO DEBUG] Getting current audio configuration");
    let state = audio_state.lock().map_err(|e| {
        eprintln!("[AUDIO ERROR] Failed to lock audio state: {}", e);
        format!("Failed to lock audio state: {}", e)
    })?;

    if let Some(ref device) = state.selected_input_device {
        println!("[AUDIO DEBUG] Current input device: {}", device);
    }
    if let Some(ref device) = state.selected_output_device {
        println!("[AUDIO DEBUG] Current output device: {}", device);
    }

    Ok(state.clone())
}

/// Get detailed properties of a specific audio device
pub fn get_device_properties(
    device_name: String,
    is_input: bool,
) -> Result<serde_json::Value, String> {
    println!(
        "[AUDIO DEBUG] Getting properties for {} device: {}",
        if is_input { "input" } else { "output" },
        device_name
    );

    let host = cpal::default_host();

    // Find the device by name
    let device = if is_input {
        host.input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
            .find(|d| d.name().ok() == Some(device_name.clone()))
    } else {
        host.output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
            .find(|d| d.name().ok() == Some(device_name.clone()))
    };

    let device = device.ok_or_else(|| format!("Device '{}' not found", device_name))?;

    // Get all supported configurations
    let mut properties = serde_json::json!({
        "name": device_name,
        "type": if is_input { "input" } else { "output" },
    });

    let mut config_ranges = Vec::new();
    if is_input {
        if let Ok(configs) = device.supported_input_configs() {
            for config in configs {
                config_ranges.push(serde_json::json!({
                    "min_sample_rate": config.min_sample_rate().0,
                    "max_sample_rate": config.max_sample_rate().0,
                    "channels": config.channels(),
                    "sample_format": format_to_string(config.sample_format()),
                    "buffer_size_range": match config.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } => {
                            serde_json::json!({ "min": min, "max": max })
                        },
                        cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                    },
                }));
            }
        }
    } else if let Ok(configs) = device.supported_output_configs() {
        for config in configs {
            config_ranges.push(serde_json::json!({
                "min_sample_rate": config.min_sample_rate().0,
                "max_sample_rate": config.max_sample_rate().0,
                "channels": config.channels(),
                "sample_format": format_to_string(config.sample_format()),
                "buffer_size_range": match config.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => {
                        serde_json::json!({ "min": min, "max": max })
                    },
                    cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                },
            }));
        }
    }
    properties["supported_config_ranges"] = serde_json::json!(config_ranges);

    // Get default configuration
    if is_input {
        if let Ok(default_config) = device.default_input_config() {
            properties["default_config"] = serde_json::json!({
                "sample_rate": default_config.sample_rate().0,
                "channels": default_config.channels(),
                "sample_format": format_to_string(default_config.sample_format()),
                "buffer_size": match default_config.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => {
                        serde_json::json!({ "min": min, "max": max })
                    },
                    cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
                },
            });
        }
    } else if let Ok(default_config) = device.default_output_config() {
        properties["default_config"] = serde_json::json!({
            "sample_rate": default_config.sample_rate().0,
            "channels": default_config.channels(),
            "sample_format": format_to_string(default_config.sample_format()),
            "buffer_size": match default_config.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => {
                    serde_json::json!({ "min": min, "max": max })
                },
                cpal::SupportedBufferSize::Unknown => serde_json::json!("unknown"),
            },
        });
    }

    Ok(properties)
}
