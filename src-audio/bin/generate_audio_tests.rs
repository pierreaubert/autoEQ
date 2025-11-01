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
//! - white_noise: white noise (flat spectrum)
//! - pink_noise: pink noise (1/f spectrum, -3dB/octave)
//! - m_noise: M-weighted noise (ITU-R 468 weighting for acoustic measurements)

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
const SWEEP_DURATION: f32 = 30.0; // Fixed duration for sweep

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SignalKind {
    Id,
    Thd1k,
    Thd100,
    ImdSmpte,
    ImdCcif,
    Sweep,
    WhiteNoise,
    PinkNoise,
    MNoise,
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
            Self::WhiteNoise => "white_noise",
            Self::PinkNoise => "pink_noise",
            Self::MNoise => "m_noise",
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
            Self::WhiteNoise,
            Self::PinkNoise,
            Self::MNoise,
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

    /// Number of channels (comma-separated, mono stereo 5.1 and 9.1.6)
    #[arg(long, value_delimiter = ',', default_values_t = vec![1, 2, 6, 16])]
    channels: Vec<u16>,

    /// Sample rates in Hz (comma-separated, should be enough to test most cases)
    #[arg(long = "sample-rates", value_delimiter = ',', default_values_t = vec![44100, 48000, 96000])]
    sample_rates: Vec<u32>,

    /// Bit depths (comma-separated, 16 or 24 only)
    #[arg(long, value_delimiter = ',', default_values_t = vec![16, 24])]
    bits: Vec<u16>,

    /// Signal types to generate (comma-separated)
    #[arg(long, value_delimiter = ',')]
    signals: Vec<SignalKind>,

    /// Duration in seconds (default 3.0, does not apply to sweep which is fixed at 10s)
    #[arg(long, default_value_t = 10.0)]
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
    Id {
        freqs: Vec<f32>,
    },
    Thd1k {
        freq: f32,
    },
    Thd100 {
        freq: f32,
    },
    ImdSmpte {
        freqs: [f32; 2],
        ratio: u8,
    },
    ImdCcif {
        freqs: [f32; 2],
    },
    Sweep {
        freq_start: f32,
        freq_end: f32,
        kind: String,
    },
    WhiteNoise {
        description: String,
    },
    PinkNoise {
        description: String,
    },
    MNoise {
        description: String,
        weighting: String,
    },
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
                eprintln!(
                    "Warning: Channel count {} out of range [1,16], skipping",
                    channels
                );
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

                    match generate_one(&cli.out_dir, *signal, channels, sr, bits, duration) {
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
            println!(
                "\nGenerated {} files. Manifest: {}",
                stats.generated,
                manifest_path.display()
            );
        }
        Err(e) => {
            eprintln!("Warning: Failed to write manifest: {}", e);
        }
    }

    println!(
        "Summary: Generated: {}, Skipped: {}, Failed: {}",
        stats.generated, stats.skipped, stats.failed
    );
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
            return Err(format!(
                "Nyquist violation: 1000 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Thd100 if 100.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 100 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::ImdSmpte if 7000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 7000 Hz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::ImdCcif if 20000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: 20 kHz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Sweep if 20000.0 >= nyquist => {
            return Err(format!(
                "Nyquist violation: sweep end 20 kHz >= {} Hz (skipped)",
                nyquist
            ));
        }
        SignalKind::Id => {
            let max_id_freq =
                (ID_BASE_FREQ + ID_STEP_FREQ * (channels as f32 - 1.0)).min(ID_MAX_FREQ);
            if max_id_freq >= nyquist {
                return Err(format!(
                    "Nyquist violation: max ID freq {} Hz >= {} Hz (skipped)",
                    max_id_freq, nyquist
                ));
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
            (
                data,
                SignalMetadata::ImdSmpte {
                    freqs: [60.0, 7000.0],
                    ratio: 4,
                },
            )
        }
        SignalKind::ImdCcif => {
            let mono = gen_two_tone(19000.0, CCIF_AMP, 20000.0, CCIF_AMP, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::ImdCcif {
                    freqs: [19000.0, 20000.0],
                },
            )
        }
        SignalKind::Sweep => {
            let mono = gen_log_sweep(20.0, 20000.0, AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::Sweep {
                    freq_start: 20.0,
                    freq_end: 20000.0,
                    kind: "log".to_string(),
                },
            )
        }
        SignalKind::WhiteNoise => {
            let mono = gen_white_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::WhiteNoise {
                    description: "Flat spectrum (white noise)".to_string(),
                },
            )
        }
        SignalKind::PinkNoise => {
            let mono = gen_pink_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::PinkNoise {
                    description: "1/f spectrum, -3dB/octave (pink noise)".to_string(),
                },
            )
        }
        SignalKind::MNoise => {
            let mono = gen_m_noise(AMP_STD, sr, duration);
            let data = replicate_mono(&mono, channels);
            (
                data,
                SignalMetadata::MNoise {
                    description: "M-weighted noise for acoustic measurements".to_string(),
                    weighting: "ITU-R 468".to_string(),
                },
            )
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

fn gen_white_noise(amp: f32, sr: u32, dur: f32) -> Vec<f32> {
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);

    // Simple LCG random number generator for deterministic output
    let mut seed: u64 = 1234567890;

    for _ in 0..n_frames {
        // LCG constants from Numerical Recipes
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        // Convert to [-1, 1] range
        let random = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        signal.push(clip(amp * random));
    }

    signal
}

