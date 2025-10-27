use clap::{Parser, Subcommand};
use sotf_backend::camilla::LoudnessCompensation;
use sotf_backend::{
    AudioManager, AudioStreamingManager, CamillaError, FilterParams, StreamingState, audio,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, sleep};

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

#[cfg(unix)]
fn install_shutdown_handler(running: Arc<AtomicBool>) -> Result<(), String> {
    // Handle SIGINT/SIGTERM via signal-hook-tokio (async-friendly)
    use futures_util::StreamExt;
    let signals = signal_hook_tokio::Signals::new([libc::SIGINT, libc::SIGTERM])
        .map_err(|e| format!("Failed to set signal handler: {}", e))?;
    tokio::spawn(async move {
        let mut signals = signals;
        if signals.next().await.is_some() {
            println!("\n\nReceived termination signal, stopping playback...");
            running.store(false, Ordering::SeqCst);
        }
    });
    Ok(())
}

#[cfg(windows)]
fn install_shutdown_handler(running: Arc<AtomicBool>) -> Result<(), String> {
    // Handle Ctrl+C/Ctrl+Break via ctrlc
    ctrlc::set_handler(move || {
        println!("\n\nReceived Ctrl+C, stopping playback...");
        running.store(false, Ordering::SeqCst);
    })
    .map_err(|e| format!("Failed to set Ctrl+C handler: {}", e))?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "sotf_audio")]
