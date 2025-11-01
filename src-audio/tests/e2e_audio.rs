// E2E audio loopback tests (local-only). Requires a loopback device and CamillaDSP.
// Gated by AEQ_E2E=1 and environment-provided devices/mappings.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sotf_backend::camilla::{ChannelMapMode, find_camilladsp_binary};
use sotf_backend::{AudioManager, FilterParams, create_decoder};

fn parse_list_u16(var: &str) -> Option<Vec<u16>> {
    env::var(var).ok().and_then(|s| {
        if s.trim().is_empty() {
            return None;
        }
        let mut v = Vec::new();
        for tok in s.split(',') {
            if let Ok(x) = tok.trim().parse::<u16>() {
                v.push(x);
            } else {
                return None;
            }
        }
        Some(v)
    })
}

fn python_exe() -> String {
    if cfg!(windows) {
        "python".to_string()
    } else {
        "python3".to_string()
    }
}

fn script_path() -> PathBuf {
    // Crate root -> ../scripts/generate_audio_tests.py
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("scripts")
        .join("generate_audio_tests.py")
}

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-audio")
}

fn generated_id_path(ch: u16, sr: u32, bits: u16) -> PathBuf {
    out_dir()
        .join("wav")
        .join("id")
        .join(format!("id_ch{}_sr{}_b{}.wav", ch, sr, bits))
}

fn generated_thd1k_path(ch: u16, sr: u32, bits: u16) -> PathBuf {
    out_dir()
        .join("wav")
        .join("thd1k")
        .join(format!("thd1k_ch{}_sr{}_b{}.wav", ch, sr, bits))
}