fn gen_pink_noise(amp: f32, sr: u32, dur: f32) -> Vec<f32> {
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);

    // Voss-McCartney algorithm (Paul Kellett's implementation)
    // Uses multiple white noise generators at different rates
    let mut seed: u64 = 9876543210;
    let mut b0 = 0.0f32;
    let mut b1 = 0.0f32;
    let mut b2 = 0.0f32;
    let mut b3 = 0.0f32;
    let mut b4 = 0.0f32;
    let mut b5 = 0.0f32;
    let mut b6 = 0.0f32;

    for _ in 0..n_frames {
        // Generate white noise
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;

        // Update pink noise state at different rates
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.96900 * b2 + white * 0.1538520;
        b3 = 0.86650 * b3 + white * 0.3104856;
        b4 = 0.55000 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.0168980;

        let pink = b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362;
        b6 = white * 0.115926;

        // Normalize and scale (pink noise is ~3dB louder than white)
        signal.push(clip(amp * pink * 0.11));
    }

    signal
}

fn gen_m_noise(amp: f32, sr: u32, dur: f32) -> Vec<f32> {
    // M-weighted noise uses ITU-R 468 weighting curve
    // This is an approximation using a shaped white noise approach
    let n_frames = frames_for(dur, sr);
    let mut signal = Vec::with_capacity(n_frames);

    // Generate white noise first
    let mut seed: u64 = 1122334455;
    let mut noise_buffer = Vec::with_capacity(n_frames);

    for _ in 0..n_frames {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let white = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
        noise_buffer.push(white);
    }

    // Apply ITU-R 468 weighting approximation using IIR filters
    // This is a simplified version that boosts high frequencies (emphasis around 6.3 kHz)
    let mut hp_state = 0.0f32;

    // High-pass filter coefficient (cutoff around 30 Hz)
    let hp_coeff = 1.0 - (2.0 * PI * 30.0 / sr as f32).exp();

    // Peak filter coefficients (peak around 6300 Hz)
    let peak_freq = 6300.0;
    let peak_gain_db = 12.0; // ITU-R 468 has peak around 6.3 kHz
    let w0 = 2.0 * PI * peak_freq / sr as f32;
    let a = 10.0f32.powf(peak_gain_db / 40.0);

    for &white in &noise_buffer {
        // High-pass filter
        hp_state = hp_coeff * (hp_state + white);

        // Simplified peak boost (approximate ITU-R 468 weighting)
        let boosted = hp_state * (1.0 + (w0 * hp_state.abs()).sin() * a * 0.3);

        signal.push(clip(amp * boosted * 0.7));
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

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    match bits {
        16 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 32767.0).round() as i16;
                writer
                    .write_sample(pcm)
                    .map_err(|e| format!("Failed to write sample: {}", e))?;
            }
        }
        24 => {
            for &sample in interleaved {
                let pcm = (clip(sample) * 8388607.0).round() as i32;
                writer
                    .write_sample(pcm)
                    .map_err(|e| format!("Failed to write sample: {}", e))?;
            }
        }
        _ => return Err(format!("Unsupported bit depth: {}", bits)),
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}

// JSON writing

fn write_sidecar(path: &Path, sidecar: &Sidecar) -> Result<(), String> {
    let json = serde_json::to_string_pretty(sidecar)
        .map_err(|e| format!("Failed to serialize sidecar: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write sidecar: {}", e))?;
    Ok(())
}

fn write_manifest(path: &Path, files: &[String]) -> Result<(), String> {
    let manifest = serde_json::json!({ "files": files });
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(path, json).map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}
