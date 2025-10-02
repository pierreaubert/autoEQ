# Audio Capture Feature Implementation

## Overview

This document describes the implementation of the audio capture feature for AutoEQ, which allows users to measure frequency response using their microphone by playing and recording a frequency sweep.

## Key Components

### Frontend (TypeScript/JavaScript)

#### 1. AudioProcessor Module (`src-ui/src/modules/audio/audio-processor.ts`)
- Enhanced with frequency sweep generation and capture functionality
- **Key methods:**
  - `startCapture()`: Initiates audio capture with microphone permission
  - `performCaptureMeasurement()`: Plays exponential frequency sweep (20Hz-20kHz) and records response
  - `smoothAndResample()`: Applies 1/24 octave smoothing and resamples to 200 logarithmic points
  - `setSweepDuration()`: Allows configurable sweep duration (5s, 10s, 15s)
- **Features:**
  - Exponential frequency sweep generation using Web Audio API
  - Real-time FFT analysis with 8192-point FFT
  - 1/24 octave smoothing for cleaner frequency response
  - Logarithmic resampling to 200 points for optimization

#### 2. UI Manager (`src-ui/src/modules/ui-manager.ts`)
- Handles capture UI interactions
- Added callback system to connect captured data to optimization manager
- Manages sweep duration selector and capture status display

#### 3. Optimization Manager (`src-ui/src/modules/optimization-manager.ts`)
- Stores captured frequency and magnitude data
- Methods: `setCapturedData()`, `clearCapturedData()`, `hasCapturedData()`
- Passes captured data to backend when input source is "capture"

#### 4. Main Application (`src-ui/src/main.ts`)
- Wires together UI callbacks and data flow
- Connects capture completion to optimization manager data storage

### Backend (Rust)

#### 1. Tauri Command Handler (`src-ui/src-tauri/src/lib.rs`)
- Extended `OptimizationParams` struct with:
  - `captured_frequencies: Option<Vec<f64>>`
  - `captured_magnitudes: Option<Vec<f64>>`
- Modified `run_optimization_internal()` to use captured data when available
- Creates `Curve` struct from captured arrays instead of loading from file

### UI Template Updates

#### Capture Tab Content (`src-ui/src/modules/templates.ts`)
The Capture tab includes:
- Microphone device selector (populated dynamically)
- Output channel selector (Left, Right, Both, System Default)
- Sample rate selector (44.1kHz, 48kHz, 96kHz, 192kHz)
- Signal type selector (Frequency Sweep, White Noise, Pink Noise)
- Duration selector (5s, 10s, 15s, 20s - only for sweep)
- Start/Stop capture button
- Real-time status display
- Progress bar for sweep playback
- Waveform and spectrum visualization canvases
- Captured response plot area

## Data Flow

1. **User Interaction**: User selects microphone and sweep duration, clicks "Start Capture"
2. **Audio Setup**: AudioProcessor requests microphone permission and initializes Web Audio API
3. **Sweep Generation**: Creates exponential sine sweep from 20Hz to 20kHz
4. **Recording**: Captures frequency response using FFT analysis at 100ms intervals
5. **Processing**:
   - Averages multiple FFT samples
   - Applies 1/24 octave smoothing
   - Resamples to 200 logarithmic points
6. **Storage**: Captured data stored in OptimizationManager
7. **Optimization**: When user runs optimization with "capture" input, data is sent to Rust backend
8. **Backend Processing**: Rust creates Curve from captured arrays and proceeds with normal optimization

## Technical Details

### Test Signals
- **Frequency Sweep**:
  - Type: Exponential (logarithmic) sweep
  - Range: 20 Hz to Nyquist frequency (depends on sample rate)
- **White Noise**:
  - Flat spectrum (equal energy per Hz)
  - Good for broadband measurements
- **Pink Noise**:
  - 1/f spectrum (equal energy per octave)
  - Better matches natural sounds and music
- **Duration**: User-selectable (5s, 10s, 15s, 20s)
- **Volume**: 0.3 (30% to avoid feedback)
- **Output Channels**: User-selectable (Left only, Right only, Both channels, System default)

