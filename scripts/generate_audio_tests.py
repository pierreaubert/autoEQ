#!/usr/bin/env python3
"""
Generate audio test files for end-to-end audio validation.

Generates WAV and/or FLAC in multiple channel counts, sample rates and bit depths.
Signals:
- id: per-channel identification tones (unique frequency per channel)
- thd1k: single-tone 1 kHz @ -3 dBFS (for THD)
- thd100: single-tone 100 Hz @ -3 dBFS (low-frequency THD)
- imd_smpte: SMPTE two-tone 60 Hz + 7 kHz (4:1 amplitude ratio)
- imd_ccif: CCIF two-tone 19 kHz + 20 kHz (equal amplitudes)

Examples:
  python3 scripts/generate_audio_tests.py --out-dir target/test-audio \
    --formats wav flac --channels 1 2 6 16 --sample-rates 44100 48000 96000 192000 --bits 16 24 \
    --signals id thd1k imd_ccif --duration 3

Notes:
- FLAC writing prefers the Python soundfile module; if unavailable, falls back to the `flac` CLI.
- WAV writer uses the Python stdlib `wave` module and supports 16/24-bit PCM.
- Cross-platform (macOS/Linux/Windows) if Python and dependencies are available.
"""

from __future__ import annotations
import argparse
import math
import os
import sys
import json
import shutil
from pathlib import Path
from typing import List, Tuple, Optional

import numpy as np

try:
    import soundfile as sf  # optional, used for FLAC
    HAS_SF = True
except Exception:
    HAS_SF = False


def db_to_linear(db: float) -> float:
    return 10.0 ** (db / 20.0)


def tone(freq: float, sr: int, dur: float, phase: float = 0.0, amp: float = 0.707) -> np.ndarray:
    t = np.arange(int(sr * dur), dtype=np.float64) / sr
    return (amp * np.sin(2.0 * np.pi * freq * t + phase)).astype(np.float32)


def two_tone(freq1: float, freq2: float, sr: int, dur: float, amp1: float, amp2: float) -> np.ndarray:
    x1 = tone(freq1, sr, dur, amp=amp1)
    x2 = tone(freq2, sr, dur, amp=amp2)
    x = x1 + x2
    peak = np.max(np.abs(x))
    if peak > 0.999:
        x = (x / peak) * 0.999  # keep headroom
    return x.astype(np.float32)


def per_channel_id_signal(channels: int, sr: int, dur: float) -> np.ndarray:
    """
    Build a multi-channel signal where each channel is a unique single-tone.
    Frequencies are spaced to avoid overlap below 6 kHz and within Nyquist at 44.1 kHz.
    """
    # base freq 300 Hz, step 300 Hz => 300, 600, 900, ...
    # cap within 6 kHz
    freqs = [min(300.0 + 300.0 * i, 6000.0) for i in range(channels)]
    data = [tone(freqs[i], sr, dur, amp=0.707) for i in range(channels)]
    return np.stack(data, axis=1), freqs


def thd_signal(freq: float, channels: int, sr: int, dur: float) -> np.ndarray:
    sig = tone(freq, sr, dur, amp=0.707)
    data = [sig.copy() for _ in range(channels)]
    return np.stack(data, axis=1)


def imd_smpte_signal(channels: int, sr: int, dur: float) -> np.ndarray:
    # SMPTE: 60 Hz at higher amplitude, 7 kHz at lower amplitude (typical 4:1 ratio)
    low = tone(60.0, sr, dur, amp=0.8)  # ~ -1.9 dBFS
    high = tone(7000.0, sr, dur, amp=0.2)  # lower level to avoid clipping
    sig = low + high
    peak = np.max(np.abs(sig))
    if peak > 0.999:
        sig = (sig / peak) * 0.999
    data = [sig.astype(np.float32) for _ in range(channels)]
    return np.stack(data, axis=1)


def imd_ccif_signal(channels: int, sr: int, dur: float) -> np.ndarray:
    # CCIF: equal amplitudes at 19 kHz and 20 kHz
    sig = two_tone(19000.0, 20000.0, sr, dur, amp1=0.5, amp2=0.5)
    data = [sig for _ in range(channels)]
    return np.stack(data, axis=1)


def float_to_int_pcm(x: np.ndarray, bits: int) -> bytes:
    assert x.dtype == np.float32
    # clip
    x = np.clip(x, -0.999999, 0.999999)
    if bits == 16:
        y = (x * 32767.0).round().astype('<i2')
        return y.tobytes()
    elif bits == 24:
        # 24-bit little-endian in 3 bytes
        y = (x * 8388607.0).round().astype(np.int32)
        # pack 3 LSB bytes per sample
        b = bytearray()
        for v in y:
            b.append(v & 0xFF)
            b.append((v >> 8) & 0xFF)
            b.append((v >> 16) & 0xFF)
        return bytes(b)
    else:
        raise ValueError(f"Unsupported PCM bit depth: {bits}")


def write_wav(path: Path, data: np.ndarray, sr: int, bits: int) -> None:
    import wave
    n_channels = data.shape[1]
    n_frames = data.shape[0]

    # interleave
    interleaved = data.reshape(n_frames, n_channels).astype(np.float32)
    interleaved = interleaved  # already interleaved by axis=1 in stacking
    # convert to PCM
    pcm_bytes = b''.join(float_to_int_pcm(interleaved[:, ch], bits) for ch in range(n_channels))
    # But above concatenates channel-by-channel; need frame interleaving
    # Rebuild with per-frame interleave to ensure proper ordering
    # Build per-frame bytes
    bytes_per_sample = 2 if bits == 16 else 3
    frame_bytes = bytearray()
    for i in range(n_frames):
        for ch in range(n_channels):
            sample = float_to_int_pcm(interleaved[i:i+1, ch], bits)
            frame_bytes.extend(sample)

    with wave.open(str(path), 'wb') as wf:
        wf.setnchannels(n_channels)
        wf.setsampwidth(2 if bits == 16 else 3)
        wf.setframerate(sr)
        wf.writeframes(frame_bytes)


