# ReplayGain Analysis - Tauri Integration

This document describes how to use the ReplayGain analysis functionality in the Tauri frontend.

## Overview

The `analyze_replaygain` Tauri command analyzes audio files and returns ReplayGain 2.0 loudness and peak values according to the EBU R128 standard.

## Supported Formats

- FLAC
- MP3
- AAC (including M4A/MP4 containers)
- WAV
- Vorbis (OGG)
- AIFF

## TypeScript Interface

```typescript
interface ReplayGainInfo {
  /** ReplayGain 2.0 Track Gain in dB */
  gain: number;
  
  /** ReplayGain 2.0 Track Peak (0.0 to 1.0+) */
  peak: number;
}
```

## Usage

### Basic Example

```typescript
import { invoke } from '@tauri-apps/api/core';

async function analyzeAudioFile(filePath: string): Promise<ReplayGainInfo> {
  try {
    const result = await invoke<ReplayGainInfo>('analyze_replaygain', {
      filePath: filePath
    });
    
    console.log(`ReplayGain: ${result.gain.toFixed(2)} dB`);
    console.log(`Peak: ${result.peak.toFixed(6)}`);
    
    return result;
  } catch (error) {
    console.error('ReplayGain analysis failed:', error);
    throw error;
  }
}
```

### With File Dialog

```typescript
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

async function analyzeSelectedFile() {
  // Open file picker
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Audio Files',
      extensions: ['flac', 'mp3', 'm4a', 'aac', 'wav', 'ogg', 'aiff', 'aif']
    }]
  });
  
  if (!selected || Array.isArray(selected)) {
    return;
  }
  
  try {
    const info = await invoke<ReplayGainInfo>('analyze_replaygain', {
      filePath: selected
    });
    
    // Display results
    alert(`ReplayGain Analysis Complete\n\nGain: ${info.gain.toFixed(2)} dB\nPeak: ${info.peak.toFixed(6)}`);
  } catch (error) {
    alert(`Analysis failed: ${error}`);
  }
}
```

### React Component Example

```tsx
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface ReplayGainInfo {
  gain: number;
  peak: number;
}

export function ReplayGainAnalyzer() {
  const [analyzing, setAnalyzing] = useState(false);
  const [result, setResult] = useState<ReplayGainInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleAnalyze = async () => {
    setAnalyzing(true);
    setError(null);
    setResult(null);

    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Audio Files',
          extensions: ['flac', 'mp3', 'm4a', 'aac', 'wav', 'ogg', 'aiff']
        }]
      });

      if (!selected || Array.isArray(selected)) {
        setAnalyzing(false);
        return;
      }

      const info = await invoke<ReplayGainInfo>('analyze_replaygain', {
        filePath: selected
      });

      setResult(info);
    } catch (err) {
      setError(String(err));
    } finally {
      setAnalyzing(false);
    }
  };

  return (
    <div className="replaygain-analyzer">
      <button 
        onClick={handleAnalyze} 
        disabled={analyzing}
      >
        {analyzing ? 'Analyzing...' : 'Analyze Audio File'}
      </button>

      {result && (
        <div className="results">
          <h3>ReplayGain Results</h3>
          <p>Gain: {result.gain.toFixed(2)} dB</p>
          <p>Peak: {result.peak.toFixed(6)}</p>
        </div>
      )}

      {error && (
        <div className="error">
          Error: {error}
        </div>
      )}
    </div>
  );
}
```

## Understanding the Values

### Gain (dB)

The ReplayGain value indicates how much the track should be adjusted to reach the reference level (-18.0 LUFS):

- **Negative values** (e.g., -5.0 dB): Track is louder than reference → reduce volume
- **Positive values** (e.g., +3.0 dB): Track is quieter than reference → increase volume
- **Zero**: Track is at reference level

### Peak (0.0 to 1.0+)

The maximum sample peak across all channels:

- Values **< 1.0**: No clipping will occur when applying the gain
- Values **≥ 1.0**: The audio already contains clipping or will clip if gain is applied
- Typically ranges from 0.5 to 1.0 for well-mastered audio

### Preventing Clipping

When applying ReplayGain, ensure the adjusted peak doesn't exceed 1.0:

```typescript
function calculateSafeGain(info: ReplayGainInfo): number {
  const targetPeak = 1.0; // Maximum safe level
  const gainLinear = Math.pow(10, info.gain / 20); // Convert dB to linear
  const adjustedPeak = info.peak * gainLinear;
  
  if (adjustedPeak > targetPeak) {
    // Reduce gain to prevent clipping
    const safeGainLinear = targetPeak / info.peak;
    return 20 * Math.log10(safeGainLinear);
  }
  
  return info.gain;
}
```

## Error Handling

Common error scenarios:

- **File not found**: The specified path doesn't exist
- **Unsupported format**: File extension not in supported list
- **Decoding failed**: File is corrupted or uses unsupported codec variant
- **Invalid file**: File is not a valid audio file

## Performance Considerations

- Analysis is CPU-intensive and processes the entire file
- For large files (> 100 MB), expect analysis times of several seconds
- Consider showing a progress indicator during analysis
- Run analysis in a background task to avoid blocking the UI

## Integration with Audio Playback

After analyzing a file, you can apply the gain during playback:

```typescript
async function playWithReplayGain(filePath: string) {
  // Analyze the file
  const rgInfo = await invoke<ReplayGainInfo>('analyze_replaygain', {
    filePath: filePath
  });
  
  // Convert gain to linear scale for volume adjustment
  const volumeAdjust = Math.pow(10, rgInfo.gain / 20);
  
  // Start playback with adjusted volume (example)
  await invoke('flac_load_file', { filePath });
  
  // Apply volume adjustment through your audio system
  // This depends on your audio playback implementation
}
```

## Reference

- [ReplayGain 2.0 Specification](https://wiki.hydrogenaud.io/index.php?title=ReplayGain_2.0_specification)
- [EBU R128 Loudness Standard](https://tech.ebu.ch/docs/r/r128.pdf)