### FFT Analysis
- **FFT Size**: 8192 samples
- **Sample Rate**: User-selectable (44.1, 48, 96, 192 kHz)
- **Smoothing Time Constant**: 0.1
- **Sampling Interval**: 100ms

### Data Processing
- **Smoothing**: 1/24 octave bandwidth
- **Output Points**: 200 logarithmically-spaced frequencies
- **Magnitude Units**: Decibels (dB)

## Implementation Notes

1. **Browser Compatibility**: Uses standard Web Audio API, requires modern browser
2. **Microphone Permission**: Automatically requests permission when starting capture
3. **Feedback Prevention**: Sweep played at reduced volume (30%)
4. **Error Handling**: Graceful fallback if microphone access denied
5. **Real-time Visualization**: Shows waveform and spectrum during capture
6. **Device Selection**: Audio devices are enumerated on UI initialization and populated in dropdown
7. **Canvas Plot**: Frequency response rendered using HTML5 Canvas with logarithmic frequency scale

## Usage Instructions

1. Navigate to the "Capture" tab in the Data Acquisition section
2. Select your measurement microphone from the dropdown
3. Select output channel (useful for testing individual speakers)
4. Select sample rate (higher = better frequency resolution)
5. Choose signal type (sweep for accuracy, noise for speed)
6. Choose duration (longer = more accurate, especially for noise)
7. Click "Start Capture"
8. Allow microphone permission if prompted
9. Wait for the signal to complete
10. Review the captured frequency response
11. Run optimization with captured data as input

## Fixes Applied

### Issue 1: Audio Device Selection Not Working
- **Problem**: Device dropdown was not populated
- **Solution**: Added `enumerateAudioDevices()` method to AudioProcessor
- **Implementation**: Devices are enumerated on UI initialization and when capture starts
- **Result**: Users can now select from available microphones

### Issue 2: Capture Plot Not Visible
- **Problem**: Plot container was blank after capture
- **Solution**: Implemented canvas-based plotting in `plotCapturedData()`
- **Features**:
  - Logarithmic frequency axis (20Hz-20kHz)
  - Grid lines for easy reading
  - Proper axis labels and title
  - Responsive sizing based on container width
- **Result**: Frequency response is now clearly visible after capture

### Enhancement: Output Channel Selection
- **Feature**: Added output channel selector for sweep playback
- **Options**:
  - Both Channels (default) - plays sweep on both L/R channels
  - Left Channel Only - useful for testing left speaker only
  - Right Channel Only - useful for testing right speaker only
  - System Default - uses system audio routing
- **Implementation**: Uses Web Audio API's ChannelMergerNode for routing
- **Use Case**: Allows measuring individual speakers in a stereo setup

### Enhancement: Sample Rate Selection
- **Feature**: Configurable sample rate for capture
- **Options**: 44.1 kHz, 48 kHz, 96 kHz, 192 kHz
- **Benefits**:
  - Higher rates provide better frequency resolution
  - 192 kHz allows measurement up to 96 kHz (ultrasonic)
  - Lower rates reduce processing load
- **Implementation**: Creates new AudioContext with selected sample rate

### Enhancement: Signal Type Selection
- **Feature**: Choice of test signal
- **Options**:
  - Frequency Sweep: Most accurate, deterministic
  - White Noise: Fast, good for broadband response
  - Pink Noise: Natural spectrum, good for room acoustics
- **Implementation**:
  - Sweep uses oscillator with exponential frequency ramp
  - Noise generated using random values (white) or Paul Kellet algorithm (pink)
- **Use Cases**:
  - Sweep: Precise measurements, best SNR
  - Noise: Quick measurements, real-time adjustments

## Future Enhancements

- Multiple measurement averaging
- Pink/white noise measurement option
- Room correction measurements (multiple positions)
- Calibration file support for measurement microphones
- Export captured data to CSV
- A/B comparison of multiple captures
- Integration with Plotly for interactive plots
