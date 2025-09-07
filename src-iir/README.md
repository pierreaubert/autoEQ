# AutoEQ IIR Filters

This crate provides IIR (Infinite Impulse Response) filter implementations for audio equalization.

## Features

- **Biquad Filters**: Implementation of common biquad filter types
  - Low-pass filters
  - High-pass filters  
  - Peak/notch filters
  - Low/high shelf filters
  - Band-pass filters
- **PEQ (Parametric Equalizer)**: Multi-band parametric equalization with advanced features
  - SPL response computation
  - Preamp gain calculation
  - EqualizerAPO format export
  - PEQ comparison and manipulation
- **Filter Design**: Specialized filter design algorithms
  - Butterworth filters (lowpass/highpass)
  - Linkwitz-Riley filters (lowpass/highpass)
- **Response Computation**: Calculate frequency and phase response
- **Filter Conversion**: Convert between different filter representations

## Filter Types

### Biquad Filter Types
- `BiquadFilterType::Lowpass`: Low-pass filter
- `BiquadFilterType::Highpass`: High-pass filter
- `BiquadFilterType::HighpassVariableQ`: High-pass filter with variable Q
- `BiquadFilterType::Bandpass`: Band-pass filter
- `BiquadFilterType::Peak`: Peak/parametric filter
- `BiquadFilterType::Notch`: Notch filter
- `BiquadFilterType::Lowshelf`: Low-shelf filter
- `BiquadFilterType::Highshelf`: High-shelf filter

## Usage Examples

### Basic Biquad Filter

```rust
use autoeq_iir::{Biquad, BiquadFilterType};

// Create a peak filter at 1kHz with Q=1.0 and 3dB gain
let filter = Biquad::new(
    BiquadFilterType::Peak,
    1000.0, // frequency
    48000.0, // sample rate
    1.0,     // Q factor
    3.0      // gain in dB
);

// Apply filter to audio samples (requires mut for state updates)
// let mut filter = Biquad::new(...); // <- use mut if processing samples
// let output = filter.process(input_sample);

// Calculate frequency response at 1kHz
let response_db = filter.log_result(1000.0);
print!("Response at 1kHz: {:.2} dB", response_db);
```

### Parametric EQ (PEQ)

```rust
use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_spl, peq_preamp_gain, peq_format_apo};
use ndarray::Array1;

// Create a multi-band EQ
let mut peq: Peq = Vec::new();

// Add a high-pass filter at 80Hz
let hp = Biquad::new(BiquadFilterType::Highpass, 80.0, 48000.0, 0.707, 0.0);
peq.push((1.0, hp));

// Add a peak filter to boost mids at 1kHz
let peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.5, 4.0);
peq.push((1.0, peak));

// Add a high-shelf to roll off highs
let hs = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.8, -2.0);
peq.push((1.0, hs));

// Calculate frequency response
let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 1000);
let response = peq_spl(&freqs, &peq);

// Calculate preamp gain to prevent clipping
let preamp = peq_preamp_gain(&peq);
print!("Recommended preamp: {:.1} dB", preamp);

// Export to EqualizerAPO format
let apo_config = peq_format_apo("My Custom EQ", &peq);
print!("{}", apo_config);
```

### Filter Design

```rust
use autoeq_iir::{peq_butterworth_lowpass, peq_linkwitzriley_highpass};

// Create a 4th-order Butterworth lowpass at 2kHz
let lp_filter = peq_butterworth_lowpass(4, 2000.0, 48000.0);
print!("Butterworth LP has {} sections", lp_filter.len());

// Create a 4th-order Linkwitz-Riley highpass at 2kHz  
let hp_filter = peq_linkwitzriley_highpass(4, 2000.0, 48000.0);
print!("LR HP has {} sections", hp_filter.len());

// These can be used for crossover design
```

## PEQ Functions Reference

