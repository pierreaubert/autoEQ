#!/usr/bin/env node

/**
 * Build script for standalone audio player module
 * Similar to plot-examples.rs but for the audio player
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BUILD_DIR = path.join(__dirname, '../../../dist-audio-player');
const SRC_DIR = __dirname; // Current directory (src/modules/audio)
const PUBLIC_AUDIO_DIR = path.join(__dirname, '../../../public/demo-audio');

console.log('🎵 Building standalone audio player...');

// Clean build directory
if (fs.existsSync(BUILD_DIR)) {
  fs.rmSync(BUILD_DIR, { recursive: true });
}
fs.mkdirSync(BUILD_DIR, { recursive: true });

try {
  // Copy audio player source files
  console.log('📁 Copying source files...');
  fs.copyFileSync(
    path.join(SRC_DIR, 'audio-player.ts'),
    path.join(BUILD_DIR, 'audio-player.ts')
  );
  fs.copyFileSync(
    path.join(SRC_DIR, 'audio-player.css'),
    path.join(BUILD_DIR, 'audio-player.css')
  );
  fs.copyFileSync(
    path.join(SRC_DIR, 'README.md'),
    path.join(BUILD_DIR, 'README.md')
  );

  // Copy wave files if they exist
  console.log('🎵 Copying audio files...');
  const audioSrcDir = PUBLIC_AUDIO_DIR;
  const audioDestDir = path.join(BUILD_DIR, 'demo-audio');
  let demoTracksConfig = '{}';

  if (fs.existsSync(audioSrcDir)) {
    fs.mkdirSync(audioDestDir, { recursive: true });
    const audioFiles = fs.readdirSync(audioSrcDir).filter(file =>
      file.endsWith('.wav') || file.endsWith('.mp3') || file.endsWith('.flac') || file.endsWith('.ogg')
    );

    if (audioFiles.length > 0) {
      const demoTracks = {};
      audioFiles.forEach(file => {
        const srcPath = path.join(audioSrcDir, file);
        const destPath = path.join(audioDestDir, file);
        fs.copyFileSync(srcPath, destPath);

        // Create demo track entry (remove extension and format name)
        const trackName = path.basename(file, path.extname(file));
        demoTracks[trackName] = `./demo-audio/${file}`;
      });

      demoTracksConfig = JSON.stringify(demoTracks, null, 8).replace(/\n/g, '\n        ');
      console.log(`📁 Copied ${audioFiles.length} audio files:`, Object.keys(demoTracks));
    }
  } else {
    console.log('⚠️  No demo-audio directory found, using empty demo tracks');
  }

  // Copy and fix demo file
  let demoContent = fs.readFileSync(path.join(SRC_DIR, 'audio-player-demo.html'), 'utf8');

  // Fix the import path in the demo
  demoContent = demoContent.replace(
    /import { AudioPlayer } from ['"]\.\/src\/modules\/audio\/audio-player\.js['"];/g,
    "import { AudioPlayer } from './audio-player.js';"
  );

  // Fix CSS path in the demo
  demoContent = demoContent.replace(
    /href=['"]src\/modules\/audio\/audio-player\.css['"]/g,
    'href="audio-player.css"'
  );

  // Update demo tracks configuration
  demoContent = demoContent.replace(
    /demoTracks: \{[^}]*\}/g,
    `demoTracks: ${demoTracksConfig}`
  );

  fs.writeFileSync(path.join(BUILD_DIR, 'demo.html'), demoContent);

  // Compile TypeScript to JavaScript
  console.log('🔨 Compiling TypeScript...');
  try {
    execSync(`npx tsc ${path.join(BUILD_DIR, 'audio-player.ts')} --target es2020 --module es2020 --lib es2020,dom --outDir ${BUILD_DIR}`, {
      stdio: 'inherit'
    });

    // Remove the .ts file after compilation
    fs.unlinkSync(path.join(BUILD_DIR, 'audio-player.ts'));
  } catch (error) {
    console.warn('⚠️  TypeScript compilation failed, keeping .ts file');
  }

  // Create package.json for the standalone module
  console.log('📦 Creating package.json...');
  const packageJson = {
    name: '@autoeq/audio-player',
    version: '1.0.0',
    description: 'Standalone audio player with EQ and spectrum analysis',
    main: 'audio-player.js',
    module: 'audio-player.js',
    types: 'audio-player.d.ts',
    files: [
      'audio-player.js',
      'audio-player.d.ts',
      'audio-player.css',
      'README.md',
      'demo.html'
    ],
    keywords: [
      'audio',
      'player',
      'equalizer',
      'spectrum',
      'web-audio',
      'typescript'
    ],
    author: 'AutoEQ Team',
    license: 'MIT',
    dependencies: {},
    peerDependencies: {},
    scripts: {
      demo: 'python -m http.server 8000'
    },
    repository: {
      type: 'git',
      url: 'https://github.com/autoeq/autoeq'
    },
    bugs: {
      url: 'https://github.com/autoeq/autoeq/issues'
    },
    homepage: 'https://github.com/autoeq/autoeq#readme'
  };

  fs.writeFileSync(
    path.join(BUILD_DIR, 'package.json'),
    JSON.stringify(packageJson, null, 2)
  );

  // Create TypeScript declaration file if compilation succeeded
  const jsFile = path.join(BUILD_DIR, 'audio-player.js');
  if (fs.existsSync(jsFile)) {
    console.log('📝 Creating TypeScript declarations...');
    const dtsContent = `
// TypeScript declarations for AudioPlayer module

export interface AudioPlayerConfig {
  demoTracks?: { [key: string]: string };
  enableEQ?: boolean;
  maxFilters?: number;
  enableSpectrum?: boolean;
  fftSize?: number;
  smoothingTimeConstant?: number;
  showProgress?: boolean;
  showFrequencyLabels?: boolean;
  compactMode?: boolean;
}

export interface FilterParam {
  frequency: number;
  q: number;
  gain: number;
}

export interface AudioPlayerCallbacks {
  onPlay?: () => void;
  onStop?: () => void;
  onEQToggle?: (enabled: boolean) => void;
  onTrackChange?: (trackName: string) => void;
  onError?: (error: string) => void;
}

export declare class AudioPlayer {
  constructor(
    container: HTMLElement,
    config?: AudioPlayerConfig,
    callbacks?: AudioPlayerCallbacks
  );

  // Playback control
  play(): Promise<void>;
  stop(): void;
  isPlaying(): boolean;

  // EQ control
  setEQEnabled(enabled: boolean): void;
  isEQEnabled(): boolean;
  updateFilterParams(filterParams: FilterParam[]): void;

  // Audio loading
  loadAudioFromUrl(url: string): Promise<void>;
  loadAudioFile(file: File): Promise<void>;

  // Utility
  getCurrentTrack(): string | null;
  destroy(): void;
}
`;

    fs.writeFileSync(path.join(BUILD_DIR, 'audio-player.d.ts'), dtsContent.trim());
  }

  // Create demo assets directory structure
  console.log('🎨 Setting up demo assets...');
  const demoAssetsDir = path.join(BUILD_DIR, 'demo-audio');
  fs.mkdirSync(demoAssetsDir, { recursive: true });

  // Create placeholder audio files info
  const audioInfo = {
    'classical.wav': 'Classical music sample',
    'country.wav': 'Country music sample',
    'edm.wav': 'Electronic dance music sample',
    'female_vocal.wav': 'Female vocal sample',
    'jazz.wav': 'Jazz music sample',
    'piano.wav': 'Piano music sample',
    'rock.wav': 'Rock music sample'
  };

  fs.writeFileSync(
    path.join(demoAssetsDir, 'README.md'),
    `# Demo Audio Files

This directory should contain the following audio files for the demo:

${Object.entries(audioInfo).map(([file, desc]) => `- **${file}**: ${desc}`).join('\n')}

## Adding Audio Files

1. Place your audio files in this directory
2. Ensure they match the filenames above
3. Supported formats: WAV, MP3, FLAC, OGG

## License

Make sure you have the rights to use any audio files you add here.
`
  );

  // Create usage examples
  console.log('📚 Creating usage examples...');
  const examplesDir = path.join(BUILD_DIR, 'examples');
  fs.mkdirSync(examplesDir, { recursive: true });

  // Basic example
  fs.writeFileSync(path.join(examplesDir, 'basic.html'), `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Basic Audio Player Example</title>
    <link rel="stylesheet" href="../audio-player.css">
</head>
<body>
    <h1>Basic Audio Player</h1>
    <div id="audio-player"></div>

    <script type="module">
        import { AudioPlayer } from '../audio-player.js';

        const player = new AudioPlayer(
            document.getElementById('audio-player'),
            {
                enableEQ: true,
                enableSpectrum: true
            },
            {
                onPlay: () => console.log('Playing'),
                onStop: () => console.log('Stopped')
            }
        );
    </script>
</body>
</html>`);

  // Compact example
  fs.writeFileSync(path.join(examplesDir, 'compact.html'), `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Compact Audio Player Example</title>
    <link rel="stylesheet" href="../audio-player.css">
</head>
<body>
    <h1>Compact Audio Player</h1>
    <div id="audio-player"></div>

    <script type="module">
        import { AudioPlayer } from '../audio-player.js';

        const player = new AudioPlayer(
            document.getElementById('audio-player'),
            {
                enableEQ: false,
                enableSpectrum: false,
                compactMode: true
            }
        );
    </script>
</body>
</html>`);

  console.log('✅ Build completed successfully!');
  console.log(`📁 Output directory: ${BUILD_DIR}`);
  console.log('');
  console.log('🚀 To test the demo:');
  console.log(`   cd ${BUILD_DIR}`);
  console.log('   python -m http.server 8000');
  console.log('   # Open http://localhost:8000/demo.html');
  console.log('');
  console.log('📦 To publish as npm package:');
  console.log(`   cd ${BUILD_DIR}`);
  console.log('   npm publish');

} catch (error) {
  console.error('❌ Build failed:', error.message);
  process.exit(1);
}
