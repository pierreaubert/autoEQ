use clap::{Parser, Subcommand};
use sotf_backend::{
    AudioFormat, AudioManager, AudioState, AudioStreamingManager, CamillaError, FilterParams,
    StreamingState, audio, probe_file,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, sleep};

#[derive(Parser)]
#[command(name = "audio_test")]
#[command(about = "Test CamillaDSP audio wrapper without Tauri", long_about = None)]
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

    /// Play an audio file with optional EQ filters (auto-detects format)
    Play {
        /// Path to audio file (WAV for direct play, FLAC for streaming)
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// Sample rate in Hz
        #[arg(short = 'r', long, default_value = "48000")]
        sample_rate: u32,

        /// Number of channels
        #[arg(short, long, default_value = "2")]
        channels: u16,

        /// EQ filters in format "freq:q:gain" (e.g., "1000:1.5:3.0")
        #[arg(short, long = "filter", value_name = "FREQ:Q:GAIN")]
        filters: Vec<String>,

        /// Hardware input channel map (comma-separated indices)
        #[arg(long = "hwaudio-input", value_delimiter = ',')]
        hwaudio_input: Option<Vec<u16>>,

        /// Hardware output channel map (comma-separated indices)
        #[arg(long = "hwaudio-output", value_delimiter = ',')]
        hwaudio_output: Option<Vec<u16>>,

        /// Swap left and right channels (useful to check channel mapping)
        #[arg(long = "swap-channels", default_value_t = false)]
        swap_channels: bool,

        /// Duration to play in seconds (0 = play until stopped)
        #[arg(short = 't', long, default_value = "0")]
        duration: u64,

        /// Buffer size in chunks (32=low latency, 128=balanced, 1024=high reliability)
        #[arg(long = "buffer-chunks", default_value = "128")]
        buffer_chunks: usize,
    },

    /// Play FLAC file with streaming decoder (supports seeking)
    FlacStream {
        /// Path to FLAC audio file
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Output device name (optional, uses default)
        #[arg(short, long)]
        device: Option<String>,

        /// EQ filters in format "freq:q:gain" (e.g., "1000:1.5:3.0")
        #[arg(short, long = "filter", value_name = "FREQ:Q:GAIN")]
        filters: Vec<String>,

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
        Commands::Play {
            file,
            device,
            sample_rate,
            channels,
            filters,
            hwaudio_input,
            hwaudio_output,
            swap_channels,
            duration,
            buffer_chunks,
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

            if let Err(e) = play_audio(
                binary_path,
                file,
                device,
                sample_rate,
                channels,
                filter_params,
                duration,
                map_mode,
                hwaudio_input,
                hwaudio_output,
                buffer_chunks,
            )
            .await
            {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::FlacStream {
            file,
            device,
            filters,
            hwaudio_output,
            swap_channels,
            duration,
            start_time,
            buffer_chunks,
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

            if let Err(e) = play_flac_stream(
                binary_path,
                file,
                device,
                filter_params,
                duration,
                start_time,
                map_mode,
                hwaudio_output,
                buffer_chunks,
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

async fn play_audio(
    binary_path: PathBuf,
    file: PathBuf,
    device: Option<String>,
    sample_rate: u32,
    channels: u16,
    filters: Vec<FilterParams>,
    duration: u64,
    map_mode: sotf_backend::camilla::ChannelMapMode,
    _hwaudio_input: Option<Vec<u16>>,
    hwaudio_output: Option<Vec<u16>>,
    buffer_chunks: usize,
) -> Result<(), String> {
    println!("Starting playback...");
    println!("  File: {:?}", file);

    // Try to detect file format
    match probe_file(&file) {
        Ok((format, spec)) => {
            println!(
                "  Format: {} ({}Hz, {}ch, {}bit)",
                format.as_str(),
                spec.sample_rate,
                spec.channels,
                spec.bits_per_sample
            );

            // Route FLAC files to streaming manager
            if format == AudioFormat::Flac {
                println!("  Using FLAC streaming decoder...");
                return play_flac_stream(
                    binary_path,
                    file,
                    device,
                    filters,
                    duration,
                    0.0, // start_time
                    map_mode,
                    hwaudio_output,
                    buffer_chunks,
                )
                .await;
            }
        }
        Err(e) => {
            println!(
                "  Warning: Could not detect format ({}), using legacy playback",
                e
            );
        }
    }

    println!("  Device: {:?}", device.as_deref().unwrap_or("default"));
    println!("  Sample rate: {}Hz", sample_rate);
    println!("  Channels: {}", channels);
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

    // Create audio manager
    let manager = AudioManager::new(binary_path);

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\n\nReceived Ctrl+C, stopping playback...");
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| format!("Failed to set Ctrl+C handler: {}", e))?;

    // Start playback with cancellation support
    let r_check = running.clone();
    tokio::select! {
        result = manager.start_playback(file, device, sample_rate, channels, filters, map_mode, hwaudio_output) => {
            result.map_err(|e| format!("Failed to start playback: {}", e))?;
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

    println!("Playback started successfully!");
    println!("Press Ctrl+C to stop\n");

    // Monitor playback
    let start_time = std::time::Instant::now();
    let mut last_peak = 0.0f32;

    while running.load(Ordering::SeqCst) {
        // Get current state
        let state = manager
            .get_state()
            .map_err(|e| format!("Failed to get state: {}", e))?;

        // Check if still playing
        if state.state != AudioState::Playing {
            println!("Playback stopped (state: {:?})", state.state);
            break;
        }

        // Get signal peak
        if let Ok(peak) = manager.get_signal_peak().await {
            if (peak - last_peak).abs() > 1.0 {
                // Only print if change is significant
                last_peak = peak;
                print!("\rSignal: {:.1} dB    ", peak);
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }
        }

        // Check duration
        if duration > 0 && start_time.elapsed().as_secs() >= duration {
            println!("\n\nDuration reached, stopping...");
            break;
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Stop playback with timeout
    println!("\nStopping playback...");
    match tokio::time::timeout(Duration::from_secs(3), manager.stop_playback()).await {
        Ok(result) => result.map_err(|e| format!("Failed to stop playback: {}", e))?,
        Err(_) => {
            println!("Stop playback timed out, forcing exit");
        }
    }

    println!("Playback stopped successfully");
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

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\n\nReceived Ctrl+C, stopping recording...");
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| format!("Failed to set Ctrl+C handler: {}", e))?;

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

async fn play_flac_stream(
    binary_path: PathBuf,
    file: PathBuf,
    device: Option<String>,
    filters: Vec<FilterParams>,
    duration: u64,
    start_time: f64,
    map_mode: sotf_backend::camilla::ChannelMapMode,
    hwaudio_output: Option<Vec<u16>>,
    buffer_chunks: usize,
) -> Result<(), String> {
    println!("Starting FLAC streaming playback...");
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

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\n\nReceived Ctrl+C, stopping playback...");
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| format!("Failed to set Ctrl+C handler: {}", e))?;

    // Load the FLAC file
    let r_check = running.clone();
    let audio_info = tokio::select! {
        result = streaming_manager.load_file(&file) => {
            result.map_err(|e| format!("Failed to load FLAC file: {}", e))?
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

    println!("Loaded FLAC file:");
    println!("  Format: {}", audio_info.format);
    println!("  Sample rate: {}Hz", audio_info.spec.sample_rate);
    println!("  Channels: {}", audio_info.spec.channels);
    println!("  Bits per sample: {}", audio_info.spec.bits_per_sample);
    if let Some(duration_secs) = audio_info.duration_seconds {
        println!("  Duration: {:.2}s", duration_secs);
    }
    println!();

    // Start playback with cancellation support
    let r_check = running.clone();
    tokio::select! {
        result = streaming_manager.start_playback(device, filters, map_mode, hwaudio_output) => {
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

    println!("FLAC streaming started successfully!");
    println!("Press Ctrl+C to stop\n");

    // Monitor playback
    let start_time_instant = std::time::Instant::now();
    let mut last_state = StreamingState::Idle;

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

        // Check duration
        if duration > 0 && start_time_instant.elapsed().as_secs() >= duration {
            println!("\n\nDuration reached, stopping...");
            break;
        }

        sleep(Duration::from_millis(200)).await;
    }

    // Stop playback with timeout
    println!("\nStopping FLAC streaming...");
    match tokio::time::timeout(Duration::from_secs(3), streaming_manager.stop()).await {
        Ok(result) => result.map_err(|e| format!("Failed to stop streaming: {}", e))?,
        Err(_) => {
            println!("Stop streaming timed out, forcing exit");
        }
    }

    println!("FLAC streaming stopped successfully");
    Ok(())
}
