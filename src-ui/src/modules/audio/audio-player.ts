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
  enabled: boolean;
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
  private isAudioPaused: boolean = false;
  private audioStartTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentFilterParams: FilterParam[] = [
    { frequency: 100, q: 1.0, gain: 0, enabled: true },
    { frequency: 1000, q: 1.0, gain: 0, enabled: true },
    { frequency: 10000, q: 1.0, gain: 0, enabled: true }
  ];
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
  private pauseBtn: HTMLButtonElement | null = null;
  private stopBtn: HTMLButtonElement | null = null;
  private eqOnBtn: HTMLButtonElement | null = null;
  private eqOffBtn: HTMLButtonElement | null = null;
  private eqConfigBtn: HTMLButtonElement | null = null;
  private eqModal: HTMLElement | null = null;
  private eqBackdrop: HTMLElement | null = null;
  private eqModalCloseBtn: HTMLButtonElement | null = null;
  private eqTableContainer: HTMLElement | null = null;
  private statusText: HTMLElement | null = null;
  private positionText: HTMLElement | null = null;
  private durationText: HTMLElement | null = null;
  private progressFill: HTMLElement | null = null;

  // Configuration
  private config: AudioPlayerConfig;
  private callbacks: AudioPlayerCallbacks;
  private instanceId: string;

  // Pause double-click tracking
  private pauseClickCount: number = 0;
  private pauseClickTimer: number | null = null;

  // Resize handler reference for cleanup
  private resizeHandler: (() => void) | null = null;

  constructor(container: HTMLElement, config: AudioPlayerConfig = {}, callbacks: AudioPlayerCallbacks = {}) {
    if (!container) {
      throw new Error('AudioPlayer: container element is required but was null/undefined');
    }
    this.container = container;
    this.instanceId = 'audio-player-' + Math.random().toString(36).substr(2, 9);
    this.config = {
      enableEQ: true,
      maxFilters: 10,
      enableSpectrum: true,
      fftSize: 4096,
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

  private _createEQModal(): void {
    console.log('[EQ Debug] Creating modal element');
    const existingModal = document.getElementById(this.instanceId + '-eq-modal');
    if (existingModal) {
      console.log('[EQ Debug] Modal already exists:', existingModal);
      return;
    }

    // Create backdrop
    const backdrop = document.createElement('div');
    backdrop.id = this.instanceId + '-eq-backdrop';
    backdrop.className = 'eq-modal-backdrop';

    // Create modal
    const modal = document.createElement('div');
    modal.id = this.instanceId + '-eq-modal';
    modal.className = 'eq-modal';
    console.log('[EQ Debug] Modal element created:', modal);
    console.log('[EQ Debug] Modal ID:', modal.id);
    modal.innerHTML = `
      <div class="eq-modal-content">
        <div class="eq-modal-header">
          <h3>Equalizer Configuration</h3>
          <button type="button" class="eq-modal-close-btn">&times;</button>
        </div>
        <div class="eq-modal-body">
          <div class="eq-table-container"></div>
        </div>
      </div>
    `;

    // Append both to body for proper layering
    document.body.appendChild(backdrop);
    document.body.appendChild(modal);
    console.log('[EQ Debug] Modal and backdrop inserted into body');
    console.log('[EQ Debug] Modal in DOM:', document.contains(modal));
    const foundModal = document.getElementById(this.instanceId + '-eq-modal');
    console.log('[EQ Debug] Can find modal after insertion:', !!foundModal);
  }

  private async init(): Promise<void> {
    try {
      await this.setupAudioContext();
      this._createEQModal();
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
    console.log('[EQ Debug] Creating UI with config:', {
      enableEQ: this.config.enableEQ,
      enableSpectrum: this.config.enableSpectrum,
      showProgress: this.config.showProgress
    });

    const html = `
      <div class="audio-player">
        <div class="audio-control-row">
          <div class="audio-left-controls">
            <div class="demo-track-container">
              <label for="${selectId}" class="demo-track-label">Demo Track</label>
              <select id="${selectId}" class="demo-audio-select">
                <option value="">Select track...</option>
                ${Object.keys(this.config.demoTracks || {}).map(key =>
                  `<option value="${key}">${this.formatTrackName(key)}</option>`
                ).join('')}
              </select>
            </div>
          </div>

          <div class="audio-center-controls">
            <div class="audio-playback-container">
              ${this.config.showProgress ? `
                <div class="audio-status" style="display: flex;">
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

              <div class="audio-playback-controls">
                <button type="button" class="listen-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M8 5V19L19 12L8 5Z"/></svg>
                </button>
                <button type="button" class="pause-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M6 4H10V20H6V4ZM14 4H18V20H14V4Z"/></svg>
                </button>
                <button type="button" class="stop-button" disabled>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><path d="M6 6H18V18H6V6Z"/></svg>
                </button>
              </div>
            </div>
          </div>

          <div class="audio-right-controls">
            ${this.config.enableSpectrum ? `
              <div class="frequency-analyzer" style="display: flex;">
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
            ${this.config.enableEQ ? `
              <div class="eq-toggle-buttons">
                <button type="button" class="eq-toggle-btn eq-on-btn active">On</button>
                <button type="button" class="eq-toggle-btn eq-config-btn">⚙️</button>
                <button type="button" class="eq-toggle-btn eq-off-btn">Off</button>
              </div>
            ` : ''}
          </div>
        </div>
      </div>
    `;

    console.log('[EQ Debug] Generated HTML contains gear button:', html.includes('eq-config-btn'));
    this.container.innerHTML = html;
    console.log('[EQ Debug] HTML injected into container');
    this.cacheUIElements();
  }

  private cacheUIElements(): void {
    console.log('[EQ Debug] Caching UI elements from container:', this.container);
    console.log('[EQ Debug] Container HTML:', this.container.innerHTML.substring(0, 500) + '...');

    this.demoSelect = this.container.querySelector('.demo-audio-select');
    this.listenBtn = this.container.querySelector('.listen-button');
    this.pauseBtn = this.container.querySelector('.pause-button');
    this.stopBtn = this.container.querySelector('.stop-button');
    this.eqOnBtn = this.container.querySelector('.eq-on-btn');
    this.eqOffBtn = this.container.querySelector('.eq-off-btn');
    this.eqConfigBtn = this.container.querySelector('.eq-config-btn');

    console.log('[EQ Debug] Elements found:', {
      demoSelect: !!this.demoSelect,
      listenBtn: !!this.listenBtn,
      eqOnBtn: !!this.eqOnBtn,
      eqOffBtn: !!this.eqOffBtn,
      eqConfigBtn: !!this.eqConfigBtn
    });

    console.log('[EQ Debug] Gear button element:', this.eqConfigBtn);
    console.log('[EQ Debug] Gear button found:', !!this.eqConfigBtn);

    // Check if EQ buttons container exists
    const eqButtonsContainer = this.container.querySelector('.eq-toggle-buttons');
    console.log('[EQ Debug] EQ buttons container found:', !!eqButtonsContainer);
    if (eqButtonsContainer) {
      console.log('[EQ Debug] EQ buttons container HTML:', eqButtonsContainer.innerHTML);
    }
    this.statusText = this.container.querySelector('.audio-status-text');
    this.positionText = this.container.querySelector('.audio-position');
    this.durationText = this.container.querySelector('.audio-duration');
    this.progressFill = this.container.querySelector('.audio-progress-fill');
    this.spectrumCanvas = this.container.querySelector('.spectrum-canvas');

    // Modal and backdrop elements are in the body
    this.eqModal = document.getElementById(this.instanceId + '-eq-modal');
    this.eqBackdrop = document.getElementById(this.instanceId + '-eq-backdrop');
    console.log('[EQ Debug] Modal element lookup ID:', this.instanceId + '-eq-modal');
    console.log('[EQ Debug] Modal element found:', this.eqModal);
    console.log('[EQ Debug] Backdrop element found:', this.eqBackdrop);
    if (this.eqModal) {
        this.eqModalCloseBtn = this.eqModal.querySelector('.eq-modal-close-btn');
        this.eqTableContainer = this.eqModal.querySelector('.eq-table-container');
    }

    if (this.spectrumCanvas) {
      this.spectrumCtx = this.spectrumCanvas.getContext('2d');
      // Set canvas dimensions
      this.resizeSpectrumCanvas();
      // Initialize spectrum analyzer immediately if enabled
      if (this.config.enableSpectrum) {
        this.initializeSpectrumDisplay();
      }
    }
  }

  private setupEventListeners(): void {
    // Handle window resize for spectrum canvas
    this.resizeHandler = () => {
      if (this.spectrumCanvas && this.config.enableSpectrum) {
        this.resizeSpectrumCanvas();
      }
    };
    window.addEventListener('resize', this.resizeHandler);

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
    this.listenBtn?.addEventListener('click', () => {
        // If truly paused, resume; otherwise, play from beginning
        if (this.isAudioPaused) {
            this.resume();
        } else {
            this.play();
        }
    });

    this.pauseBtn?.addEventListener('click', () => {
        this.handlePauseClick();
    });

    this.stopBtn?.addEventListener('click', () => {
        this.stop();
    });

    // EQ controls
    this.eqOnBtn?.addEventListener('click', () => this.setEQEnabled(true));
    this.eqOffBtn?.addEventListener('click', () => this.setEQEnabled(false));
    if (this.eqConfigBtn) {
      console.log('[EQ Debug] Adding click event listener to gear button');
      this.eqConfigBtn.addEventListener('click', () => {
        console.log('[EQ Debug] Gear button clicked - event triggered');
        try {
          console.log('[EQ Debug] Executing modal show logic');
          this.openEQModal();
        } catch (error) {
          console.error('[EQ Debug] Error in click handler:', error);
        }
      });
      console.log('[EQ Debug] Click event listener attached to gear button');
    } else {
      console.error('[EQ Debug] Gear button not found, cannot attach event listener');
    }
    this.eqModalCloseBtn?.addEventListener('click', () => this.closeEQModal());
    this.eqBackdrop?.addEventListener('click', () => this.closeEQModal());
  }

  private openEQModal(): void {
    console.log('[EQ Debug] Attempting to show modal');
    console.log('[EQ Debug] Current modal state:', {
      exists: !!this.eqModal,
      backdropExists: !!this.eqBackdrop,
      id: this.eqModal?.id,
      className: this.eqModal?.className,
      parentElement: this.eqModal?.parentElement?.tagName
    });

    if (this.eqModal && this.eqBackdrop && this.eqConfigBtn) {
      this.renderEQTable();

      // Position the modal above the gear button
      const buttonRect = this.eqConfigBtn.getBoundingClientRect();
      const modalWidth = 450; // Match CSS width
      const modalHeight = 350; // Approximate height

      // Calculate position - center above the button
      let left = buttonRect.left + (buttonRect.width / 2) - (modalWidth / 2);
      let top = buttonRect.top - modalHeight - 10; // 10px gap

      // Keep modal within viewport
      const padding = 10;
      if (left < padding) left = padding;
      if (left + modalWidth > window.innerWidth - padding) {
        left = window.innerWidth - modalWidth - padding;
      }

      // If not enough space above, show below
      if (top < padding) {
        top = buttonRect.bottom + 10;
      }

      // Apply positioning
      this.eqModal.style.left = `${left}px`;
      this.eqModal.style.top = `${top}px`;

      console.log('[EQ Debug] Modal positioned at:', { left, top, buttonRect });

      // Show backdrop and modal
      this.eqBackdrop.classList.add('visible');
      this.eqModal.classList.add('visible');

      console.log('[EQ Debug] Modal classes after show:', {
        modal: this.eqModal.className,
        backdrop: this.eqBackdrop.className
      });

      // Add click outside handler
      document.addEventListener('mousedown', this.handleClickOutside, true);
    } else {
      console.error('[EQ Debug] Modal, backdrop, or gear button element is null or undefined');
    }
  }

  private closeEQModal(): void {
    if (this.eqModal) {
      this.eqModal.classList.remove('visible');
    }
    if (this.eqBackdrop) {
      this.eqBackdrop.classList.remove('visible');
    }
    document.removeEventListener('mousedown', this.handleClickOutside, true);
  }

  private handleClickOutside = (event: MouseEvent): void => {
    if (this.eqModal && !this.eqModal.contains(event.target as Node) && !this.eqConfigBtn?.contains(event.target as Node)) {
      this.closeEQModal();
    }
  };

  private renderEQTable(): void {
    console.log('[EQ Debug] Rendering EQ table');
    console.log('[EQ Debug] EQ table container:', this.eqTableContainer);
    console.log('[EQ Debug] Current filter params:', this.currentFilterParams);
    if (!this.eqTableContainer) {
      console.error('[EQ Debug] EQ table container not found');
      return;
    }

    const table = document.createElement('table');
    table.innerHTML = `
      <thead>
        <tr>
          <th>Enabled</th>
          <th>Frequency (Hz)</th>
          <th>Q</th>
          <th>Gain (dB)</th>
        </tr>
      </thead>
      <tbody>
        ${this.currentFilterParams.map((filter, index) => `
          <tr>
            <td><input type="checkbox" data-index="${index}" class="eq-enabled" ${filter.enabled ? 'checked' : ''}></td>
            <td><input type="number" data-index="${index}" class="eq-frequency" value="${filter.frequency.toFixed(1)}" step="1"></td>
            <td><input type="number" data-index="${index}" class="eq-q" value="${filter.q.toFixed(2)}" step="0.1"></td>
            <td><input type="number" data-index="${index}" class="eq-gain" value="${filter.gain.toFixed(2)}" step="0.1"></td>
          </tr>
        `).join('')}
      </tbody>
    `;

    this.eqTableContainer.innerHTML = '';
    this.eqTableContainer.appendChild(table);

    table.addEventListener('input', (e) => this.handleEQTableChange(e));
  }

  private handleEQTableChange(e: Event): void {
    const target = e.target as HTMLInputElement;
    const index = parseInt(target.dataset.index || '0', 10);
    const type = target.className.replace('eq-', '');

    if (isNaN(index) || !this.currentFilterParams[index]) return;

    let value: number | boolean;
    if (target.type === 'checkbox') {
      value = target.checked;
    } else {
      value = parseFloat(target.value);
      if (isNaN(value)) return;
    }

    (this.currentFilterParams[index] as any)[type] = value;

    this.updateFilterParams(this.currentFilterParams);

    // If playing, the change is applied immediately because setupEQFilters is called
    // and the audio graph is reconnected.
    if (this.isAudioPlaying) {
        this.setupEQFilters();
        this.connectAudioChain();
    }
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
    this.stop(); // Stop any currently playing audio

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

      if (this.config.enableSpectrum) {
        this.startSpectrumAnalysis();
      }
    } catch (error) {
      console.error('Error loading audio from URL:', error);
      throw error;
    }
  }

  private clearAudio(): void {
    this.stop();
    this.stopSpectrumAnalysis();
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
    // Progress bar is always visible now, so we don't hide it
    // This method is kept for backward compatibility but does nothing
    const audioStatus = this.container.querySelector('.audio-status') as HTMLElement;
    if (audioStatus && this.config.showProgress) {
      audioStatus.style.display = 'flex'; // Always show if progress is enabled
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
  updateFilterParams(filterParams: Partial<FilterParam>[]): void {
    this.currentFilterParams = filterParams.map(p => ({ ...p, frequency: p.frequency || 0, q: p.q || 1, gain: p.gain || 0, enabled: p.enabled ?? true }));
    this.setupEQFilters();
  }

  private setupEQFilters(): void {
    if (!this.audioContext || !this.gainNode) return;

    // Clear existing filters
    this.eqFilters.forEach(filter => filter.disconnect());
    this.eqFilters = [];

    // Calculate maximum positive gain for compensation
    let maxPositiveGain = 0;

    // Create new filters from parameters
    this.currentFilterParams.forEach(param => {
      if (param.enabled && Math.abs(param.gain) > 0.1) { // Only create filter if enabled and gain is significant
        const filter = this.audioContext!.createBiquadFilter();
        filter.type = 'peaking';
        filter.frequency.value = param.frequency;
        filter.Q.value = param.q;
        filter.gain.value = param.gain;
        this.eqFilters.push(filter);

        // Track maximum positive gain
        if (param.gain > maxPositiveGain) {
          maxPositiveGain = param.gain;
        }
      }
    });

    // Apply gain compensation to prevent clipping
    if (maxPositiveGain > 0) {
      const compensationGain = Math.pow(10, -maxPositiveGain / 20); // Convert dB to linear scale
      this.gainNode.gain.value = compensationGain;
      console.log(`Applied gain compensation: -${maxPositiveGain.toFixed(1)} dB (${compensationGain.toFixed(3)} linear)`);
    } else {
      this.gainNode.gain.value = 1.0; // No compensation needed
    }

    console.log(`Created ${this.eqFilters.length} EQ filters with gain compensation`);
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

  // Resize spectrum canvas to fit container
  private resizeSpectrumCanvas(): void {
    if (!this.spectrumCanvas) return;

    const container = this.spectrumCanvas.parentElement;
    if (!container) return;

    // Get the actual width of the container
    const rect = container.getBoundingClientRect();
    const width = Math.max(rect.width || container.clientWidth || 400, 200);
    const height = 52; // Fixed height matching CSS

    // Set canvas dimensions
    this.spectrumCanvas.width = width;
    this.spectrumCanvas.height = height;

    // Redraw idle spectrum if not playing
    if (!this.isAudioPlaying && this.config.enableSpectrum) {
      this.drawIdleSpectrum();
    }
  }

  // Initialize spectrum display even when not playing
  private initializeSpectrumDisplay(): void {
    if (!this.spectrumCanvas || !this.spectrumCtx) return;

    const frequencyAnalyzer = this.container.querySelector('.frequency-analyzer') as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = 'flex';
    }

    // Draw initial empty spectrum
    this.drawIdleSpectrum();
  }

  private drawIdleSpectrum(): void {
    if (!this.spectrumCanvas || !this.spectrumCtx) return;

    const width = this.spectrumCanvas.width;
    const height = this.spectrumCanvas.height;

    // Detect color scheme
    const isDarkMode = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;

    // Clear canvas with theme-appropriate background
    this.spectrumCtx.fillStyle = isDarkMode ? 'rgb(0, 0, 0)' : 'rgb(255, 255, 255)';
    this.spectrumCtx.fillRect(0, 0, width, height);

    // Draw a subtle baseline to indicate the spectrum analyzer is ready
    const barsCount = Math.min(width / 2, 256);
    const barWidth = width / barsCount;

    for (let i = 0; i < barsCount; i++) {
      const baseHeight = 2; // Minimal height for idle state

      if (isDarkMode) {
        this.spectrumCtx.fillStyle = 'rgba(88, 101, 242, 0.3)'; // Subtle blue
      } else {
        this.spectrumCtx.fillStyle = 'rgba(0, 123, 255, 0.3)'; // Subtle blue
      }

      const x = i * barWidth;
      this.spectrumCtx.fillRect(x, height - baseHeight, barWidth - 1, baseHeight);
    }
  }

  // Spectrum Analyzer
  private startSpectrumAnalysis(): void {
    if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

    const frequencyAnalyzer = this.container.querySelector('.frequency-analyzer') as HTMLElement;
    if (frequencyAnalyzer) {
      frequencyAnalyzer.style.display = 'flex';
    }

    if (this.spectrumAnimationFrame) return; // Animation already running

    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);

    const draw = () => {
      if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) {
        this.spectrumAnimationFrame = null;
        return;
      }

      const width = this.spectrumCanvas.width;
      const height = this.spectrumCanvas.height;

      // Detect color scheme and set appropriate colors
      const isDarkMode = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;

      // Set background color based on theme
      this.spectrumCtx.fillStyle = isDarkMode ? 'rgb(0, 0, 0)' : 'rgb(255, 255, 255)';
      this.spectrumCtx.fillRect(0, 0, width, height);

      if (this.isAudioPlaying) {
        this.analyserNode.getByteFrequencyData(dataArray);

        // Use logarithmic frequency mapping (20Hz - 20kHz)
        const minFreq = 20;
        const maxFreq = 20000;
        const sampleRate = this.audioContext!.sampleRate;
        const nyquist = sampleRate / 2;
        const barsCount = Math.min(width / 2, 256); // Limit bars for performance
        const barWidth = width / barsCount;

        for (let i = 0; i < barsCount; i++) {
          // Calculate logarithmic frequency for this bar
          const logMin = Math.log10(minFreq);
          const logMax = Math.log10(maxFreq);
          const logFreq = logMin + (logMax - logMin) * (i / barsCount);
          const freq = Math.pow(10, logFreq);

          // Map frequency to FFT bin
          const binIndex = Math.round((freq / nyquist) * dataArray.length);
          const clampedBin = Math.min(binIndex, dataArray.length - 1);

          // Get magnitude and apply some smoothing by averaging nearby bins
          let magnitude = 0;
          const smoothingRange = Math.max(1, Math.floor(dataArray.length / barsCount / 2));
          let count = 0;

          for (let j = Math.max(0, clampedBin - smoothingRange);
               j <= Math.min(dataArray.length - 1, clampedBin + smoothingRange); j++) {
            magnitude += dataArray[j];
            count++;
          }
          magnitude = count > 0 ? magnitude / count : 0;

          const barHeight = (magnitude / 255) * height * 0.9; // Use 90% of height for better visuals

          // Use different colors based on theme and frequency
          if (isDarkMode) {
            // Dark mode: bright colors with frequency-based hues
            const hueShift = (i / barsCount) * 60; // 0-60 degrees (red to yellow)
            const intensity = Math.floor(barHeight / height * 155 + 100);
            this.spectrumCtx.fillStyle = `hsl(${hueShift}, 80%, ${Math.min(intensity / 255 * 70 + 30, 90)}%)`;
          } else {
            // Light mode: darker colors with frequency-based variation
            const hueShift = (i / barsCount) * 240; // 0-240 degrees (red to blue)
            const saturation = 70 + (barHeight / height) * 30; // 70-100%
            const lightness = Math.max(20, 60 - (barHeight / height) * 40); // 60-20%
            this.spectrumCtx.fillStyle = `hsl(${hueShift}, ${saturation}%, ${lightness}%)`;
          }

          const x = i * barWidth;
          this.spectrumCtx.fillRect(x, height - barHeight, barWidth - 1, barHeight);
        }
      } else {
        // When not playing, show idle spectrum
        this.drawIdleSpectrum();
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

    // Keep spectrum analyzer visible but show idle state
    if (this.config.enableSpectrum) {
      this.drawIdleSpectrum();
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

      this.callbacks.onPlay?.();
      console.log('Audio playback started successfully');
    } catch (error) {
      console.error('Error during audio playback:', error);
      this.callbacks.onError?.('Playback failed: ' + error);
      throw error;
    }
  }

  private handlePauseClick(): void {
    this.pauseClickCount++;

    if (this.pauseClickCount === 1) {
      // First click - pause
      this.pause();

      // Set a timer to reset click count
      this.pauseClickTimer = window.setTimeout(() => {
        this.pauseClickCount = 0;
        this.pauseClickTimer = null;
      }, 500); // 500ms window for double-click
    } else if (this.pauseClickCount === 2) {
      // Second click - restart
      if (this.pauseClickTimer) {
        clearTimeout(this.pauseClickTimer);
        this.pauseClickTimer = null;
      }
      this.pauseClickCount = 0;
      this.restart();
    }
  }

  pause(): void {
    if (this.audioContext && this.audioContext.state === 'running') {
      this.audioContext.suspend();
      this.isAudioPlaying = false;
      this.isAudioPaused = true;
      this.updatePlaybackUI();
      console.log('Audio playback paused');
    }
  }

  private restart(): void {
    console.log('Restarting audio playback');
    this.stop();
    // Small delay to ensure stop is complete
    setTimeout(() => {
      this.play();
    }, 50);
  }

  resume(): void {
    if (this.audioContext && this.audioContext.state === 'suspended' && this.isAudioPaused) {
      this.audioContext.resume();
      this.isAudioPlaying = true;
      this.isAudioPaused = false;
      this.updatePlaybackUI();
      console.log('Audio playback resumed');
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
    this.isAudioPaused = false;

    if (this.audioAnimationFrame) {
      cancelAnimationFrame(this.audioAnimationFrame);
      this.audioAnimationFrame = null;
    }

    this.updatePlaybackUI();
    this.callbacks.onStop?.();
    console.log('Audio playback stopped');
  }

  private updatePlaybackUI(): void {
    const isPlaying = this.isAudioPlaying;
    const isPaused = this.isAudioPaused;

    // Update button states based on playback status
    if (this.listenBtn) {
      this.listenBtn.disabled = isPlaying;
    }

    if (this.pauseBtn) {
      this.pauseBtn.disabled = !isPlaying;
    }

    if (this.stopBtn) {
      this.stopBtn.disabled = !isPlaying && !isPaused;
    }

    if (this.statusText) {
      let status = 'Audio ready';
      if (isPaused) {
        status = this.eqEnabled ? 'Paused (EQ On)' : 'Paused (EQ Off)';
      } else if (isPlaying) {
        status = this.eqEnabled ? 'Playing (EQ On)' : 'Playing (EQ Off)';
      }
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
    this.stop(); // Stop any currently playing audio

    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    try {
      const arrayBuffer = await file.arrayBuffer();
      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log('Audio file loaded successfully');

      this.updateAudioInfo();
      this.setListenButtonEnabled(true);
      this.setStatus('Audio ready');

      if (this.config.enableSpectrum) {
        this.startSpectrumAnalysis();
      }
    } catch (error) {
      console.error('Error loading audio file:', error);
      this.callbacks.onError?.('Failed to load audio file: ' + error);
      throw error;
    }
  }

  // Cleanup
  destroy(): void {
    this.stop();
    this.stopSpectrumAnalysis();

    // Remove window resize listener
    if (this.resizeHandler) {
      window.removeEventListener('resize', this.resizeHandler);
      this.resizeHandler = null;
    }

    // Remove modal and backdrop from DOM
    const modal = document.getElementById(this.instanceId + '-eq-modal');
    const backdrop = document.getElementById(this.instanceId + '-eq-backdrop');
    if (modal) modal.remove();
    if (backdrop) backdrop.remove();

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
