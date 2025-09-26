// Standalone Audio Player Module
// Extracted from audio-processor.ts and related UI components

export interface AudioPlayerConfig {
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

export class AudioPlayer {
  private audioContext: AudioContext | null = null;
  private audioBuffer: AudioBuffer | null = null;
  private audioSource: AudioBufferSourceNode | null = null;
  private eqFilters: BiquadFilterNode[] = [];
  private gainNode: GainNode | null = null;
  private isAudioPlaying: boolean = false;
  private audioStartTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentFilterParams: FilterParam[] = [];
  private eqEnabled: boolean = true;

  // Frequency analyzer
  private analyserNode: AnalyserNode | null = null;
  private spectrumCanvas: HTMLCanvasElement | null = null;
  private spectrumCtx: CanvasRenderingContext2D | null = null;
  private spectrumAnimationFrame: number | null = null;

  // UI Elements
  private container: HTMLElement;
  private demoSelect: HTMLSelectElement | null = null;
  private listenBtn: HTMLButtonElement | null = null;
  private stopBtn: HTMLButtonElement | null = null;
  private eqOnBtn: HTMLButtonElement | null = null;
  private eqOffBtn: HTMLButtonElement | null = null;
  private statusText: HTMLElement | null = null;
  private positionText: HTMLElement | null = null;
  private durationText: HTMLElement | null = null;
  private progressFill: HTMLElement | null = null;

  // Configuration
  private config: AudioPlayerConfig;
  private callbacks: AudioPlayerCallbacks;
  private instanceId: string;

  constructor(container: HTMLElement, config: AudioPlayerConfig = {}, callbacks: AudioPlayerCallbacks = {}) {
    this.container = container;
    this.instanceId = 'audio-player-' + Math.random().toString(36).substr(2, 9);
    this.config = {
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
      },
      ...config
    };
    this.callbacks = callbacks;

    this.init();
  }

  private async init(): Promise<void> {
    try {
      await this.setupAudioContext();
      this.createUI();
      this.setupEventListeners();
      console.log('AudioPlayer initialized successfully');
    } catch (error) {
      console.error('Failed to initialize AudioPlayer:', error);
      this.callbacks.onError?.('Failed to initialize audio player: ' + error);
    }
  }

  private async setupAudioContext(): Promise<void> {
    try {
      this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
      this.gainNode = this.audioContext.createGain();

      if (this.config.enableSpectrum) {
        this.analyserNode = this.audioContext.createAnalyser();
        this.analyserNode.fftSize = this.config.fftSize || 2048;
        this.analyserNode.smoothingTimeConstant = this.config.smoothingTimeConstant || 0.8;
      }
    } catch (error) {
      console.error('Failed to initialize audio context:', error);
      throw error;
    }
  }

  private createUI(): void {
    const selectId = `demo-audio-select-${this.instanceId}`;
    const html = `
      <div class="audio-player">
        <div class="audio-control-row">
          <div class="audio-left-controls">
            <label for="${selectId}">Demo Track</label>
            <select id="${selectId}" class="demo-audio-select">
              <option value="">Select track...</option>
              ${Object.keys(this.config.demoTracks || {}).map(key =>
                `<option value="${key}">${this.formatTrackName(key)}</option>`
              ).join('')}
            </select>
          </div>

          <div class="audio-center-controls">
            <div class="audio-playback-container">
              ${this.config.showProgress ? `
                <div class="audio-status" style="display: none;">
                  <div class="audio-info-compact">
                    <span class="audio-status-text">Ready</span> •
                    <span class="audio-position">--:--</span> •
                    <span class="audio-duration">--:--</span>
                  </div>
                  <div class="audio-progress">
                    <div class="audio-progress-bar">
                      <div class="audio-progress-fill" style="width: 0%;"></div>
                    </div>
                  </div>
                </div>
              ` : ''}

              ${this.config.enableSpectrum ? `
                <div class="frequency-analyzer" style="display: none;">
                  <canvas class="spectrum-canvas"></canvas>
                  ${this.config.showFrequencyLabels ? `
                    <div class="frequency-labels">
                      <span class="freq-label" data-range="sub-bass">Sub Bass<br><small>&lt;60Hz</small></span>
                      <span class="freq-label" data-range="bass">Bass<br><small>60-250Hz</small></span>
                      <span class="freq-label" data-range="low-mid">Low Mid<br><small>250-500Hz</small></span>
                      <span class="freq-label" data-range="mid">Mid<br><small>500-2kHz</small></span>
                      <span class="freq-label" data-range="high-mid">High Mid<br><small>2-4kHz</small></span>
                      <span class="freq-label" data-range="presence">Presence<br><small>4-6kHz</small></span>
                      <span class="freq-label" data-range="brilliance">Brilliance<br><small>6-20kHz</small></span>
                    </div>
                  ` : ''}
                </div>
              ` : ''}
            </div>
          </div>

          <div class="audio-right-controls">
            ${this.config.enableEQ ? `
              <div class="eq-toggle-buttons">
                <button type="button" class="eq-toggle-btn eq-on-btn active">On</button>
                <button type="button" class="eq-toggle-btn eq-off-btn">Off</button>
              </div>
            ` : ''}
            <button type="button" class="listen-button" disabled>
              🎵 Listen
            </button>
            <button type="button" class="stop-button" disabled style="display: none;">
              ⏹️ Stop
            </button>
          </div>
        </div>
      </div>
    `;

    this.container.innerHTML = html;
    this.cacheUIElements();
  }