fn goertzel_mag(signal: &[f32], sr: u32, freq: f32) -> f32 {
    // Single-bin Goertzel
    let n = signal.len();
    if n == 0 {
        return 0.0;
    }
    let k = ((n as f32 * freq) / sr as f32).round();
    let w = 2.0 * std::f32::consts::PI * k / n as f32;
    let cosine = w.cos();
    let coeff = 2.0 * cosine;
    let mut s_prev = 0.0f32;
    let mut s_prev2 = 0.0f32;
    for &x in signal {
        let s = x + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    let real = s_prev - s_prev2 * cosine;
    let imag = s_prev2 * w.sin();
    (real * real + imag * imag).sqrt() / (n as f32 / 2.0).max(1.0)
}

fn thd_ratio(signal: &[f32], sr: u32, f0: f32, max_h: usize) -> f32 {
    let a1 = goertzel_mag(signal, sr, f0);
    if a1 <= 1e-9 {
        return 0.0;
    }
    let mut dist2 = 0.0f32;
    for h in 2..=max_h {
        let fh = f0 * h as f32;
        if fh >= (sr as f32 / 2.0) - 1.0 {
            break;
        }
        let ah = goertzel_mag(signal, sr, fh);
        dist2 += ah * ah;
    }
    dist2.sqrt() / a1
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_loopback_id_and_thd() {
    if env::var("AEQ_E2E").ok().as_deref() != Some("1") {
        eprintln!("AEQ_E2E!=1, skipping e2e tests");
        return;
    }

    let out_dev = env::var("AEQ_E2E_OUT_DEVICE").ok();
    let in_dev = env::var("AEQ_E2E_IN_DEVICE").ok();
    let out_map = parse_list_u16("AEQ_E2E_OUT_MAP");
    let in_map = parse_list_u16("AEQ_E2E_IN_MAP");

    let sr: u32 = env::var("AEQ_E2E_SR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(44100);
    let ch: u16 = env::var("AEQ_E2E_CH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let bits: u16 = env::var("AEQ_E2E_BITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16);
    let duration: u64 = env::var("AEQ_E2E_DUR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    // Generate files with Python script
    // let script = script_path();
    // assert!(
    //     script.exists(),
    //     "generator script not found at {:?}",
    //     script
    // );
    // let status = Command::new(python_exe())
    //     .arg(&script)
    //     .arg("--out-dir")
    //     .arg(out_dir())
    //     .arg("--formats")
    //     .arg("wav")
    //     .arg("--channels")
    //     .arg(ch.to_string())
    //     .arg("--sample-rates")
    //     .arg(sr.to_string())
    //     .arg("--bits")
    //     .arg(bits.to_string())
    //     .arg("--signals")
    //     .arg("id")
    //     .arg("thd1k")
    //     .arg("--duration")
    //     .arg(duration.to_string())
    //     .status()
    //     .expect("failed to run python");
    // assert!(status.success(), "generator failed");

    let id_file = generated_id_path(ch, sr, bits);
    let thd_file = generated_thd1k_path(ch, sr, bits);
    assert!(id_file.exists());
    assert!(thd_file.exists());

    // Find camilladsp
    let cam_bin = match find_camilladsp_binary() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    // 1) Record loopback while playing ID signal
    let capture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e_capture_id.wav");

    // Start recorder
    let rec_mgr = AudioManager::new(cam_bin.clone());
    rec_mgr
        .start_recording(capture_path.clone(), in_dev.clone(), sr, ch, in_map.clone())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Start playback
    let play_mgr = AudioManager::new(cam_bin.clone());
    play_mgr
        .start_playback(
            id_file.clone(),
            out_dev.clone(),
            sr,
            ch,
            Vec::<FilterParams>::new(),
            ChannelMapMode::Normal,
            out_map.clone(),
            None,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(duration + 1)).await;

    // Stop both
    play_mgr.stop_playback().await.unwrap();
    rec_mgr.stop_recording().await.unwrap();

    assert!(capture_path.exists());

    // Decode original and captured
    let mut dec_ref = create_decoder(&id_file).unwrap();
    let spec_ref = dec_ref.spec().clone();
    let mut ref_samples: Vec<f32> = Vec::new();
    while let Ok(Some(pkt)) = dec_ref.decode_next() {
        ref_samples.extend_from_slice(&pkt.samples);
    }

    let mut dec_cap = create_decoder(&capture_path).unwrap();
    let spec_cap = dec_cap.spec().clone();
    assert_eq!(
        spec_cap.channels, ch,
        "captured channels do not match request"
    );

    let mut cap_samples: Vec<f32> = Vec::new();
    while let Ok(Some(pkt)) = dec_cap.decode_next() {
        cap_samples.extend_from_slice(&pkt.samples);
    }

    // De-interleave and compare dominant tones per channel using Goertzel (latency independent)
    let n_ref_frames = ref_samples.len() / (spec_ref.channels as usize);
    let n_cap_frames = cap_samples.len() / (spec_cap.channels as usize);
    let min_frames = n_ref_frames
        .min(n_cap_frames)
        .min((sr as usize) * duration as usize);

    // Frequencies used in generator
    let freqs: Vec<f32> = (0..ch as usize)
        .map(|i| (300.0 + 300.0 * i as f32).min(6000.0) as f32)
        .collect();

    let mut pass_channels = 0;
    for c in 0..ch as usize {
        // slice per channel
        let ref_ch: Vec<f32> = (0..min_frames)
            .map(|i| ref_samples[i * ch as usize + c])
            .collect();
        let cap_ch: Vec<f32> = (0..min_frames)
            .map(|i| cap_samples[i * ch as usize + c])
            .collect();
        let f0 = freqs[c];
        let a_ref = goertzel_mag(&ref_ch, spec_ref.sample_rate, f0);
        let a_cap = goertzel_mag(&cap_ch, spec_cap.sample_rate, f0);
        let db_ref = 20.0 * (a_ref.max(1e-12)).log10();
        let db_cap = 20.0 * (a_cap.max(1e-12)).log10();
        let delta = (db_cap - db_ref).abs();
        // Allow 1 dB deviation
        if delta <= 1.0 {
            pass_channels += 1;
        }
    }
    assert_eq!(
        pass_channels, ch as usize,
        "level mismatch on some channels (ID signal)"
    );

    // 2) THD check with 1 kHz
    let capture_path_thd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("e2e_capture_thd.wav");

    let rec2 = AudioManager::new(cam_bin.clone());
    rec2.start_recording(
        capture_path_thd.clone(),
        in_dev.clone(),
        sr,
        ch,
        in_map.clone(),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let play2 = AudioManager::new(cam_bin.clone());
    play2
        .start_playback(
            thd_file.clone(),
            out_dev.clone(),
            sr,
            ch,
            Vec::<FilterParams>::new(),
            ChannelMapMode::Normal,
            out_map.clone(),
            None,
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(duration + 1)).await;
    play2.stop_playback().await.unwrap();
    rec2.stop_recording().await.unwrap();
    assert!(capture_path_thd.exists());

    // Analyze THD on first channel only to keep runtime reasonable
    let mut dec_t = create_decoder(&capture_path_thd).unwrap();
    let spec_t = dec_t.spec().clone();
    let mut sam_t: Vec<f32> = Vec::new();
    while let Ok(Some(pkt)) = dec_t.decode_next() {
        sam_t.extend_from_slice(&pkt.samples);
    }
    let frames_t = sam_t.len() / (spec_t.channels as usize);
    let cap_ch0: Vec<f32> = (0..frames_t)
        .map(|i| sam_t[i * spec_t.channels as usize + 0])
        .collect();

    let thd = thd_ratio(&cap_ch0, spec_t.sample_rate, 1000.0, 10);
    let thd_max: f32 = env::var("AEQ_E2E_THD_MAX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.02);
    assert!(thd <= thd_max, "THD too high: {:.4} > {:.4}", thd, thd_max);
}