def write_flac(path: Path, data: np.ndarray, sr: int, bits: int) -> None:
    global HAS_SF
    if HAS_SF:
        subtype = 'PCM_24' if bits == 24 else 'PCM_16'
        sf.write(str(path), data, sr, subtype=subtype, format='FLAC')
        return
    # fallback to `flac` CLI using a temp WAV
    if shutil.which('flac') is None:
        raise RuntimeError("FLAC output requested but neither soundfile module nor 'flac' CLI is available")
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        tmp_wav = Path(td) / 'tmp.wav'
        write_wav(tmp_wav, data, sr, bits)
        # -f overwrite, -s silent
        cmd = ['flac', '-f', '-s', str(tmp_wav), '-o', str(path)]
        rc = os.spawnvp(os.P_WAIT, cmd[0], cmd)
        if rc != 0:
            raise RuntimeError(f"flac encoding failed with code {rc}")


def ensure_dir(p: Path) -> None:
    p.mkdir(parents=True, exist_ok=True)


def build_filename(signal: str, channels: int, sr: int, bits: int, ext: str) -> str:
    return f"{signal}_ch{channels}_sr{sr}_b{bits}.{ext}"


def generate_one(out_dir: Path, fmt: str, signal: str, channels: int, sr: int, bits: int, dur: float) -> Path:
    if channels < 1 or channels > 16:
        raise ValueError("channels must be between 1 and 16")
    if bits not in (16, 24):
        raise ValueError("bits must be 16 or 24")

    # Make signal
    if signal == 'id':
        data, freqs = per_channel_id_signal(channels, sr, dur)
        manifest = {"type": "id", "freqs": freqs}
    elif signal == 'thd1k':
        data = thd_signal(1000.0, channels, sr, dur)
        manifest = {"type": "thd1k", "freq": 1000.0}
    elif signal == 'thd100':
        data = thd_signal(100.0, channels, sr, dur)
        manifest = {"type": "thd100", "freq": 100.0}
    elif signal == 'imd_smpte':
        data = imd_smpte_signal(channels, sr, dur)
        manifest = {"type": "imd_smpte", "freqs": [60.0, 7000.0], "ratio": "~4:1"}
    elif signal == 'imd_ccif':
        data = imd_ccif_signal(channels, sr, dur)
        manifest = {"type": "imd_ccif", "freqs": [19000.0, 20000.0]}
    else:
        raise ValueError(f"unknown signal: {signal}")

    # Make directories by format/signal
    subdir = out_dir / fmt / signal
    ensure_dir(subdir)
    fname = build_filename(signal, channels, sr, bits, fmt)
    out_path = subdir / fname

    # Write
    if fmt == 'wav':
        write_wav(out_path, data, sr, bits)
    elif fmt == 'flac':
        write_flac(out_path, data, sr, bits)
    else:
        raise ValueError("format must be wav or flac")

    # Write sidecar JSON with metadata
    sidecar = out_path.with_suffix(out_path.suffix + '.json')
    with open(sidecar, 'w') as f:
        json.dump({
            "format": fmt,
            "channels": channels,
            "sample_rate": sr,
            "bits": bits,
            "duration": dur,
            "signal": manifest,
        }, f, indent=2)

    return out_path


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Generate audio test files")
    p.add_argument('--out-dir', type=Path, default=Path('target/test-audio'), help='Output directory')
    p.add_argument('--formats', nargs='+', default=['wav', 'flac'], choices=['wav', 'flac'])
    p.add_argument('--channels', nargs='+', type=int, default=[2, 6, 16])
    p.add_argument('--sample-rates', nargs='+', type=int, default=[44100, 96000, 192000])
    p.add_argument('--bits', nargs='+', type=int, default=[16, 24])
    p.add_argument('--signals', nargs='+', default=['id', 'thd1k', 'thd100', 'imd_smpte', 'imd_ccif'],
                   choices=['id', 'thd1k', 'thd100', 'imd_smpte', 'imd_ccif'])
    p.add_argument('--duration', type=float, default=3.0, help='Duration in seconds')
    return p.parse_args()


def main() -> int:
    args = parse_args()
    ensure_dir(args.out_dir)

    generated = []
    for fmt in args.formats:
        for ch in args.channels:
            for sr in args.sample_rates:
                nyq = sr / 2.0
                for bits in args.bits:
                    for sig in args.signals:
                        # avoid CCIF at too-low sample rates if tones are too close to Nyquist
                        if sig == 'imd_ccif' and nyq < 20500.0:
                            # Still acceptable at 44.1k (nyq 22050), so keep it; skip only if nyq < 20500
                            pass
                        try:
                            path = generate_one(args.out_dir, fmt, sig, ch, sr, bits, args.duration)
                            generated.append(str(path))
                        except Exception as e:
                            print(f"[WARN] failed to generate {fmt} {sig} ch{ch} sr{sr} b{bits}: {e}", file=sys.stderr)
                            continue

    manifest_path = args.out_dir / 'manifest.json'
    with open(manifest_path, 'w') as f:
        json.dump({"files": generated}, f, indent=2)
    print(f"Generated {len(generated)} files. Manifest: {manifest_path}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
