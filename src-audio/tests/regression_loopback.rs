//! Regression Tests for Audio Recording & Analysis
//!
//! This test suite verifies:
//! 1. Playback works correctly with valid config
//! 2. Recording can generate each kind of test signal (tone, sweep, pink noise)
//! 3. Recording supports channel mapping (both input and output)
//! 4. Loopback can demonstrably send a signal and read it back accurately
//!
//! Tests are gated by AEQ_E2E=1 environment variable and require:
//! - Audio interface with loopback capability
//! - CamillaDSP binary in PATH
//! - Hardware channel mapping via environment variables:
//!   - AEQ_E2E_SEND_CH: Hardware channel to send to (e.g., "1")
//!   - AEQ_E2E_RECORD_CH: Hardware channel to record from (e.g., "1")
//!   - AEQ_E2E_SR: Sample rate (default: 48000)

use sotf_audio::analysis::{analyze_recording, write_analysis_csv};
use sotf_audio::camilla::{AudioManager, ChannelMapMode, find_camilladsp_binary};
use sotf_audio::signals::{gen_log_sweep, gen_pink_noise, gen_tone};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio;

// ============================================================================
// Test Configuration
// ============================================================================

fn should_run_e2e_tests() -> bool {
    env::var("AEQ_E2E").ok().as_deref() == Some("1")
}

fn get_test_config() -> TestConfig {
    TestConfig {
        sample_rate: env::var("AEQ_E2E_SR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(48000),
        send_channel: env::var("AEQ_E2E_SEND_CH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        record_channel: env::var("AEQ_E2E_RECORD_CH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
    }
}

struct TestConfig {
    sample_rate: u32,
    send_channel: u16,
    record_channel: u16,
}

fn test_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("regression-tests")
}

// Helper to write WAV file for playback
fn write_wav_file(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    use hound::{WavSpec, WavWriter};
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).unwrap();
    for &sample in samples {
        let sample_i32 = (sample.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
        writer.write_sample(sample_i32).unwrap();
    }
    writer.finalize().unwrap();
}

// ============================================================================
// Test 1: Playback Works with Valid Config
// ============================================================================

#[tokio::test]
async fn test_playback_valid_config() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1). Set AEQ_E2E=1 to run.");
        return;
    }

    let config = get_test_config();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== Test 1: Playback with Valid Config ===");
    println!("Sample rate: {} Hz", config.sample_rate);
    println!("Send channel: {}", config.send_channel);

    // Generate a 2-second 1kHz tone
    let tone = gen_tone(1000.0, 0.5, config.sample_rate, 2.0);
    let playback_file = output_dir.join("test_playback_tone.wav");
    write_wav_file(&playback_file, &tone, config.sample_rate);

    // Start playback
    let manager = AudioManager::new(find_camilladsp_binary().unwrap());

    let result = manager
        .start_playback(
            playback_file.clone(),
            None, // Use default device
            config.sample_rate,
            1,
            Vec::new(), // No filters
            ChannelMapMode::Normal,
            Some(vec![config.send_channel]),
            None,
        )
        .await;

    assert!(
        result.is_ok(),
        "Playback failed to start: {:?}",
        result.err()
    );

    println!("✓ Playback started successfully");

    // Let it play
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Stop playback
    let stop_result = manager.stop_playback().await;
    assert!(
        stop_result.is_ok(),
        "Playback failed to stop: {:?}",
        stop_result.err()
    );

    println!("✓ Playback stopped successfully");
    println!("✓ Test 1 passed: Playback works with valid config\n");
}

// ============================================================================
// Test 2: Recording Can Generate Each Signal Type
// ============================================================================

