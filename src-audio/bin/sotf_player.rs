use autoeq_iir::{Biquad, BiquadFilterType};
use clap::{Parser, Subcommand};
use sotf_audio::LoudnessCompensation;
use sotf_audio::{AudioStreamingManager, PluginConfig, StreamingState};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

fn parse_loudness_compensation(vals: &Vec<f64>) -> Result<Option<LoudnessCompensation>, String> {
    let (ref_level, low, high) = match vals.as_slice() {
        [r, l] => (*r, *l, *l),
        [r, l, h] => (*r, *l, *h),
        _ => return Err("Expected 2 or 3 values: REF,LOW[,HIGH]".to_string()),
    };
    LoudnessCompensation::new(ref_level, low, high)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// Create upmixer PluginConfig from parameters
fn create_upmixer_plugin_config(
    fft_size: usize,
    gain_front_direct: f32,
    gain_front_ambient: f32,
    gain_rear_ambient: f32,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Validate FFT size
    if !fft_size.is_power_of_two() {
        return Err(format!(
            "Upmixer FFT size must be power of 2, got {}",
            fft_size
        ));
    }

    let parameters = json!({
        "fft_size": fft_size,
        "gain_front_direct": gain_front_direct,
        "gain_front_ambient": gain_front_ambient,
        "gain_rear_ambient": gain_rear_ambient,
    });

    Ok(PluginConfig {
        plugin_type: "upmixer".to_string(),
        parameters,
    })
}

/// Convert loudness compensation to PluginConfig
fn create_loudness_compensation_plugin_config(
    lc: &LoudnessCompensation,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Map from LoudnessCompensation fields to plugin parameters
    // reference_level and attenuate_mid are not used by the plugin
    // The plugin uses fixed frequencies (100Hz low, 10kHz high)
    let parameters = json!({
        "low_freq": 100.0,  // Fixed low-shelf frequency
        "low_gain": lc.low_boost,
        "high_freq": 10000.0,  // Fixed high-shelf frequency
        "high_gain": lc.high_boost,
    });

    Ok(PluginConfig {
        plugin_type: "loudness_compensation".to_string(),
        parameters,
    })
}

/// Convert Biquad filters to PluginConfig for EQ plugin
fn create_eq_plugin_config(filters: &[Biquad]) -> Result<PluginConfig, String> {
    use serde_json::json;

    // Convert Biquad to BiquadFilterConfig format
    let filter_configs: Result<Vec<_>, String> = filters
        .iter()
        .map(|f| {
            // Use long_name() from BiquadFilterType
            let filter_type = match f.filter_type {
                BiquadFilterType::HighpassVariableQ => "highpass".to_string(),
                _ => f.filter_type.long_name().to_lowercase(),
            };

            Ok(json!({
                "filter_type": filter_type,
                "freq": f.freq,
                "q": f.q,
                "db_gain": f.db_gain,
            }))
        })
        .collect();

    let filter_configs = filter_configs?;

    let parameters = json!({
        "filters": filter_configs,
    });

    Ok(PluginConfig {
        plugin_type: "eq".to_string(),
        parameters,
    })
}

#[derive(Parser)]
#[command(name = "sotf_player")]
#[command(about = "Audio player with EQ, upmixing, and LUFS monitoring", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available audio devices
    Devices,

    /// Analyze an audio file and print ReplayGain data (gain and peak)
    #[command(name = "replay-gain")]
    ReplayGain {
        /// Path to audio file (supports WAV, FLAC, MP3, AAC/M4A, Vorbis/OGG, AIFF)
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Play an audio file using the streaming decoder (supports seeking and LUFS)
    Play {
        /// Path to audio file (supports WAV, FLAC, MP3, AAC/M4A, Vorbis/OGG, AIFF)
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// EQ filters: "freq:q:gain" (Peak) or "type:freq:q:gain"
        ///
        /// Filter types: PK/PEAK, LS/LOWSHELF, HS/HIGHSHELF, LP/LOWPASS, HP/HIGHPASS, NO/NOTCH, BP/BANDPASS
        ///
        /// Examples: "1000:1.5:3.0" (Peak +3dB), "LS:100:0.7:-2.0" (Lowshelf -2dB), "HP:80:0.707:0"
        #[arg(short, long = "filter", value_name = "FILTER")]
        filters: Vec<String>,

        /// Hardware output channel mapping: "input_channels->output_channels"
        ///
        /// Maps input channels to specific hardware output channels. Use "_" for gaps.
        ///
        /// Examples:
        ///   "1,2->9,10"                 - Route stereo to hardware channels 9,10
        ///   "1,2,3,4,5->1,2,3,_,5,6"    - Route 5ch with gap (skip channel 4 position)
        ///   "1,2,3,4,5,6->13,14,15,16,17,18"  - Route 5.1 to channels 13-18
        #[arg(long = "hwaudio-play")]
        hwaudio_play: Option<String>,

        /// Duration to play in seconds (0 = play until stopped)
        #[arg(short = 't', long, default_value = "0")]
        duration: u64,

        /// Start playback at specific time (seconds)
        #[arg(short = 's', long, default_value = "0")]
        start_time: f64,

        /// Buffer size in chunks (32=low latency, 128=balanced, 1024=high reliability)
        #[arg(long = "buffer-chunks", default_value = "32")]
        _buffer_chunks: usize,

        /// Enable real-time LUFS monitoring (prints momentary/short-term loudness)
        #[arg(long = "lufs", alias = "monitor-lufs", default_value_t = false)]
        lufs: bool,

        /// Loudness compensation: 2 or 3 floats: REF LOW [HIGH] (dB; REF -100..20, boosts 0..20)
        #[arg(long = "loudness-compensation", value_name = "REF,LOW[,HIGH]", value_parser = clap::value_parser!(f64), value_delimiter = ',')]
        loudness_compensation: Option<Vec<f64>>,

        /// Enable stereo-to-5.0 upmixer (converts 2ch to 5ch surround)
        #[arg(long = "upmixer", default_value_t = false)]
        upmixer: bool,

        /// Upmixer FFT size (must be power of 2: 1024, 2048, 4096)
        #[arg(long = "upmixer-fft-size", default_value = "2048")]
        upmixer_fft_size: usize,

        /// Upmixer front direct gain (0.0-2.0)
        #[arg(long = "upmixer-gain-front-direct", default_value = "1.0")]
        upmixer_gain_front_direct: f32,

        /// Upmixer front ambient gain (0.0-2.0)
        #[arg(long = "upmixer-gain-front-ambient", default_value = "0.5")]
        upmixer_gain_front_ambient: f32,

        /// Upmixer rear ambient gain (0.0-2.0)
        #[arg(long = "upmixer-gain-rear-ambient", default_value = "1.0")]
        upmixer_gain_rear_ambient: f32,
    },

    /// Get current playback status
    Status,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Devices => {
            if let Err(e) = list_devices() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::ReplayGain { file } => match sotf_audio::replaygain::analyze_file(&file) {
            Ok(info) => {
                println!("ReplayGain analysis:");
                println!("  File: {:?}", file);
                println!("  Gain: {:+.2} dB", info.gain);
                println!("  Peak: {:.6}", info.peak);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::Play {
            file,
            device,
            filters,
            hwaudio_play,
            duration,
            start_time,
            _buffer_chunks,
            lufs,
            loudness_compensation,
            upmixer,
            upmixer_fft_size,
            upmixer_gain_front_direct,
            upmixer_gain_front_ambient,
            upmixer_gain_rear_ambient,
        } => {
            // Parse filters
            let filter_params = match parse_filters(&filters) {
                Ok(params) => params,
                Err(e) => {
                    eprintln!("Error parsing filters: {}", e);
                    std::process::exit(1);
                }
            };

            // Parse loudness compensation
            let loudness: Option<LoudnessCompensation> = match loudness_compensation {
                Some(ref vals) => parse_loudness_compensation(vals).unwrap_or_else(|e| {
                    eprintln!("Error in --loudness-compensation: {}", e);
                    std::process::exit(1);
                }),
                None => None,
            };

            if let Err(e) = play_stream(
                file,
                device,
                filter_params,
                duration,
                start_time,
                hwaudio_play,
                lufs,
                loudness,
                upmixer,
                upmixer_fft_size,
                upmixer_gain_front_direct,
                upmixer_gain_front_ambient,
                upmixer_gain_rear_ambient,
            ) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Status => {
            println!("Status command not yet implemented (requires running manager instance)");
        }
    }
}

fn list_devices() -> Result<(), String> {
    println!("Enumerating audio devices...\n");

    let devices = sotf_audio::devices::get_audio_devices()
        .map_err(|e| format!("Failed to get devices: {}", e))?;

    // Print input devices
    if let Some(input_devices) = devices.get("input") {
        println!("Input Devices:");
        println!("{}", "=".repeat(60));
        for (idx, device) in input_devices.iter().enumerate() {
            println!("  [{}] {}", idx + 1, device.name);
            if device.is_default {
                println!("      (Default)");
            }
            if let Some(config) = &device.default_config {
                println!(
                    "      {}Hz, {} ch, {}",
                    config.sample_rate, config.channels, config.sample_format
                );
            }
        }
        println!();
    }

    // Print output devices
    if let Some(output_devices) = devices.get("output") {
        println!("Output Devices:");
        println!("{}", "=".repeat(60));
        for (idx, device) in output_devices.iter().enumerate() {
            println!("  [{}] {}", idx + 1, device.name);
            if device.is_default {
                println!("      (Default)");
            }
            if let Some(config) = &device.default_config {
                println!(
                    "      {}Hz, {} ch, {}",
                    config.sample_rate, config.channels, config.sample_format
                );
            }
        }
        println!();
    }

    Ok(())
}

fn parse_filter_type(type_str: &str) -> Result<BiquadFilterType, String> {
    match type_str.to_uppercase().as_str() {
        "PK" | "PEAK" => Ok(BiquadFilterType::Peak),
        "LS" | "LOWSHELF" => Ok(BiquadFilterType::Lowshelf),
        "HS" | "HIGHSHELF" => Ok(BiquadFilterType::Highshelf),
        "LP" | "LOWPASS" => Ok(BiquadFilterType::Lowpass),
        "HP" | "HIGHPASS" => Ok(BiquadFilterType::Highpass),
        "NO" | "NOTCH" => Ok(BiquadFilterType::Notch),
        "BP" | "BANDPASS" => Ok(BiquadFilterType::Bandpass),
        _ => Err(format!(
            "Unknown filter type '{}'. Valid types: PK/PEAK, LS/LOWSHELF, HS/HIGHSHELF, LP/LOWPASS, HP/HIGHPASS, NO/NOTCH, BP/BANDPASS",
            type_str
        )),
    }
}

fn parse_filters(filter_strings: &[String]) -> Result<Vec<Biquad>, String> {
    filter_strings
        .iter()
        .map(|filter_str| {
            let parts: Vec<&str> = filter_str.split(':').collect();

            // Support both formats:
            // - 3 parts: freq:q:gain (defaults to Peak)
            // - 4 parts: type:freq:q:gain
            let (filter_type, frequency, q, gain) = match parts.len() {
                3 => {
                    // Format: freq:q:gain (default to Peak)
                    let frequency = parts[0]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[0]))?;
                    let q = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[1]))?;
                    let gain = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[2]))?;
                    (BiquadFilterType::Peak, frequency, q, gain)
                }
                4 => {
                    // Format: type:freq:q:gain
                    let filter_type = parse_filter_type(parts[0])?;
                    let frequency = parts[1]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid frequency: {}", parts[1]))?;
                    let q = parts[2]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid Q: {}", parts[2]))?;
                    let gain = parts[3]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid gain: {}", parts[3]))?;
                    (filter_type, frequency, q, gain)
                }
                _ => {
                    return Err(format!(
                        "Invalid filter format '{}'. Expected 'freq:q:gain' or 'type:freq:q:gain'",
                        filter_str
                    ));
                }
            };

            // Validate ranges
            if !(20.0..=20000.0).contains(&frequency) {
                return Err(format!(
                    "Frequency must be between 20 and 20000 Hz, got {}",
                    frequency
                ));
            }
            if q <= 0.0 || q > 100.0 {
                return Err(format!("Q must be between 0 and 100, got {}", q));
            }
            if gain.abs() > 30.0 {
                return Err(format!("Gain must be between -30 and +30 dB, got {}", gain));
            }

            // Use placeholder sample rate - will be updated by EqPlugin::initialize()
            Ok(Biquad::new(filter_type, frequency, 48000.0, q, gain))
        })
        .collect()
}