#[command(about = "CamillaDSP audio tool (streaming-only playback)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to CamillaDSP binary (optional, will search PATH)
    #[arg(short, long)]
    binary: Option<PathBuf>,
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
    ///
    /// Notes:
    /// - This subcommand always uses the streaming backend.
    /// - Sample rate, channels, and hwaudio-input flags are ignored.
    Play {
        /// Path to audio file (supports WAV, FLAC, MP3, AAC/M4A, Vorbis/OGG, AIFF)
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// Sample rate in Hz (ignored; always streamed)
        #[arg(short = 'r', long, default_value = "48000")]
        sample_rate: u32,

        /// Number of channels (ignored; always streamed)
        #[arg(short, long, default_value = "2")]
        channels: u16,

        /// EQ filters in format "freq:q:gain" (e.g., "1000:1.5:3.0")
        #[arg(short, long = "filter", value_name = "FREQ:Q:GAIN")]
        filters: Vec<String>,

        /// Hardware input channel map (ignored; always streamed)
        #[arg(long = "hwaudio-input", value_delimiter = ',')]
        hwaudio_input: Option<Vec<u16>>,

        /// Hardware output channel map (comma-separated indices)
        #[arg(long = "hwaudio-output", value_delimiter = ',')]
        hwaudio_output: Option<Vec<u16>>,

        /// Swap left and right channels
        #[arg(long = "swap-channels", default_value_t = false)]
        swap_channels: bool,

        /// Duration to play in seconds (0 = play until stopped)
        #[arg(short = 't', long, default_value = "0")]
        duration: u64,

        /// Start playback at specific time (seconds)
        #[arg(short = 's', long, default_value = "0")]
        start_time: f64,

        /// Buffer size in chunks (32=low latency, 128=balanced, 1024=high reliability)
        #[arg(long = "buffer-chunks", default_value = "128")]
        buffer_chunks: usize,

        /// Enable real-time LUFS monitoring (prints momentary/short-term loudness)
        #[arg(long = "lufs", alias = "monitor-lufs", default_value_t = false)]
        lufs: bool,

        /// Loudness compensation: 2 or 3 floats: REF LOW [HIGH] (dB; REF -100..20, boosts 0..20)
        #[arg(long = "loudness-compensation", value_name = "REF,LOW[,HIGH]", value_parser = clap::value_parser!(f64), value_delimiter = ',')]
        loudness_compensation: Option<Vec<f64>>,
    },

    /// Record audio from input device
    Record {
        /// Output file path
        #[arg(value_name = "FILE")]
        output: PathBuf,

        /// Input device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// Sample rate in Hz
        #[arg(short = 'r', long, default_value = "48000")]
        sample_rate: u32,

        /// Number of channels
        #[arg(short, long, default_value = "2")]
        channels: u16,

        /// Hardware input channel map (comma-separated indices)
        #[arg(long = "hwaudio-input", value_delimiter = ',')]
        hwaudio_input: Option<Vec<u16>>,

        /// Duration to record in seconds
        #[arg(short = 't', long, default_value = "10")]
        duration: u64,
    },

    /// Get current playback status
    Status,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Find CamillaDSP binary
    let binary_path = match cli.binary {
        Some(path) => {
            if !path.exists() {
                eprintln!("Error: Binary not found at {:?}", path);
                std::process::exit(1);
            }
            path
        }
        None => match sotf_backend::camilla::find_camilladsp_binary() {
            Ok(path) => {
                println!("Found CamillaDSP binary at: {:?}", path);
                path
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("\nPlease install CamillaDSP or specify the binary path with --binary");
                std::process::exit(1);
            }
        },
    };

    match cli.command {
        Commands::Devices => {
            if let Err(e) = list_devices().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::ReplayGain { file } => match sotf_backend::replaygain::analyze_file(&file) {
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
            sample_rate: _,
            channels: _,
            filters,
            hwaudio_input: _,
            hwaudio_output,
            swap_channels,
            duration,
            start_time,
            buffer_chunks,
            lufs,
            loudness_compensation,
        } => {
            // Parse filters
            let filter_params = match parse_filters(&filters) {
                Ok(params) => params,
                Err(e) => {
                    eprintln!("Error parsing filters: {}", e);
                    std::process::exit(1);
                }
            };

            let map_mode = if swap_channels {
                sotf_backend::camilla::ChannelMapMode::Swap
            } else {
                sotf_backend::camilla::ChannelMapMode::Normal
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
                binary_path,
                file,
                device,
                filter_params,
                duration,
                start_time,
                map_mode,
                hwaudio_output,
                buffer_chunks,
                lufs,
                loudness,
            )
            .await
            {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Record {
            output,
            device,
            sample_rate,
            channels,
            hwaudio_input,
            duration,
        } => {
            if let Err(e) = record_audio(
                binary_path,
                output,
                device,
                sample_rate,
                channels,
                duration,
                hwaudio_input,
            )
            .await
            {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Status => {
            println!("Status command not yet implemented (requires running manager instance)");
        }
    }
}

async fn list_devices() -> Result<(), String> {
    println!("Enumerating audio devices...\n");

    let devices =
        audio::get_audio_devices().map_err(|e| format!("Failed to get devices: {}", e))?;

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

async fn record_audio(
    binary_path: PathBuf,
    output: PathBuf,
    device: Option<String>,
    sample_rate: u32,
    channels: u16,
    duration: u64,
    hwaudio_input: Option<Vec<u16>>,
) -> Result<(), String> {
    println!("Starting recording...");
    println!("  Output: {:?}", output);
    println!("  Device: {:?}", device.as_deref().unwrap_or("default"));
    println!("  Sample rate: {}Hz", sample_rate);
    println!("  Channels: {}", channels);
    println!("  Duration: {}s", duration);
    println!();

    // Create audio manager
    let manager = AudioManager::new(binary_path);

    // Set up shutdown handler (Ctrl+C / SIGTERM)
    let running = Arc::new(AtomicBool::new(true));
    install_shutdown_handler(running.clone())?;

    // Start recording
    manager
        .start_recording(output.clone(), device, sample_rate, channels, hwaudio_input)
        .await
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    println!("Recording started!");
    println!("Press Ctrl+C to stop early\n");

    // Monitor recording
    let start_time = std::time::Instant::now();

    while running.load(Ordering::SeqCst) {
        let elapsed = start_time.elapsed().as_secs();
        print!("\rRecording: {}s / {}s", elapsed, duration);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        if elapsed >= duration {
            break;
        }

        sleep(Duration::from_secs(1)).await;
    }

    // Stop recording
    println!("\n\nStopping recording...");
    manager
        .stop_recording()
        .await
        .map_err(|e| format!("Failed to stop recording: {}", e))?;

    println!("Recording saved to: {:?}", output);
    Ok(())
}

fn parse_filters(filter_strings: &[String]) -> Result<Vec<FilterParams>, CamillaError> {
    let mut filters = Vec::new();

    for filter_str in filter_strings {
        let parts: Vec<&str> = filter_str.split(':').collect();
        if parts.len() != 3 {
            return Err(CamillaError::InvalidConfiguration(format!(
                "Invalid filter format '{}'. Expected 'freq:q:gain'",
                filter_str
            )));
        }

        let frequency = parts[0].parse::<f64>().map_err(|_| {
            CamillaError::InvalidConfiguration(format!("Invalid frequency: {}", parts[0]))
        })?;

        let q = parts[1]
            .parse::<f64>()
            .map_err(|_| CamillaError::InvalidConfiguration(format!("Invalid Q: {}", parts[1])))?;

        let gain = parts[2].parse::<f64>().map_err(|_| {
            CamillaError::InvalidConfiguration(format!("Invalid gain: {}", parts[2]))
        })?;

        let filter = FilterParams::new(frequency, q, gain);
        filter.validate()?;
        filters.push(filter);
    }

    Ok(filters)
}

#[allow(clippy::too_many_arguments)]
async fn play_stream(
    binary_path: PathBuf,
    file: PathBuf,
    device: Option<String>,
    filters: Vec<FilterParams>,
    duration: u64,
    start_time: f64,
    map_mode: sotf_backend::camilla::ChannelMapMode,
    hwaudio_output: Option<Vec<u16>>,
    buffer_chunks: usize,
    lufs: bool,
    loudness: Option<LoudnessCompensation>,
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
                filter.frequency,
                filter.q,
                filter.gain
            );
        }
    }
    println!();

    // Create streaming manager
    let mut streaming_manager = AudioStreamingManager::new(binary_path);

    // Configure buffer size
    let clamped_chunks = buffer_chunks.clamp(32, 1024);
    streaming_manager.set_buffer_chunks(clamped_chunks);
    println!(
        "  Buffer: {} chunks ({} frames, ~{:.1}ms latency)",
        clamped_chunks,
        clamped_chunks * 1024,
        (clamped_chunks * 1024) as f64 / 48000.0 * 1000.0
    );

    // Set up shutdown handler (Ctrl+C / SIGTERM)
    let running = Arc::new(AtomicBool::new(true));
    install_shutdown_handler(running.clone())?;

    // Load the audio file
    let r_check = running.clone();
    let audio_info = tokio::select! {
        result = streaming_manager.load_file(&file) => {
            result.map_err(|e| format!("Failed to load audio file: {}", e))?
        }
        _ = async {
            while r_check.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100)).await;
            }
        } => {
            println!("Loading cancelled");
            return Ok(());
        }
    };

    println!("Loaded audio file:");
    println!("  Format: {}", audio_info.format);
    println!("  Sample rate: {}Hz", audio_info.spec.sample_rate);
    println!("  Channels: {}", audio_info.spec.channels);
    println!("  Bits per sample: {}", audio_info.spec.bits_per_sample);
    if let Some(duration_secs) = audio_info.duration_seconds {
        println!("  Duration: {:.2}s", duration_secs);
    }
    println!();

    // Enable loudness monitoring if requested
    if lufs {
        streaming_manager
            .enable_loudness_monitoring()
            .map_err(|e| format!("Failed to enable loudness monitoring: {}", e))?;
        println!("Real-time LUFS monitoring enabled\n");
    }

    // Start playback with cancellation support
    let r_check = running.clone();
    tokio::select! {
        result = streaming_manager.start_playback(device, filters, map_mode, hwaudio_output, loudness) => {
            result.map_err(|e| format!("Failed to start streaming playback: {}", e))?;
        }
        _ = async {
            while r_check.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(100)).await;
            }
        } => {
            println!("Playback start cancelled");
            return Ok(());
        }
    }

    // Seek to start time if specified
    if start_time > 0.0 {
        println!("Seeking to {:.2}s...", start_time);
        streaming_manager
            .seek(start_time)
            .await
            .map_err(|e| format!("Failed to seek: {}", e))?;
    }

    println!("Streaming playback started successfully!");
    println!("Press Ctrl+C to stop\n");

    // Monitor playback
    let start_time_instant = std::time::Instant::now();
    let mut last_state = StreamingState::Idle;
    let mut last_shortterm: Option<f64> = None;

    while running.load(Ordering::SeqCst) {
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

        sleep(Duration::from_millis(100)).await;
    }

    // Stop playback with timeout
    println!("\nStopping streaming playback...");
    match tokio::time::timeout(Duration::from_secs(3), streaming_manager.stop()).await {
        Ok(result) => result.map_err(|e| format!("Failed to stop streaming: {}", e))?,
        Err(_) => {
            println!("Stop streaming timed out, forcing exit");
        }
    }

    println!("Streaming playback stopped successfully");
    Ok(())
}