  private cacheUIElements(): void {
    this.demoSelect = this.container.querySelector('.demo-audio-select');
    this.listenBtn = this.container.querySelector('.listen-button');
    this.stopBtn = this.container.querySelector('.stop-button');
    this.eqOnBtn = this.container.querySelector('.eq-on-btn');
    this.eqOffBtn = this.container.querySelector('.eq-off-btn');
    this.statusText = this.container.querySelector('.audio-status-text');
    this.positionText = this.container.querySelector('.audio-position');
    this.durationText = this.container.querySelector('.audio-duration');
    this.progressFill = this.container.querySelector('.audio-progress-fill');
    this.spectrumCanvas = this.container.querySelector('.spectrum-canvas');

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext('2d');
    }
  }

  private setupEventListeners(): void {
    // Demo track selection
    this.demoSelect?.addEventListener('change', async (e) => {
      const trackName = (e.target as HTMLSelectElement).value;
      if (trackName) {
        await this.loadDemoTrack(trackName);
        this.callbacks.onTrackChange?.(trackName);
      } else {
        this.clearAudio();
      }
    });

    // Playback controls
    this.listenBtn?.addEventListener('click', () => this.play());
    this.stopBtn?.addEventListener('click', () => this.stop());

    // EQ controls
    this.eqOnBtn?.addEventListener('click', () => this.setEQEnabled(true));
    this.eqOffBtn?.addEventListener('click', () => this.setEQEnabled(false));
  }

  private formatTrackName(key: string): string {
    return key.split('_').map(word =>
      word.charAt(0).toUpperCase() + word.slice(1)
    ).join(' ');
  }

  private async loadDemoTrack(trackName: string): Promise<void> {
    const url = this.config.demoTracks?.[trackName];
    if (!url) {
      throw new Error(`Demo track '${trackName}' not found`);
    }

    this.setStatus('Loading audio...');
    this.setListenButtonEnabled(false);

    try {
      await this.loadAudioFromUrl(url);
      this.setStatus('Audio ready');
      this.setListenButtonEnabled(true);
      this.showAudioStatus(true);
    } catch (error) {
      this.setStatus('Failed to load audio');
      this.callbacks.onError?.('Failed to load demo track: ' + error);
      throw error;
    }
  }

  private async loadAudioFromUrl(url: string): Promise<void> {
    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    try {
      console.log('Loading audio from URL:', url);
      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(`Failed to fetch audio: ${response.status} ${response.statusText}`);
      }

      const arrayBuffer = await response.arrayBuffer();
      console.log('Audio data fetched, decoding...');

      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log('Audio loaded successfully:', {
        duration: this.audioBuffer.duration,
        sampleRate: this.audioBuffer.sampleRate,
        channels: this.audioBuffer.numberOfChannels
      });

      this.updateAudioInfo();
    } catch (error) {
      console.error('Error loading audio from URL:', error);
      throw error;
    }
  }

  private clearAudio(): void {
    this.stop();
    this.audioBuffer = null;
    this.setListenButtonEnabled(false);
    this.showAudioStatus(false);
    this.setStatus('No audio selected');
  }

  private setStatus(status: string): void {
    if (this.statusText) {
      this.statusText.textContent = status;
    }
  }

  private setListenButtonEnabled(enabled: boolean): void {
    if (this.listenBtn) {
      this.listenBtn.disabled = !enabled;
      if (enabled) {
        this.listenBtn.classList.remove('disabled');
      } else {
        this.listenBtn.classList.add('disabled');
      }
    }
  }

  private showAudioStatus(show: boolean): void {
    const audioStatus = this.container.querySelector('.audio-status') as HTMLElement;
    if (audioStatus) {
      audioStatus.style.display = show ? 'flex' : 'none';
    }
  }

  private updateAudioInfo(): void {
    if (this.audioBuffer && this.durationText) {
      const duration = this.audioBuffer.duration;
      this.durationText.textContent = this.formatTime(duration);
    }
  }

  private formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  // EQ Filter Management
  updateFilterParams(filterParams: FilterParam[]): void {
    this.currentFilterParams = [...filterParams];
    this.setupEQFilters();
  }

  private setupEQFilters(): void {
    if (!this.audioContext || !this.gainNode) return;

    // Clear existing filters
    this.eqFilters.forEach(filter => filter.disconnect());
    this.eqFilters = [];

    // Create new filters from parameters
    this.currentFilterParams.forEach(param => {
      if (Math.abs(param.gain) > 0.1) { // Only create filter if gain is significant
        const filter = this.audioContext!.createBiquadFilter();
        filter.type = 'peaking';
        filter.frequency.value = param.frequency;
        filter.Q.value = param.q;
        filter.gain.value = param.gain;
        this.eqFilters.push(filter);
      }
    });

    console.log(`Created ${this.eqFilters.length} EQ filters`);
  }

  private connectAudioChain(): void {
    if (!this.audioSource || !this.gainNode || !this.audioContext) {
      console.error('Cannot connect audio chain - missing components');
      return;
    }

    console.log('Connecting audio chain with', this.eqFilters.length, 'EQ filters');
    let currentNode: AudioNode = this.audioSource;

    // Connect EQ filters in series if EQ is enabled
    if (this.eqEnabled) {
      this.eqFilters.forEach((filter, index) => {
        console.log(`Connecting EQ filter ${index + 1}`);
        currentNode.connect(filter);
        currentNode = filter;
      });
    }

    // Connect to gain and analyzer
    currentNode.connect(this.gainNode);
    if (this.analyserNode) {
      this.gainNode.connect(this.analyserNode);
      this.analyserNode.connect(this.audioContext.destination);
    } else {
      this.gainNode.connect(this.audioContext.destination);
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    // Update button states
    if (this.eqOnBtn && this.eqOffBtn) {
      if (enabled) {
        this.eqOnBtn.classList.add('active');
        this.eqOffBtn.classList.remove('active');
      } else {
        this.eqOnBtn.classList.remove('active');
        this.eqOffBtn.classList.add('active');
      }
    }

    // Reconnect audio chain if playing
    if (this.isAudioPlaying && this.audioSource) {
      this.connectAudioChain();
    }

    console.log(`EQ ${enabled ? 'enabled' : 'disabled'}`);
    this.callbacks.onEQToggle?.(enabled);
  }

  // Spectrum Analyzer
  private startSpectrumAnalysis(): void {
    if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

    const frequencyAnalyzer = this.container.querySelector('.frequency-analyzer') as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = 'flex';
    }

    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);

    const draw = () => {
      if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx || !this.isAudioPlaying) return;

      this.analyserNode.getByteFrequencyData(dataArray);

      const width = this.spectrumCanvas.width;
      const height = this.spectrumCanvas.height;

      this.spectrumCtx.fillStyle = 'rgb(0, 0, 0)';
      this.spectrumCtx.fillRect(0, 0, width, height);

      const barWidth = width / dataArray.length;
      let x = 0;

      for (let i = 0; i < dataArray.length; i++) {
        const barHeight = (dataArray[i] / 255) * height;

        this.spectrumCtx.fillStyle = `rgb(${barHeight + 100}, 50, 50)`;
        this.spectrumCtx.fillRect(x, height - barHeight, barWidth, barHeight);

        x += barWidth;
      }

      this.spectrumAnimationFrame = requestAnimationFrame(draw);
    };

    draw();
  }

  private stopSpectrumAnalysis(): void {
    if (this.spectrumAnimationFrame) {
      cancelAnimationFrame(this.spectrumAnimationFrame);
      this.spectrumAnimationFrame = null;
    }

    const frequencyAnalyzer = this.container.querySelector('.frequency-analyzer') as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = 'none';
    }
  }

  // Position Updates
  private startPositionUpdates(): void {
    const updatePosition = () => {
      if (!this.isAudioPlaying) {
        this.audioAnimationFrame = null;
        return;
      }

      const currentTime = this.getCurrentTime();
      const duration = this.getDuration();

      if (this.positionText) {
        this.positionText.textContent = this.formatTime(currentTime);
      }

      if (this.progressFill && duration > 0) {
        const progress = (currentTime / duration) * 100;
        this.progressFill.style.width = `${Math.min(progress, 100)}%`;
      }

      this.audioAnimationFrame = requestAnimationFrame(updatePosition);
    };

    updatePosition();
  }

  private getCurrentTime(): number {
    if (!this.audioContext || !this.isAudioPlaying) return 0;
    return this.audioContext.currentTime - this.audioStartTime;
  }

  private getDuration(): number {
    return this.audioBuffer ? this.audioBuffer.duration : 0;
  }

  // Playback Controls
  async play(): Promise<void> {
    console.log('Play method called');

    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    if (!this.audioBuffer) {
      throw new Error('No audio loaded for playback');
    }

    // Resume audio context if suspended
    if (this.audioContext.state === 'suspended') {
      console.log('Resuming suspended audio context...');
      await this.audioContext.resume();
    }

    this.stop(); // Stop any currently playing audio

    try {
      this.audioSource = this.audioContext.createBufferSource();
      this.audioSource.buffer = this.audioBuffer;

      this.connectAudioChain();

      this.audioSource.start();
      this.audioStartTime = this.audioContext.currentTime;
      this.isAudioPlaying = true;

      this.audioSource.onended = () => {
        console.log('Audio playback ended');
        this.isAudioPlaying = false;
        this.audioSource = null;
        this.updatePlaybackUI();
        if (this.audioAnimationFrame) {
          cancelAnimationFrame(this.audioAnimationFrame);
          this.audioAnimationFrame = null;
        }
      };

      this.updatePlaybackUI();
      this.startPositionUpdates();

      if (this.config.enableSpectrum) {
        this.startSpectrumAnalysis();
      }

      this.callbacks.onPlay?.();
      console.log('Audio playback started successfully');
    } catch (error) {
      console.error('Error during audio playback:', error);
      this.callbacks.onError?.('Playback failed: ' + error);
      throw error;
    }
  }

  stop(): void {
    if (this.audioSource) {
      try {
        this.audioSource.stop();
      } catch (error) {
        // Ignore errors if already stopped
      }
      this.audioSource = null;
    }

    this.isAudioPlaying = false;

    if (this.audioAnimationFrame) {
      cancelAnimationFrame(this.audioAnimationFrame);
      this.audioAnimationFrame = null;
    }

    this.stopSpectrumAnalysis();
    this.updatePlaybackUI();
    this.callbacks.onStop?.();
    console.log('Audio playback stopped');
  }

  private updatePlaybackUI(): void {
    const isPlaying = this.isAudioPlaying;

    if (this.listenBtn) {
      this.listenBtn.style.display = isPlaying ? 'none' : 'flex';
    }

    if (this.stopBtn) {
      this.stopBtn.style.display = isPlaying ? 'flex' : 'none';
      this.stopBtn.disabled = !isPlaying;
    }

    if (this.statusText) {
      const status = isPlaying ?
        (this.eqEnabled ? 'Playing (EQ On)' : 'Playing (EQ Off)') :
        'Audio ready';
      this.statusText.textContent = status;
    }
  }

  // Public API
  isPlaying(): boolean {
    return this.isAudioPlaying;
  }

  isEQEnabled(): boolean {
    return this.eqEnabled;
  }

  getCurrentTrack(): string | null {
    return this.demoSelect?.value || null;
  }

  // Load external audio file
  async loadAudioFile(file: File): Promise<void> {
    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    try {
      const arrayBuffer = await file.arrayBuffer();
      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log('Audio file loaded successfully');

      this.updateAudioInfo();
      this.setListenButtonEnabled(true);
      this.showAudioStatus(true);
      this.setStatus('Audio ready');
    } catch (error) {
      console.error('Error loading audio file:', error);
      this.callbacks.onError?.('Failed to load audio file: ' + error);
      throw error;
    }
  }

  // Cleanup
  destroy(): void {
    this.stop();

    this.eqFilters.forEach(filter => filter.disconnect());
    this.eqFilters = [];

    if (this.gainNode) {
      this.gainNode.disconnect();
    }

    if (this.analyserNode) {
      this.analyserNode.disconnect();
    }

    if (this.audioContext && this.audioContext.state !== 'closed') {
      this.audioContext.close();
    }

    this.audioContext = null;
    this.audioBuffer = null;
    this.gainNode = null;
    this.analyserNode = null;
  }
}