/// Parse channel mapping specification and create matrix plugin config
///
/// Format: "in1,in2,...->out1,out2,..." where channels are 1-indexed
/// Use "_" in output to skip a channel position
///
/// Examples:
///   "1,2->9,10"                 - Route stereo to HW channels 9,10
///   "1,2,3,4,5->1,2,3,_,5,6"    - Route 5ch with gap (skip position 4)
///   "1,2,3,4,5,6->13,14,15,16,17,18"  - Route 5.1 to channels 13-18
///
/// Returns: (input_channels, output_channels, matrix, max_hw_channel)
fn parse_channel_mapping(mapping_str: &str) -> Result<(usize, usize, Vec<f32>, usize), String> {
    let parts: Vec<&str> = mapping_str.split("->").collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid mapping format '{}'. Expected 'in1,in2,...->out1,out2,...'",
            mapping_str
        ));
    }

    // Parse input channels (1-indexed)
    let input_channels: Result<Vec<usize>, _> = parts[0]
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| format!("Invalid input channel: '{}'", s))
        })
        .collect();
    let input_channels = input_channels?;

    if input_channels.is_empty() {
        return Err("No input channels specified".to_string());
    }

    // Parse output channel mapping (1-indexed, with "_" for gaps)
    let output_spec: Vec<&str> = parts[1].split(',').map(|s| s.trim()).collect();
    if output_spec.is_empty() {
        return Err("No output channels specified".to_string());
    }

    // Build mapping: input_ch_idx -> output_hw_ch (0-indexed internally, but 1-indexed in spec)
    let mut channel_map: Vec<Option<usize>> = Vec::new();
    let mut max_hw_channel = 0;

    for spec in output_spec.iter() {
        if *spec == "_" {
            channel_map.push(None); // Gap/skip
        } else {
            let hw_ch = spec
                .parse::<usize>()
                .map_err(|_| format!("Invalid output channel: '{}'", spec))?;
            if hw_ch == 0 {
                return Err("Channel indices must be >= 1 (1-indexed)".to_string());
            }
            channel_map.push(Some(hw_ch - 1)); // Convert to 0-indexed
            max_hw_channel = max_hw_channel.max(hw_ch);
        }
    }

    // Check that we have enough output specs for input channels
    let non_gap_outputs: Vec<_> = channel_map.iter().filter_map(|&x| x).collect();
    if non_gap_outputs.len() != input_channels.len() {
        return Err(format!(
            "Mismatch: {} input channels but {} non-gap output positions",
            input_channels.len(),
            non_gap_outputs.len()
        ));
    }

    // Build matrix: input_channels x max_hw_channel
    // Matrix is row-major: matrix[out_ch * input_channels + in_ch]
    let input_count = input_channels.len();
    let output_count = max_hw_channel;
    let mut matrix = vec![0.0f32; output_count * input_count];

    // Map each input channel to its output position
    let mut input_idx = 0;
    for &output_hw_ch_opt in channel_map.iter() {
        if let Some(output_hw_ch) = output_hw_ch_opt {
            // Set gain from input_idx to output_hw_ch
            matrix[output_hw_ch * input_count + input_idx] = 1.0;
            input_idx += 1;
        }
    }

    Ok((input_count, output_count, matrix, max_hw_channel))
}

