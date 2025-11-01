#!/usr/bin/env rust
//! Generate audio test files for end-to-end audio validation.
//!
//! Generates WAV files in multiple channel counts, sample rates and bit depths.
//! Signals:
//! - id: per-channel identification tones (unique frequency per channel)
//! - thd1k: single-tone 1 kHz @ -3 dBFS (for THD)
//! - thd100: single-tone 100 Hz @ -3 dBFS (low-frequency THD)
//! - imd_smpte: SMPTE two-tone 60 Hz + 7 kHz (4:1 amplitude ratio)
//! - imd_ccif: CCIF two-tone 19 kHz + 20 kHz (equal amplitudes)
//! - sweep: logarithmic frequency sweep from 20 Hz to 20 kHz (10s fixed duration)

use clap::{Parser, ValueEnum};
use hound::{SampleFormat, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use serde_json;
use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

// Constants
const AMP_STD: f32 = 0.707; // ~-3 dBFS
const SMPTE_AMP1: f32 = 0.8; // 60 Hz amplitude
const SMPTE_AMP2: f32 = 0.2; // 7 kHz amplitude
const CCIF_AMP: f32 = 0.5; // 19/20 kHz equal amplitudes
const ID_BASE_FREQ: f32 = 300.0;
const ID_STEP_FREQ: f32 = 300.0;
const ID_MAX_FREQ: f32 = 6000.0;
const SWEEP_DURATION: f32 = 10.0; // Fixed duration for sweep

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SignalKind {
    Id,
    Thd1k,
    Thd100,
    ImdSmpte,
    ImdCcif,
    Sweep,
}

impl SignalKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Thd1k => "thd1k",
            Self::Thd100 => "thd100",
            Self::ImdSmpte => "imd_smpte",
            Self::ImdCcif => "imd_ccif",
            Self::Sweep => "sweep",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::Id,
            Self::Thd1k,
            Self::Thd100,
            Self::ImdSmpte,
            Self::ImdCcif,
            Self::Sweep,
        ]
    }
}

#[derive(Parser)]
#[command(name = "generate_audio_tests")]
#[command(about = "Generate audio test files for validation", long_about = None)]
struct Cli {
    /// Output directory
    #[arg(long, default_value = "target/test-audio")]
    out_dir: PathBuf,

    /// Number of channels (comma-separated)
    #[arg(long, value_delimiter = ',', default_values_t = vec![2, 6, 16])]
    channels: Vec<u16>,

    /// Sample rates in Hz (comma-separated)
    #[arg(long = "sample-rates", value_delimiter = ',', default_values_t = vec![44100, 96000, 192000])]
    sample_rates: Vec<u32>,

    /// Bit depths (comma-separated, 16 or 24 only)
    #[arg(long, value_delimiter = ',', default_values_t = vec![16, 24])]
    bits: Vec<u16>,

    /// Signal types to generate (comma-separated)
    #[arg(long, value_delimiter = ',')]
    signals: Vec<SignalKind>,