#[tokio::test]
async fn test_recording_all_signal_types() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }

    let config = get_test_config();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== Test 2: Recording Each Signal Type ===");

    let signal_types = vec![
        ("tone", gen_tone(1000.0, 0.5, config.sample_rate, 2.0)),
        (
            "sweep",
            gen_log_sweep(20.0, 20000.0, 0.5, config.sample_rate, 3.0),
        ),
        ("pink_noise", gen_pink_noise(0.3, config.sample_rate, 2.0)),
    ];

    for (signal_name, reference_signal) in signal_types {
        println!("Testing signal type: {}", signal_name);

        let playback_file = output_dir.join(format!("test_signal_{}.wav", signal_name));
        let record_file = output_dir.join(format!("test_record_{}.wav", signal_name));

        write_wav_file(&playback_file, &reference_signal, config.sample_rate);

        // Start recording
        let rec_manager = AudioManager::new(find_camilladsp_binary().unwrap());
        rec_manager
            .start_recording(
                None,
                record_file.clone(),
                config.sample_rate,
                1,
                Some(vec![config.record_channel]),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start playback
        let play_manager = AudioManager::new(find_camilladsp_binary().unwrap());
        play_manager
            .start_playback(
                playback_file.clone(),
                None,
                config.sample_rate,
                1,
                Vec::new(),
                ChannelMapMode::Normal,
                Some(vec![config.send_channel]),
                None,
            )
            .await
            .unwrap();

        // Wait for recording to complete
        let duration = reference_signal.len() as f32 / config.sample_rate as f32;
        tokio::time::sleep(Duration::from_secs_f32(duration + 2.0)).await;

        // Stop both
        play_manager.stop_playback().await.unwrap();
        rec_manager.stop_recording().await.unwrap();

        // Verify recorded file exists and is valid
        assert!(
            record_file.exists(),
            "Recording file not created for {}",
            signal_name
        );

        let reader = hound::WavReader::open(&record_file);
        assert!(
            reader.is_ok(),
            "Cannot open recorded WAV file for {}: {:?}",
            signal_name,
            reader.err()
        );

        let mut reader = reader.unwrap();
        let spec = reader.spec();
        assert_eq!(
            spec.sample_rate, config.sample_rate,
            "Sample rate mismatch for {}",
            signal_name
        );

        // Read samples and verify non-zero content
        let samples: Vec<i32> = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();
        let non_zero = samples.iter().filter(|&&s| s.abs() > 100).count();
        let non_zero_percent = (non_zero as f32 / samples.len() as f32) * 100.0;

        assert!(
            non_zero_percent > 10.0,
            "Recording for {} contains mostly zeros ({:.1}% non-zero)",
            signal_name,
            non_zero_percent
        );

        println!(
            "  ✓ {} recorded successfully ({:.1}% non-zero)",
            signal_name, non_zero_percent
        );
    }

    println!("✓ Test 2 passed: All signal types record successfully\n");
}

// ============================================================================
// Test 3: Channel Mapping (Both Input and Output)
// ============================================================================

#[tokio::test]
async fn test_channel_mapping() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }

    let config = get_test_config();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== Test 3: Channel Mapping ===");
    println!(
        "Output mapping: send to hardware channel {}",
        config.send_channel
    );
    println!(
        "Input mapping: record from hardware channel {}",
        config.record_channel
    );

    // Generate test signal
    let tone = gen_tone(1000.0, 0.5, config.sample_rate, 2.0);
    let playback_file = output_dir.join("test_mapping_playback.wav");
    let record_file = output_dir.join("test_mapping_record.wav");

    write_wav_file(&playback_file, &tone, config.sample_rate);

    // Test recording with explicit input channel mapping
    let rec_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    let rec_result = rec_manager
        .start_recording(
            None,
            record_file.clone(),
            config.sample_rate,
            1,
            Some(vec![config.record_channel]), // Explicit input mapping
        )
        .await;

    assert!(
        rec_result.is_ok(),
        "Recording with input mapping failed: {:?}",
        rec_result.err()
    );
    println!(
        "✓ Recording started with input channel map: [{}]",
        config.record_channel
    );

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test playback with explicit output channel mapping
    let play_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    let play_result = play_manager
        .start_playback(
            playback_file.clone(),
            None,
            config.sample_rate,
            1,
            Vec::new(),
            ChannelMapMode::Normal,
            Some(vec![config.send_channel]), // Explicit output mapping
            None,
        )
        .await;

    assert!(
        play_result.is_ok(),
        "Playback with output mapping failed: {:?}",
        play_result.err()
    );
    println!(
        "✓ Playback started with output channel map: [{}]",
        config.send_channel
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

    play_manager.stop_playback().await.unwrap();
    rec_manager.stop_recording().await.unwrap();

    // Verify recording succeeded with correct channel
    assert!(
        record_file.exists(),
        "Recording with channel mapping failed to create file"
    );

    let reader = hound::WavReader::open(&record_file).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "Expected 1 channel in recording");

    println!("✓ Test 3 passed: Channel mapping works correctly\n");
}

