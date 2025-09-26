# Audio Player Module

A standalone, reusable audio player component extracted from the AutoEQ application. This module provides comprehensive audio playback functionality with EQ controls, spectrum analysis, and a modern UI.

## Features

- **Audio Playback**: Support for various audio formats via Web Audio API
- **EQ Controls**: Real-time parametric EQ with biquad filters
- **Spectrum Analyzer**: Real-time frequency visualization
- **Demo Tracks**: Built-in demo audio track selection
- **File Loading**: Support for loading external audio files
- **Progress Tracking**: Real-time playback position and duration
- **Responsive Design**: Mobile-friendly responsive layout
- **Event System**: Comprehensive callback system for integration

## Quick Start

### Basic Usage

```typescript
import { AudioPlayer } from './modules/audio-player';

// Create a basic audio player
const player = new AudioPlayer(
  document.getElementById('audio-container'),
  {
    enableEQ: true,
    enableSpectrum: true,
    showProgress: true
  },
  {
    onPlay: () => console.log('Playing'),
    onStop: () => console.log('Stopped'),
    onError: (error) => console.error('Error:', error)
  }
);
```

### HTML Structure

```html
<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="modules/audio-player.css">
</head>
<body>
  <div id="audio-container"></div>
  <script type="module" src="your-script.js"></script>
</body>
</html>
```

## Configuration Options

### AudioPlayerConfig

```typescript
interface AudioPlayerConfig {
  // Demo audio tracks configuration
  demoTracks?: { [key: string]: string };

  // EQ configuration
  enableEQ?: boolean;
  maxFilters?: number;

  // Spectrum analyzer configuration
  enableSpectrum?: boolean;
  fftSize?: number;
  smoothingTimeConstant?: number;

  // UI configuration
  showProgress?: boolean;
  showFrequencyLabels?: boolean;
  compactMode?: boolean;
}
```

### Default Configuration

```typescript
{
  enableEQ: true,
  maxFilters: 10,
  enableSpectrum: true,
  fftSize: 2048,
  smoothingTimeConstant: 0.8,
  showProgress: true,
  showFrequencyLabels: true,
  compactMode: false,
  demoTracks: {
    'classical': '/demo-audio/classical.wav',
    'country': '/demo-audio/country.wav',
    'edm': '/demo-audio/edm.wav',
    'female_vocal': '/demo-audio/female_vocal.wav',
    'jazz': '/demo-audio/jazz.wav',
    'piano': '/demo-audio/piano.wav',
    'rock': '/demo-audio/rock.wav'
  }
}
```

## Callbacks

### AudioPlayerCallbacks

```typescript
interface AudioPlayerCallbacks {
  onPlay?: () => void;
  onStop?: () => void;
  onEQToggle?: (enabled: boolean) => void;
  onTrackChange?: (trackName: string) => void;
  onError?: (error: string) => void;
}
```

## API Methods

### Playback Control

```typescript
// Start playback
await player.play();

// Stop playback
player.stop();

// Check if playing
const isPlaying = player.isPlaying();
```

### EQ Control

```typescript
// Set EQ enabled/disabled
player.setEQEnabled(true);

// Check EQ status
const eqEnabled = player.isEQEnabled();

// Update EQ filters
player.updateFilterParams([
  { frequency: 100, q: 1, gain: 3 },
  { frequency: 1000, q: 2, gain: -2 },
  { frequency: 10000, q: 1, gain: 1 }
]);
```

### Audio Loading

```typescript
// Load from URL
await player.loadAudioFromUrl('/path/to/audio.wav');

// Load from file
const fileInput = document.getElementById('file-input');
fileInput.addEventListener('change', async (e) => {
  const file = e.target.files[0];
  if (file) {
    await player.loadAudioFile(file);
  }
});
```

### Utility Methods

```typescript
// Get current track
const currentTrack = player.getCurrentTrack();

// Cleanup resources
player.destroy();
```

## Examples

### Minimal Player

```typescript
const minimalPlayer = new AudioPlayer(
  document.getElementById('minimal-player'),
  {
    enableEQ: false,
    enableSpectrum: false,
    compactMode: true
  }
);
```

### Full-Featured Player