    /// Duration in seconds (default 3.0, does not apply to sweep which is fixed at 10s)
    #[arg(long, default_value_t = 3.0)]
    duration: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Sidecar {
    format: String,
    channels: u16,
    sample_rate: u32,
    bits: u16,
    duration: f32,
    signal: SignalMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum SignalMetadata {
    Id { freqs: Vec<f32> },
    Thd1k { freq: f32 },
    Thd100 { freq: f32 },
    ImdSmpte { freqs: [f32; 2], ratio: u8 },
    ImdCcif { freqs: [f32; 2] },
    Sweep { freq_start: f32, freq_end: f32, kind: String },
}

#[derive(Debug)]
struct GenerationStats {
    generated: usize,
    skipped: usize,
    failed: usize,
}

impl GenerationStats {
    fn new() -> Self {
        Self {
            generated: 0,
            skipped: 0,
            failed: 0,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // Validate bit depths
    for &bits in &cli.bits {
        if bits != 16 && bits != 24 {
            eprintln!("Error: Bit depth must be 16 or 24, got {}", bits);
            std::process::exit(1);
        }
    }

    // If no signals specified, use all
    let signals = if cli.signals.is_empty() {
        SignalKind::all()
    } else {
        cli.signals.clone()
    };

    // Create output directory
    if let Err(e) = fs::create_dir_all(&cli.out_dir) {
        eprintln!("Error: Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    let mut stats = GenerationStats::new();
    let mut manifest_files = Vec::new();

    // Generate all combinations
    for signal in &signals {
        for &channels in &cli.channels {
            if channels < 1 || channels > 16 {
                eprintln!("Warning: Channel count {} out of range [1,16], skipping", channels);
                stats.skipped += 1;
                continue;
            }

            for &sr in &cli.sample_rates {
                for &bits in &cli.bits {
                    let duration = if *signal == SignalKind::Sweep {
                        SWEEP_DURATION
                    } else {
                        cli.duration
                    };

                    match generate_one(
                        &cli.out_dir,
                        *signal,
                        channels,
                        sr,
                        bits,
                        duration,
                    ) {
                        Ok(path) => {
                            manifest_files.push(path.to_string_lossy().to_string());
                            stats.generated += 1;
                        }
                        Err(e) => {
                            if e.contains("Nyquist") || e.contains("skipped") {
                                stats.skipped += 1;
                            } else {
                                eprintln!(
                                    "Warning: Failed to generate {} ch{} sr{} b{}: {}",
                                    signal.as_str(),
                                    channels,
                                    sr,
                                    bits,
                                    e
                                );
                                stats.failed += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Write manifest
    let manifest_path = cli.out_dir.join("manifest.json");
    match write_manifest(&manifest_path, &manifest_files) {
        Ok(_) => {
            println!("\nGenerated {} files. Manifest: {}", stats.generated, manifest_path.display());
        }
        Err(e) => {
            eprintln!("Warning: Failed to write manifest: {}", e);
        }
    }

    println!("Summary: Generated: {}, Skipped: {}, Failed: {}", 
             stats.generated, stats.skipped, stats.failed);
}

fn generate_one(
    out_dir: &Path,
    signal: SignalKind,
    channels: u16,
    sr: u32,
    bits: u16,
    duration: f32,
) -> Result<PathBuf, String> {
    let nyquist = sr as f32 / 2.0;

    // Check Nyquist violations
    match signal {
        SignalKind::Thd1k if 1000.0 >= nyquist => {
            return Err(format!("Nyquist violation: 1000 Hz >= {} Hz (skipped)", nyquist));
        }
        SignalKind::Thd100 if 100.0 >= nyquist => {
            return Err(format!("Nyquist violation: 100 Hz >= {} Hz (skipped)", nyquist));
        }
        SignalKind::ImdSmpte if 7000.0 >= nyquist => {
            return Err(format!("Nyquist violation: 7000 Hz >= {} Hz (skipped)", nyquist));
        }
        SignalKind::ImdCcif if 20000.0 >= nyquist => {
            return Err(format!("Nyquist violation: 20 kHz >= {} Hz (skipped)", nyquist));
        }
        SignalKind::Sweep if 20000.0 >= nyquist => {
            return Err(format!("Nyquist violation: sweep end 20 kHz >= {} Hz (skipped)", nyquist));
        }
        SignalKind::Id => {
            let max_id_freq = (ID_BASE_FREQ + ID_STEP_FREQ * (channels as f32 - 1.0)).min(ID_MAX_FREQ);
            if max_id_freq >= nyquist {
                return Err(format!("Nyquist violation: max ID freq {} Hz >= {} Hz (skipped)", max_id_freq, nyquist));
            }
        }
        _ => {}
    }

    // Generate signal data
    let (audio_data, metadata) = match signal {
        SignalKind::Id => {
            let mut freqs = Vec::new();
            let mut per_channel = Vec::new();
            for ch in 0..channels {
                let freq = (ID_BASE_FREQ * (ch as f32 + 1.0)).min(ID_MAX_FREQ);
                freqs.push(freq);
                per_channel.push(gen_tone(freq, AMP_STD, sr, duration));
            }
            let data = interleave_per_channel(&per_channel);
            (data, SignalMetadata::Id { freqs })
        }
        SignalKind::Thd1k => {
            let mono = gen_tone(1000.0, AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::Thd1k { freq: 1000.0 })
        }
        SignalKind::Thd100 => {
            let mono = gen_tone(100.0, AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::Thd100 { freq: 100.0 })
        }
        SignalKind::ImdSmpte => {
            let mono = gen_two_tone(60.0, SMPTE_AMP1, 7000.0, SMPTE_AMP2, sr, duration);
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::ImdSmpte { freqs: [60.0, 7000.0], ratio: 4 })
        }
        SignalKind::ImdCcif => {
            let mono = gen_two_tone(19000.0, CCIF_AMP, 20000.0, CCIF_AMP, sr, duration);
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::ImdCcif { freqs: [19000.0, 20000.0] })
        }
        SignalKind::Sweep => {
            let mono = gen_log_sweep(20.0, 20000.0, AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (data, SignalMetadata::Sweep {
                freq_start: 20.0,
                freq_end: 20000.0,
                kind: "log".to_string(),
            })
        }
    };

    // Create output directory structure
    let subdir = out_dir.join("wav").join(signal.as_str());
    fs::create_dir_all(&subdir).map_err(|e| format!("Failed to create directory: {}", e))?;

    // Build filename
    let filename = format!("{}_ch{}_sr{}_b{}.wav", signal.as_str(), channels, sr, bits);
    let wav_path = subdir.join(&filename);

    // Write WAV file
    write_wav(&wav_path, &audio_data, sr, channels, bits)?;

    // Write sidecar JSON
    let sidecar = Sidecar {
        format: "wav".to_string(),
        channels,
        sample_rate: sr,
        bits,
        duration,
        signal: metadata,
    };

    let sidecar_path = wav_path.with_extension("wav.json");
    write_sidecar(&sidecar_path, &sidecar)?;

    Ok(wav_path)
}

// DSP Helpers

fn clip(x: f32) -> f32 {
    x.clamp(-0.999_999, 0.999_999)
}

fn frames_for(duration: f32, sr: u32) -> usize {
    (duration * sr as f32).round() as usize
}

// Signal Generators

fn gen_tone(freq: f32, amp: f32, sr: u32, dur: f32) -> Vec<f32> {
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi = 2.0 * PI * freq / sr as f32;
    let mut phase: f32 = 0.0;

    for _ in 0..n_frames {
        signal.push(clip(amp * phase.sin()));
        phase += dphi;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }

    signal
}

fn gen_two_tone(f1: f32, a1: f32, f2: f32, a2: f32, sr: u32, dur: f32) -> Vec<f32> {
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);
    let dphi1 = 2.0 * PI * f1 / sr as f32;
    let dphi2 = 2.0 * PI * f2 / sr as f32;
    let mut phase1: f32 = 0.0;
    let mut phase2: f32 = 0.0;

    for _ in 0..n_frames {
        let sample = a1 * phase1.sin() + a2 * phase2.sin();
        signal.push(clip(sample));
        phase1 += dphi1;
        phase2 += dphi2;
        if phase1 > 2.0 * PI {
            phase1 -= 2.0 * PI;
        }
        if phase2 > 2.0 * PI {
            phase2 -= 2.0 * PI;
        }
    }

    signal
}

fn gen_log_sweep(f_start: f32, f_end: f32, amp: f32, sr: u32, dur: f32) -> Vec<f32> {
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);
    
    let k = (f_end / f_start).ln() / dur;
    let coefficient = 2.0 * PI * f_start / k;

    for n in 0..n_frames {
        let t = n as f32 / sr as f32;
        let phase = coefficient * ((k * t).exp() - 1.0);
        signal.push(clip(amp * phase.sin()));
    }

    signal
}

// Channel operations

fn interleave_per_channel(per_channel: &[Vec<f32>]) -> Vec<f32> {
    let n_channels = per_channel.len();
    if n_channels == 0 {
        return Vec::new();
    }
    let n_frames = per_channel[0].len();
    let mut interleaved = Vec::with_capacity(n_frames * n_channels);

    for frame in 0..n_frames {
        for ch in 0..n_channels {
            interleaved.push(per_channel[ch][frame]);
        }
    }

    interleaved
}

fn replicate_mono(mono: &[f32], channels: u16) -> Vec<f32> {
    let n_frames = mono.len();
    let mut interleaved = Vec::with_capacity(n_frames * channels as usize);

    for &sample in mono {
        for _ in 0..channels {
            interleaved.push(sample);
        }
    }

    interleaved
}

// WAV writing

fn write_wav(
    path: &Path,
    interleaved: &[f32],
    sr: u32,
    channels: u16,
    bits: u16,
) -> Result<(), String> {
    let spec = WavSpec {
        channels,
        sample_rate: sr,
        bits_per_sample: bits,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    match bits {
        16 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 32767.0).round() as i16;
                writer.write_sample(pcm)
                    .map_err(|e| format!("Failed to write sample: {}", e))?;
            }
        }
        24 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 8388607.0).round() as i32;
                writer.write_sample(pcm)
                    .map_err(|e| format!("Failed to write sample: {}", e))?;
            }
        }
        _ => return Err(format!("Unsupported bit depth: {}", bits)),
    }

    writer.finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}

// JSON writing

fn write_sidecar(path: &Path, sidecar: &Sidecar) -> Result<(), String> {
    let json = serde_json::to_string_pretty(sidecar)
        .map_err(|e| format!("Failed to serialize sidecar: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write sidecar: {}", e))?;
    Ok(())
}

fn write_manifest(path: &Path, files: &[String]) -> Result<(), String> {
    let manifest = serde_json::json!({ "files": files });
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}