// ============================================================================
// Test 4: Loopback Accuracy - Send and Read Back
// ============================================================================

#[tokio::test]
async fn test_loopback_accuracy() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }

    let config = get_test_config();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== Test 4: Loopback Accuracy ===");
    println!("This test verifies that a signal sent through loopback is read back accurately");

    // Generate a 5-second log sweep
    let reference = gen_log_sweep(5.0, 22000.0, 0.5, config.sample_rate, 5.0);
    let playback_file = output_dir.join("test_loopback_send.wav");
    let record_file = output_dir.join("test_loopback_receive.wav");
    let csv_file = output_dir.join("test_loopback_analysis.csv");

    write_wav_file(&playback_file, &reference, config.sample_rate);

    // Start recording
    let rec_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    rec_manager
        .start_recording(
            None,
            record_file.clone(),
            config.sample_rate,
            1,
            Some(vec![config.record_channel]),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Start playback
    let play_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    play_manager
        .start_playback(
            playback_file.clone(),
            None,
            config.sample_rate,
            1,
            Vec::new(),
            ChannelMapMode::Normal,
            Some(vec![config.send_channel]),
            None,
        )
        .await
        .unwrap();

    println!("Playing and recording 5-second sweep...");
    tokio::time::sleep(Duration::from_secs(7)).await;

    play_manager.stop_playback().await.unwrap();
    rec_manager.stop_recording().await.unwrap();

    // Analyze the loopback recording
    println!("Analyzing loopback recording...");
    let result = analyze_recording(&record_file, &reference, config.sample_rate)
        .expect("Failed to analyze recording");

    write_analysis_csv(&result, &csv_file).unwrap();

    println!("Analysis complete:");
    println!(
        "  Estimated lag: {} samples ({:.2} ms)",
        result.estimated_lag_samples,
        result.estimated_lag_samples as f32 * 1000.0 / config.sample_rate as f32
    );

    // Verify CSV format
    let csv_lines: Vec<String> = std::fs::read_to_string(&csv_file)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(
        csv_lines.len(),
        201,
        "CSV should have 201 lines (1 header + 200 data)"
    );
    assert_eq!(
        csv_lines[0], "frequency_hz,spl_db,phase_deg",
        "CSV header mismatch"
    );
    println!("  CSV format: {} lines", csv_lines.len());

    // Calculate SPL statistics in the 100Hz-10kHz range
    let mut spl_100_10k = Vec::new();
    for i in 0..result.frequencies.len() {
        let freq = result.frequencies[i];
        if freq >= 100.0 && freq <= 10000.0 {
            spl_100_10k.push(result.spl_db[i]);
        }
    }

    let mean_spl = spl_100_10k.iter().sum::<f32>() / spl_100_10k.len() as f32;
    let min_spl = spl_100_10k.iter().copied().fold(f32::INFINITY, f32::min);
    let max_spl = spl_100_10k
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let variation = max_spl - min_spl;

    println!("  SPL (100Hz-10kHz):");
    println!("    Mean:  {:.3} dB", mean_spl);
    println!("    Min:   {:.3} dB", min_spl);
    println!("    Max:   {:.3} dB", max_spl);
    println!("    Range: {:.3} dB", variation);

    // Assertions for loopback accuracy
    assert!(
        mean_spl.abs() < 0.5,
        "Mean SPL ({:.3} dB) deviates too much from 0 dB for loopback. Expected ~0 dB.",
        mean_spl
    );
    println!("  ✓ Mean SPL is close to 0 dB (within 0.5 dB)");

    assert!(
        variation < 2.0,
        "SPL variation ({:.3} dB) is too high. Expected < 2 dB for clean loopback.",
        variation
    );
    println!("  ✓ SPL variation is low (< 2 dB)");

    // Verify phase wrapping
    for (i, &phase) in result.phase_deg.iter().enumerate() {
        assert!(
            phase >= -180.0 && phase <= 180.0,
            "Phase at {:.1} Hz ({:.1}°) is outside [-180, 180] range",
            result.frequencies[i],
            phase
        );
    }
    println!("  ✓ Phase values properly wrapped to [-180, 180]°");

    // Additional test: Verify the recording is NOT identical to reference
    // (regression test for file copy bug)
    let mut reader = hound::WavReader::open(&record_file).unwrap();
    let recorded_samples: Vec<i32> = reader.samples().collect::<Result<Vec<_>, _>>().unwrap();
    let recorded_f32: Vec<f32> = recorded_samples
        .iter()
        .map(|&s| s as f32 / i32::MAX as f32)
        .collect();

    let compare_len = reference
        .len()
        .min(recorded_f32.len())
        .min(config.sample_rate as usize * 3);
    let mut identical_count = 0;
    for i in 0..compare_len {
        if (reference[i] - recorded_f32[i]).abs() < 1e-6 {
            identical_count += 1;
        }
    }

    let identical_percent = (identical_count as f32 / compare_len as f32) * 100.0;
    assert!(
        identical_percent < 50.0,
        "Recording is suspiciously similar to reference ({:.1}% identical). \
        This may indicate a file copy bug instead of actual recording.",
        identical_percent
    );
    println!(
        "  ✓ Recording differs from reference ({:.1}% identical samples)",
        identical_percent
    );

    println!("\n✓ Test 4 passed: Loopback sends and reads back signal accurately\n");
}