```typescript
const fullPlayer = new AudioPlayer(
  document.getElementById('full-player'),
  {
    enableEQ: true,
    enableSpectrum: true,
    showProgress: true,
    showFrequencyLabels: true,
    fftSize: 4096,
    maxFilters: 20
  },
  {
    onPlay: () => console.log('Playback started'),
    onStop: () => console.log('Playback stopped'),
    onEQToggle: (enabled) => console.log(`EQ ${enabled ? 'on' : 'off'}`),
    onTrackChange: (track) => console.log(`Track: ${track}`),
    onError: (error) => console.error(`Player error: ${error}`)
  }
);
```

### Custom Demo Tracks

```typescript
const customPlayer = new AudioPlayer(
  document.getElementById('custom-player'),
  {
    demoTracks: {
      'my_track_1': '/audio/track1.mp3',
      'my_track_2': '/audio/track2.wav',
      'my_track_3': '/audio/track3.flac'
    }
  }
);
```

## CSS Customization

The audio player uses CSS custom properties (variables) for theming:

```css
:root {
  --bg-primary: #ffffff;
  --bg-secondary: #f8f9fa;
  --text-primary: #212529;
  --text-secondary: #6c757d;
  --button-primary: #007bff;
  --button-primary-hover: #0056b3;
  --border-color: #dee2e6;
  --danger-color: #dc3545;
}
```

### Dark Theme

```css
@media (prefers-color-scheme: dark) {
  :root {
    --bg-primary: #1a1a1a;
    --bg-secondary: #2d2d2d;
    --text-primary: #ffffff;
    --text-secondary: #b0b0b0;
    --button-primary: #4dabf7;
    --button-primary-hover: #339af0;
  }
}
```

### Custom Styling

```css
.audio-player {
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.audio-player .listen-button {
  background: linear-gradient(45deg, #ff6b6b, #ee5a24);
}
```

## Browser Compatibility

- **Chrome**: 66+
- **Firefox**: 60+
- **Safari**: 14.1+
- **Edge**: 79+

### Required Features

- Web Audio API
- ES6 Modules
- CSS Custom Properties
- Canvas API (for spectrum analyzer)
- File API (for file loading)

## Demo

Run the included demo to see all features in action:

```bash
# Serve the demo locally
python -m http.server 8000
# or
npx serve .

# Open http://localhost:8000/audio-player-demo.html
```

## Integration with Existing Projects

### With AutoEQ Application

The audio player can be integrated back into the main AutoEQ application:

```typescript
import { AudioPlayer } from './modules/audio-player';

// Replace existing audio functionality
const audioContainer = document.getElementById('audio-controls');
const player = new AudioPlayer(audioContainer, config, callbacks);

// Connect to optimization results
optimizationManager.onComplete((result) => {
  if (result.filter_params) {
    const filters = result.filter_params.map((param, i) => ({
      frequency: param[0],
      q: param[1],
      gain: param[2]
    }));
    player.updateFilterParams(filters);
  }
});
```

### With Other Applications

```typescript
// Standalone integration
import { AudioPlayer } from './path/to/audio-player';

const player = new AudioPlayer(
  document.getElementById('audio-player'),
  { /* config */ },
  {
    onTrackChange: (track) => {
      // Update your app's state
      updateCurrentTrack(track);
    },
    onEQToggle: (enabled) => {
      // Sync with your app's EQ state
      syncEQState(enabled);
    }
  }
);
```

## Troubleshooting

### Common Issues

1. **Audio not playing**: Check browser autoplay policies
2. **No spectrum visualization**: Ensure canvas element is properly sized
3. **EQ not working**: Verify filter parameters are valid
4. **File loading fails**: Check file format support and CORS headers

### Debug Mode

Enable console logging for debugging:

```typescript
// Add to your configuration
const player = new AudioPlayer(container, config, {
  ...callbacks,
  onError: (error) => {
    console.error('AudioPlayer Error:', error);
    // Your error handling
  }
});
```

## Performance Considerations

- **Large Files**: Consider streaming for files > 50MB
- **Multiple Players**: Limit concurrent players to avoid audio context limits
- **Mobile**: Reduce FFT size on mobile devices for better performance
- **Memory**: Call `destroy()` when removing players to prevent memory leaks

## License

This module is part of the AutoEQ project and follows the same license terms.
