# AutoEQ CEA2034 Scoring

This crate implements CEA2034-based preference scoring algorithms for loudspeaker measurements, based on research by Harman, Olive, and others.

## Features

- **CEA2034 Metrics Computation**: Calculate standard metrics from spinorama measurements
- **Preference Score Calculation**: Harman/Olive preference rating algorithm
- **Curve Analysis**: Slope, smoothness, and spectral analysis tools
- **PIR Computation**: Predicted In-Room response calculation from CEA2034 data
- **Octave Band Processing**: Frequency-weighted analysis and filtering

## CEA2034 Standard

The CEA2034 standard defines a set of measurements for evaluating loudspeaker performance:

- **On-axis (0°)**: Direct response
- **Listening Window**: Average of on-axis and early reflections (±10°, ±15°, ±20°, ±25°, ±30°)
- **Early Reflections**: Floor, ceiling, front/rear wall, and side wall reflections
- **Sound Power**: Total acoustic power output
- **Directivity Index (DI)**: Ratio of on-axis to sound power

## Preference Score Algorithm

The preference score is based on research showing correlation between measured responses and listener preference:

```rust
use autoeq_cea2034::{compute_cea2034_metrics, Curve};

let metrics = compute_cea2034_metrics(
    &frequencies,
    &spinorama_data,
    Some(&equalized_response)
).await?;

println!("Preference Score: {:.2}", metrics.pref_score);
println!("LW Score: {:.2}", metrics.lw_score);
println!("PIR Score: {:.2}", metrics.pir_score);
```

## Scoring Components

### Listening Window (LW) Score
- Flatness and smoothness of the listening window response
- Deviation from target curve
- Frequency-weighted penalties

### Predicted In-Room (PIR) Score  
- Computed from listening window, early reflections, and sound power
- Represents typical in-room response
- Critical for overall preference rating

### Bass Extension
- Low-frequency response evaluation
- Extension and smoothness below 100 Hz

### Directivity Analysis
- Consistency of off-axis response
- Smoothness of directivity index
- Early reflection characteristics

## Usage

```rust
use autoeq_cea2034::{score, octave_intervals, compute_pir_from_lw_er_sp};
use ndarray::Array1;
use std::collections::HashMap;

// Compute preference score for a frequency response
let frequencies = Array1::from(vec![20.0, 25.0, 31.5, /* ... */ 20000.0]);
let response = Array1::from(vec![-2.1, -1.8, -1.2, /* ... */ -10.5]);

let preference_score = score(
    &frequencies,
    &response,
    100.0,    // reference frequency  
    10000.0,  // upper frequency limit
    Some(&target_curve)
)?;

// Compute PIR from CEA2034 measurements
let pir = compute_pir_from_lw_er_sp(&lw_curve, &er_curve, &sp_curve);
```

## Integration

This crate is part of the AutoEQ ecosystem:
- Used by `autoeq` for optimization target scoring
- Provides objective functions for `autoeq-de` optimization  
- Integrates with measurement data from Spinorama.org API

## Research Background

Based on published research:
- Olive, S. E., & Toole, F. E. (1989). "The detection of reflections in typical rooms"
- Olive, S. E. (2004). "A method for training listeners and selecting program material"  
- Harman International patent applications on preference scoring algorithms

## License

GPL-3.0-or-later
