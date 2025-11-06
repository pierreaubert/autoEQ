use clap::{Parser, Subcommand};
use sotf_audio::loudness_compensation::LoudnessCompensation;
use sotf_audio::{AudioStreamingManager, CamillaError, FilterParams, StreamingState};
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
    /// - Sample rate and channels from the file are used automatically.
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

        /// Hardware output channel map (comma-separated indices)
        #[arg(long = "hwaudio-play", value_delimiter = ',')]
        hwaudio_play: Option<Vec<u16>>,

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
        #[arg(long = "buffer-chunks", default_value = "32")]
        buffer_chunks: usize,

        /// Enable real-time LUFS monitoring (prints momentary/short-term loudness)
        #[arg(long = "lufs", alias = "monitor-lufs", default_value_t = false)]
        lufs: bool,

        /// Loudness compensation: 2 or 3 floats: REF LOW [HIGH] (dB; REF -100..20, boosts 0..20)
        #[arg(long = "loudness-compensation", value_name = "REF,LOW[,HIGH]", value_parser = clap::value_parser!(f64), value_delimiter = ',')]
        loudness_compensation: Option<Vec<f64>>,
    },

    /// Generate and record test signals with analysis
    Record {
        /// Signal type: tone, two-tone, sweep, white-noise, pink-noise, m-noise
        #[arg(long)]
        signal: String,

        /// Duration in seconds
        #[arg(long)]
        duration: f32,

        /// Sample rate in Hz
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Number of signal channels (must be 1)
        #[arg(long, default_value = "1")]
        channels: u16,

        /// Hardware output channel to send signal to (0-based, single channel only)
        #[arg(long)]
        hwaudio_send_to: String,

        /// Hardware input channels to record from (0-based, comma-separated)
        #[arg(long)]
        hwaudio_record_from: String,

        /// Optional filename prefix
        #[arg(long)]
        name: Option<String>,

        /// Output device name (optional, uses default)
        #[arg(long = "output-device")]
        output_device: Option<String>,

        /// Input device name (optional, uses default)
        #[arg(long = "input-device")]
        input_device: Option<String>,

        // Signal-specific parameters
        /// Tone frequency in Hz (for tone signal)
        #[arg(long)]
        freq: Option<f32>,

        /// First frequency in Hz (for two-tone signal)
        #[arg(long)]
        freq1: Option<f32>,

        /// Second frequency in Hz (for two-tone signal)
        #[arg(long)]
        freq2: Option<f32>,

        /// Start frequency in Hz (for sweep signal)
        #[arg(long, default_value = "5")]
        start_freq: Option<f32>,

        /// End frequency in Hz (for sweep signal)
        #[arg(long, default_value = "22000")]
        end_freq: Option<f32>,

        /// Amplitude (0.0-1.0)
        #[arg(long)]
        amp: Option<f32>,

        /// First amplitude (0.0-1.0, for two-tone signal)
        #[arg(long)]
        amp1: Option<f32>,

        /// Second amplitude (0.0-1.0, for two-tone signal)
        #[arg(long)]
        amp2: Option<f32>,
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
        None => match sotf_audio::camilla::find_camilladsp_binary() {
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
            sample_rate: _,
            channels: _,
            filters,
            hwaudio_play,
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
                sotf_audio::camilla::ChannelMapMode::Swap
            } else {
                sotf_audio::camilla::ChannelMapMode::Normal
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
                hwaudio_play,
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
            signal,
            duration,
            sample_rate,
            channels,
            hwaudio_send_to,
            hwaudio_record_from,
            name,
            output_device,
            input_device,
            freq,
            freq1,
            freq2,
            start_freq,
            end_freq,
            amp,
            amp1,
            amp2,
        } => {
            if let Err(e) = record_signal(
                binary_path,
                signal,
                duration,
                sample_rate,
                channels,
                hwaudio_send_to,
                hwaudio_record_from,
                name,
                output_device,
                input_device,
                freq,
                freq1,
                freq2,
                start_freq,
                end_freq,
                amp,
                amp1,
                amp2,
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
    map_mode: sotf_audio::camilla::ChannelMapMode,
    hwaudio_play: Option<Vec<u16>>,
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
        result = streaming_manager.start_playback(device, filters, map_mode, hwaudio_play, loudness) => {
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

#[allow(clippy::too_many_arguments)]
pub async fn record_signal(
    _binary_path: PathBuf,
    signal: String,
    duration: f32,
    sample_rate: u32,
    channels: u16,
    hwaudio_send_to: String,
    hwaudio_record_from: String,
    name: Option<String>,
    _output_device: Option<String>,
    _input_device: Option<String>,
    freq: Option<f32>,
    freq1: Option<f32>,
    freq2: Option<f32>,
    start_freq: Option<f32>,
    end_freq: Option<f32>,
    amp: Option<f32>,
    amp1: Option<f32>,
    amp2: Option<f32>,
) -> Result<(), String> {
    use sotf_audio::signal_recorder::*;

    println!("{}", "=".repeat(60));
    println!("Signal Recording and Analysis");
    println!("{}", "=".repeat(60));

    // Validate channels
    if channels != 1 {
        return Err(format!(
            "Channels must be 1 (mono signal generation), got {}",
            channels
        ));
    }

    // Parse signal type
    let signal_type = SignalType::from_str(&signal)?;

    // Parse channel lists
    let send_to_channels = parse_channel_list(&hwaudio_send_to)?;
    let record_from_channels = parse_channel_list(&hwaudio_record_from)?;

    // Validate that we have at least one send-to channel
    if send_to_channels.is_empty() {
        return Err("hwaudio-send-to must specify at least 1 channel".to_string());
    }

    // Validate that the number of send and record channels match
    if send_to_channels.len() != record_from_channels.len() {
        return Err(format!(
            "Number of send-to channels ({}) must equal number of record-from channels ({})",
            send_to_channels.len(),
            record_from_channels.len()
        ));
    }

    // Build signal parameters based on signal type
    let params = match signal_type {
        SignalType::Tone => {
            let freq = freq.ok_or("--freq is required for tone signal")?;
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Tone { freq, amp }
        }
        SignalType::TwoTone => {
            let freq1 = freq1.ok_or("--freq1 is required for two-tone signal")?;
            let freq2 = freq2.ok_or("--freq2 is required for two-tone signal")?;
            let amp1 = amp1.unwrap_or(0.5).clamp(0.0, 1.0);
            let amp2 = amp2.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::TwoTone {
                freq1,
                amp1,
                freq2,
                amp2,
            }
        }
        SignalType::Sweep => {
            let start_freq = start_freq.ok_or("--start-freq is required for sweep signal")?;
            let end_freq = end_freq.ok_or("--end-freq is required for sweep signal")?;
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Sweep {
                start_freq,
                end_freq,
                amp,
            }
        }
        SignalType::WhiteNoise | SignalType::PinkNoise | SignalType::MNoise => {
            let amp = amp.unwrap_or(0.5).clamp(0.0, 1.0);
            SignalParams::Noise { amp }
        }
    };

    // Validate parameters
    validate_signal_params(signal_type, &params, duration, sample_rate)?;

    println!("\nConfiguration:");
    println!("  Signal: {}", signal_type.as_str());
    println!("  Duration: {:.2}s", duration);
    println!("  Sample rate: {}Hz", sample_rate);
    println!("  Channel pairs (send → record):");
    for (&send_ch, &record_ch) in send_to_channels.iter().zip(record_from_channels.iter()) {
        println!("    hw output {} → hw input {}", send_ch, record_ch);
    }
    println!(
        "  Total recordings: {} (one mono file per pair)",
        send_to_channels.len()
    );
    if let Some(ref n) = name {
        println!("  Output prefix: {}", n);
    }
    println!();

    // Generate the base signal
    let total_recordings = send_to_channels.len(); // One recording per send/record pair
    println!("[1/{}] Generating signal...", total_recordings + 2);
    let base_signal = generate_signal(signal_type, &params, duration, sample_rate)?;

    // Validate that the signal is mono (Vec<f32> represents mono)
    // All our signal generation functions return mono signals
    println!(
        "  ✓ Generated mono signal with {} samples",
        base_signal.len()
    );

    // Prepare mono signal with fades and padding
    println!("\n[2/{}] Preparing mono signal...", total_recordings + 2);
    let prepared_signal = prepare_signal(base_signal.clone(), sample_rate);
    println!(
        "  ✓ Prepared mono signal with {} samples",
        prepared_signal.len()
    );

    // Perform recording for each send/record channel pair (one-to-one mapping)
    // Each send channel is paired with exactly one record channel
    for (idx, (&send_ch, &record_ch)) in send_to_channels
        .iter()
        .zip(record_from_channels.iter())
        .enumerate()
    {
        println!(
            "\n[{}/{}] Playing to hw channel {}, recording from hw channel {}...",
            idx + 3,
            total_recordings + 2,
            send_ch,
            record_ch
        );

        // Generate output filenames - include both send and record channels
        let (wav_path, csv_path) = generate_output_filenames_stereo(
            name.as_deref(),
            signal_type,
            send_ch,
            record_ch,
            sample_rate,
        );

        println!("  Output WAV: {:?}", wav_path);
        println!("  Output CSV: {:?}", csv_path);

        // Write mono signal to temporary WAV file
        println!("  Writing temporary mono WAV file...");
        let temp_wav = write_temp_wav(&prepared_signal, sample_rate, 1)?;
        println!("  Temp file: {:?}", temp_wav.path());

        // Perform actual playback and recording
        println!("  Starting playback and recording...");
        println!("  Playing mono signal to hw output channel {}", send_ch);
        println!("  Recording mono from hw input channel {}", record_ch);

        record_and_analyze(
            temp_wav.path(),  // Use the temporary WAV file for playback
            &wav_path,        // Record to the final output WAV file
            &prepared_signal, // Use the prepared mono signal for analysis
            sample_rate,
            &csv_path,
            send_ch,   // Output channel
            record_ch, // Input channel
        )
        .await?;

        println!("  ✓ Recording complete");
    }

    println!("\n{}", "=".repeat(60));
    println!("All recordings complete!");
    println!("{}", "=".repeat(60));

    Ok(())
}