### Core PEQ Operations
- `peq_spl(freq, peq)`: Calculate SPL response across frequencies
- `peq_equal(left, right)`: Compare two PEQs for equality
- `peq_preamp_gain(peq)`: Calculate recommended preamp gain
- `peq_preamp_gain_max(peq)`: Calculate conservative preamp gain with safety margin
- `peq_format_apo(comment, peq)`: Export PEQ to EqualizerAPO format

### Filter Design Functions
- `peq_butterworth_q(order)`: Calculate Q values for Butterworth filters
- `peq_butterworth_lowpass(order, freq, srate)`: Create Butterworth lowpass filter
- `peq_butterworth_highpass(order, freq, srate)`: Create Butterworth highpass filter
- `peq_linkwitzriley_q(order)`: Calculate Q values for Linkwitz-Riley filters
- `peq_linkwitzriley_lowpass(order, freq, srate)`: Create Linkwitz-Riley lowpass filter
- `peq_linkwitzriley_highpass(order, freq, srate)`: Create Linkwitz-Riley highpass filter

### Utility Functions
- `bw2q(bw)`: Convert bandwidth in octaves to Q factor
- `q2bw(q)`: Convert Q factor to bandwidth in octaves

## Advanced Example: Building a Complete Audio Processor

```rust
use autoeq_iir::*;
use ndarray::Array1;

fn create_studio_eq() -> Peq {
    let mut peq = Vec::new();
    
    // High-pass filter to remove subsonic content
    let hp = peq_butterworth_highpass(2, 20.0, 48000.0);
    peq.extend(hp);
    
    // Presence boost
    let presence = Biquad::new(BiquadFilterType::Peak, 3000.0, 48000.0, 1.2, 2.5);
    peq.push((1.0, presence));
    
    // Air band enhancement
    let air = Biquad::new(BiquadFilterType::Highshelf, 10000.0, 48000.0, 0.9, 1.5);
    peq.push((1.0, air));
    
    peq
}

fn analyze_eq(peq: &Peq) {
    // Generate frequency sweep
    let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 200);
    
    // Calculate response
    let response = peq_spl(&freqs, peq);
    
    // Find peak response
    let max_gain = response.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min_gain = response.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    
    println!("EQ Analysis:");
    println!("  Peak gain: {:.2} dB", max_gain);
    println!("  Min gain: {:.2} dB", min_gain);
    println!("  Dynamic range: {:.2} dB", max_gain - min_gain);
    println!("  Recommended preamp: {:.2} dB", peq_preamp_gain(peq));
}

fn main() {
    let studio_eq = create_studio_eq();
    analyze_eq(&studio_eq);
    
    // Export for use in EqualizerAPO
    let config = peq_format_apo("Studio EQ v1.0", &studio_eq);
    println!("\nEqualizerAPO Configuration:");
    println!("{}", config);
}
```

## Key Concepts

### PEQ Type
The `Peq` type is defined as `Vec<(f64, Biquad)>` where:
- The `f64` is the weight/amplitude multiplier for each filter
- The `Biquad` is the individual filter definition
- This allows for flexible filter chaining and weighting

### Filter Order vs. Sections
- **Butterworth filters**: An Nth-order filter uses N/2 biquad sections (rounded up)
- **Linkwitz-Riley filters**: Special case of Butterworth designed for crossovers
- Higher orders provide steeper rolloff but more computational cost

### Q Factor Guidelines
- **Q < 0.5**: Wide, gentle curves
- **Q = 0.707**: Butterworth response (maximally flat)
- **Q = 1.0**: Good compromise for most applications
- **Q > 5**: Very narrow, surgical corrections
- **Q > 10**: Extreme precision, potential ringing

## Integration

This crate is part of the AutoEQ ecosystem and is designed to work with:
- `autoeq-de`: Differential Evolution optimizer
- `autoeq-cea2034`: CEA2034 scoring algorithms  
- `autoeq`: Main AutoEQ application

The PEQ functions are particularly useful for:
- Automatic speaker/headphone equalization
- Room correction systems
- Audio mastering and mixing
- Crossover network design
- Research and analysis

## License

GPL-3.0-or-later