/// Create matrix plugin config from parsed mapping
fn create_matrix_plugin_config(
    input_channels: usize,
    output_channels: usize,
    matrix: Vec<f32>,
) -> Result<PluginConfig, String> {
    use serde_json::json;

    let parameters = json!({
        "input_channels": input_channels,
        "output_channels": output_channels,
        "matrix": matrix,
    });

    Ok(PluginConfig {
        plugin_type: "matrix".to_string(),
        parameters,
    })
}

#[allow(clippy::too_many_arguments)]
fn play_stream(
    file: PathBuf,
    device: Option<String>,
    filters: Vec<Biquad>,
    duration: u64,
    start_time: f64,
    hwaudio_play: Option<String>,
    lufs: bool,
    loudness: Option<LoudnessCompensation>,
    upmixer: bool,
    upmixer_fft_size: usize,
    upmixer_gain_front_direct: f32,
    upmixer_gain_front_ambient: f32,
    upmixer_gain_rear_ambient: f32,
) -> Result<(), String> {
    println!("Starting streaming playback...");
    println!("  File: {:?}", file);
    println!("  Device: {:?}", device.as_deref().unwrap_or("default"));
    if start_time > 0.0 {
        println!("  Start time: {:.2}s", start_time);
    }
    println!("  Filters: {}", filters.len());

    if !filters.is_empty() {
        println!("\nEQ Filters:");
        for (idx, filter) in filters.iter().enumerate() {
            println!(
                "  [{}] {} Hz, Q={:.2}, Gain={:.1} dB",
                idx + 1,
                filter.freq,
                filter.q,
                filter.db_gain
            );
        }
    }
    println!();

    // Create streaming manager with signal watching enabled (manager handles Ctrl+C)
    let mut streaming_manager = AudioStreamingManager::with_signal_watching(true);

    // Load the audio file
    let audio_info = streaming_manager
        .load_file(&file)
        .map_err(|e| format!("Failed to load audio file: {}", e))?;

    println!("Loaded audio file:");
    println!("  Format: {}", audio_info.format);
    println!("  Sample rate: {}Hz", audio_info.spec.sample_rate);
    println!("  Channels: {}", audio_info.spec.channels);
    println!("  Bits per sample: {}", audio_info.spec.bits_per_sample);
    if let Some(duration_secs) = audio_info.duration_seconds {
        println!("  Duration: {:.2}s", duration_secs);
    }
    println!();

    // Build plugin chain
    let mut plugins = Vec::new();

    // Upmixer (if enabled)
    let output_channels = if upmixer {
        // Check that input is stereo
        if audio_info.spec.channels != 2 {
            return Err(format!(
                "Upmixer requires stereo input, got {} channels",
                audio_info.spec.channels
            ));
        }

        println!("Enabling stereo-to-5.0 upmixer plugin:");
        println!("  FFT size: {}", upmixer_fft_size);
        println!("  Front direct gain: {:.2}", upmixer_gain_front_direct);
        println!("  Front ambient gain: {:.2}", upmixer_gain_front_ambient);
        println!("  Rear ambient gain: {:.2}", upmixer_gain_rear_ambient);
        println!("  Output: 5.0 surround (FL, FR, C, RL, RR)\n");

        let upmixer_plugin = create_upmixer_plugin_config(
            upmixer_fft_size,
            upmixer_gain_front_direct,
            upmixer_gain_front_ambient,
            upmixer_gain_rear_ambient,
        )?;
        plugins.push(upmixer_plugin);
        eprintln!("Added upmixer plugin: 2ch -> 5ch");
        5 // Upmixer outputs 5.0 surround
    } else {
        audio_info.spec.channels as usize
    };

    // Loudness compensation (before channel mapping)
    if let Some(ref lc) = loudness {
        let lc_plugin = create_loudness_compensation_plugin_config(lc)?;
        plugins.push(lc_plugin);
        eprintln!("Added loudness compensation plugin");
    }

    // EQ filters (assuming it is room eq)
    if !filters.is_empty() {
        let eq_plugin = create_eq_plugin_config(&filters)?;
        plugins.push(eq_plugin);
        eprintln!("Added EQ plugin with {} filters", filters.len());
    }

    // 4. Channel mapping to hardware (last plugin before output)
    let output_channels = if let Some(ref mapping_str) = hwaudio_play {
        let (map_input_ch, map_output_ch, matrix, max_hw_ch) = parse_channel_mapping(mapping_str)?;

        // Verify that mapping input matches current output channels
        if map_input_ch != output_channels {
            return Err(format!(
                "Channel mapping input mismatch: mapping expects {} channels but plugin chain outputs {}",
                map_input_ch, output_channels
            ));
        }

        println!("\nChannel mapping enabled:");
        println!("  Mapping: {}", mapping_str);
        println!("  Input channels: {}", map_input_ch);
        println!(
            "  Output hardware channels: {} (max channel {})",
            map_output_ch, max_hw_ch
        );

        let matrix_plugin = create_matrix_plugin_config(map_input_ch, map_output_ch, matrix)?;
        plugins.push(matrix_plugin);
        eprintln!(
            "Added matrix plugin: {}ch -> {}ch",
            map_input_ch, map_output_ch
        );

        max_hw_ch // Hardware will need this many channels
    } else {
        output_channels // No mapping, use current channel count
    };

    // Start playback (signal handling is done by the manager)
    streaming_manager
        .start_playback(device, plugins, output_channels)
        .map_err(|e| format!("Failed to start streaming playback: {}", e))?;

    // Enable loudness monitoring if requested (must be after playback starts)
    if lufs {
        streaming_manager
            .enable_loudness_monitoring()
            .map_err(|e| format!("Failed to enable loudness monitoring: {}", e))?;
        println!("Real-time LUFS monitoring enabled");
    }

    // Seek to start time if specified
    if start_time > 0.0 {
        println!("Seeking to {:.2}s...", start_time);
        streaming_manager
            .seek(start_time)
            .map_err(|e| format!("Failed to seek: {}", e))?;
    }

    println!("Streaming playback started successfully!");
    println!("Press Ctrl+C to stop\n");

    // Monitor playback
    let start_time_instant = std::time::Instant::now();
    let mut last_state = StreamingState::Idle;
    let mut last_shortterm: Option<f64> = None;

    loop {
        // Check for events (this updates internal state based on engine state)
        streaming_manager.try_recv_event();

        let current_state = streaming_manager.get_state();

        // Print state changes
        if current_state != last_state {
            match current_state {
                StreamingState::Loading => println!("State: Loading..."),
                StreamingState::Ready => println!("State: Ready"),
                StreamingState::Playing => println!("State: Playing"),
                StreamingState::Paused => println!("State: Paused"),
                StreamingState::Seeking => println!("State: Seeking..."),
                StreamingState::Error => {
                    println!("State: Error!");
                    break;
                }
                StreamingState::Idle => {
                    if last_state == StreamingState::Playing {
                        println!("\nPlayback finished");
                    }
                    break;
                }
            }
            last_state = current_state;
        }

        // Print loudness measurements if monitoring is enabled
        if lufs
            && current_state == StreamingState::Playing
            && let Some(loudness) = streaming_manager.get_loudness()
        {
            let st = loudness.shortterm_lufs;
            let changed = match last_shortterm {
                None => true,
                Some(prev) => (st - prev).abs() >= 0.1,
            };
            if changed {
                let momentary_str = if loudness.momentary_lufs.is_infinite() {
                    "-∞".to_string()
                } else {
                    format!("{:5.1}", loudness.momentary_lufs)
                };
                let shortterm_str = if st.is_infinite() {
                    "-∞".to_string()
                } else {
                    format!("{:5.1}", st)
                };
                // Dynamic ReplayGain relative to -18.0 LUFS reference
                let rg = if st.is_infinite() { 0.0 } else { -18.0 - st };
                print!(
                    "\rLUFS: M={} S={}  RG={:+4.1} dB  Peak={:.3}  ",
                    momentary_str, shortterm_str, rg, loudness.peak
                );
                std::io::Write::flush(&mut std::io::stdout()).ok();
                last_shortterm = Some(st);
            }
        }

        // Check duration
        if duration > 0 && start_time_instant.elapsed().as_secs() >= duration {
            println!("\n\nDuration reached, stopping...");
            break;
        }

        sleep(Duration::from_millis(100));
    }

    // Manager handles its own cleanup via Drop
    // If stopped by signal, threads are already shut down
    // If stopped naturally (end of stream/duration), cleanup happens on drop
    println!("Streaming playback stopped successfully");
    Ok(())
}