// ============================================================================
// Additional Regression Tests
// ============================================================================

#[tokio::test]
async fn test_no_duplicate_wav_headers() {
    if !should_run_e2e_tests() {
        eprintln!("Skipping test (AEQ_E2E!=1)");
        return;
    }

    let config = get_test_config();
    let output_dir = test_output_dir();
    std::fs::create_dir_all(&output_dir).unwrap();

    println!("\n=== Regression Test: No Duplicate WAV Headers ===");

    let tone = gen_tone(1000.0, 0.5, config.sample_rate, 1.0);
    let playback_file = output_dir.join("test_no_dup_playback.wav");
    let record_file = output_dir.join("test_no_dup_record.wav");

    write_wav_file(&playback_file, &tone, config.sample_rate);

    let rec_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    rec_manager
        .start_recording(
            None,
            record_file.clone(),
            config.sample_rate,
            1,
            Some(vec![config.record_channel]),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let play_manager = AudioManager::new(find_camilladsp_binary().unwrap());
    play_manager
        .start_playback(
            playback_file.clone(),
            None,
            config.sample_rate,
            1,
            Vec::new(),
            ChannelMapMode::Normal,
            Some(vec![config.send_channel]),
            None,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    play_manager.stop_playback().await.unwrap();
    rec_manager.stop_recording().await.unwrap();

    // Check for duplicate RIFF headers
    let file_bytes = std::fs::read(&record_file).unwrap();
    assert_eq!(&file_bytes[0..4], b"RIFF", "Missing RIFF header");

    // Count RIFF headers
    let mut riff_count = 0;
    for i in 0..file_bytes.len().saturating_sub(4) {
        if &file_bytes[i..i + 4] == b"RIFF" {
            riff_count += 1;
            if riff_count > 1 {
                panic!("Found duplicate RIFF header at offset 0x{:X}", i);
            }
        }
    }

    println!("✓ No duplicate WAV headers found\n");
}
