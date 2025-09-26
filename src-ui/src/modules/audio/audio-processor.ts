// Audio processing and Web Audio API functionality

export interface CaptureResult {
  frequencies: number[];
  magnitudes: number[];
  success: boolean;
  error?: string;
}

export class AudioProcessor {
  private audioContext: AudioContext | null = null;
  private audioBuffer: AudioBuffer | null = null;
  private audioSource: AudioBufferSourceNode | null = null;
  private eqFilters: BiquadFilterNode[] = [];
  private gainNode: GainNode | null = null;
  private isAudioPlaying: boolean = false;
  private audioStartTime: number = 0;
  private audioAnimationFrame: number | null = null;
  private currentFilterParams: number[] = [];
  private originalFilterParams: number[] = [];
  private eqEnabled: boolean = true;

  // Frequency analyzer
  private analyserNode: AnalyserNode | null = null;
  private spectrumCanvas: HTMLCanvasElement | null = null;
  private spectrumCtx: CanvasRenderingContext2D | null = null;
  private spectrumAnimationFrame: number | null = null;
  private frequencyBinRanges: { start: number; end: number }[] = [];

  // Audio capture state
  private mediaStream: MediaStream | null = null;
  private mediaStreamSource: MediaStreamAudioSourceNode | null = null;
  private capturing: boolean = false;
  private captureController: AbortController | null = null;
  private captureAnalyser: AnalyserNode | null = null;

  // UI elements for audio status
  private audioStatusElements: {
    status?: HTMLElement;
    statusText?: HTMLElement;
    duration?: HTMLElement;
    position?: HTMLElement;
    progressFill?: HTMLElement;
  } = {};

  constructor() {
    this.setupAudioContext();
  }

  private setupAudioContext(): void {
    try {
      this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
      this.gainNode = this.audioContext.createGain();
      this.analyserNode = this.audioContext.createAnalyser();

      if (this.analyserNode) {
        this.analyserNode.fftSize = 2048;
        this.analyserNode.smoothingTimeConstant = 0.8;
      }
    } catch (error) {
      console.error('Failed to initialize audio context:', error);
    }
  }

  async loadAudioFile(file: File): Promise<void> {
    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    try {
      const arrayBuffer = await file.arrayBuffer();
      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log('Audio file loaded successfully');
    } catch (error) {
      console.error('Error loading audio file:', error);
      throw error;
    }
  }

  async loadAudioFromUrl(url: string): Promise<void> {
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
      console.log('Audio loaded from URL successfully:', {
        duration: this.audioBuffer.duration,
        sampleRate: this.audioBuffer.sampleRate,
        channels: this.audioBuffer.numberOfChannels
      });

      // Update audio status elements if available
      this.updateAudioStatus();
    } catch (error) {
      console.error('Error loading audio from URL:', error);
      throw error;
    }
  }

  updateFilterParams(filterParams: number[]): void {
    this.currentFilterParams = [...filterParams];
    this.setupEQFilters();
  }

  private setupEQFilters(): void {
    if (!this.audioContext || !this.gainNode) return;

    // Clear existing filters
    this.eqFilters.forEach(filter => filter.disconnect());
    this.eqFilters = [];

    // Create new filters from parameters
    for (let i = 0; i < this.currentFilterParams.length; i += 3) {
      if (i + 2 < this.currentFilterParams.length) {
        const freq = this.currentFilterParams[i];
        const q = this.currentFilterParams[i + 1];
        const gain = this.currentFilterParams[i + 2];

        if (Math.abs(gain) > 0.1) { // Only create filter if gain is significant
          const filter = this.audioContext.createBiquadFilter();
          filter.type = 'peaking';
          filter.frequency.value = freq;
          filter.Q.value = q;
          filter.gain.value = gain;
          this.eqFilters.push(filter);
        }
      }
    }

    console.log(`Created ${this.eqFilters.length} EQ filters`);
  }

  private connectAudioChain(): void {
    if (!this.audioSource || !this.gainNode || !this.audioContext) {
      console.error('Cannot connect audio chain - missing components:', {
        audioSource: !!this.audioSource,
        gainNode: !!this.gainNode,
        audioContext: !!this.audioContext
      });
      return;
    }

    console.log('Connecting audio chain with', this.eqFilters.length, 'EQ filters');
    let currentNode: AudioNode = this.audioSource;

    // Connect EQ filters in series
    this.eqFilters.forEach((filter, index) => {
      console.log(`Connecting EQ filter ${index + 1}`);
      currentNode.connect(filter);
      currentNode = filter;
    });

    // Connect to gain and analyzer
    console.log('Connecting to gain node and destination');
    currentNode.connect(this.gainNode);
    if (this.analyserNode) {
      this.gainNode.connect(this.analyserNode);
      this.analyserNode.connect(this.audioContext.destination);
      console.log('Audio chain connected: source -> EQ filters -> gain -> analyser -> destination');
    } else {
      this.gainNode.connect(this.audioContext.destination);
      console.log('Audio chain connected: source -> EQ filters -> gain -> destination');
    }
  }


  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    if (!this.gainNode) return;

    if (enabled && this.currentFilterParams.length > 0) {
      this.setupEQFilters();
    } else {
      // Disconnect all EQ filters but keep the audio chain connected
      this.eqFilters.forEach(filter => filter.disconnect());
      this.eqFilters = [];

      // Reconnect audio chain without EQ filters
      if (this.audioSource && this.isAudioPlaying) {
        this.connectAudioChain();
      }
    }

    console.log(`EQ ${enabled ? 'enabled' : 'disabled'}`);
    this.updateAudioStatus();
  }

  getCurrentTime(): number {
    if (!this.audioContext || !this.isAudioPlaying) return 0;
    return this.audioContext.currentTime - this.audioStartTime;
  }

  getDuration(): number {
    return this.audioBuffer ? this.audioBuffer.duration : 0;
  }

  isPlaying(): boolean {
    return this.isAudioPlaying;
  }

  isCapturing(): boolean {
    return this.capturing;
  }

  setupSpectrumAnalyzer(canvas: HTMLCanvasElement): void {
    this.spectrumCanvas = canvas;
    this.spectrumCtx = canvas.getContext('2d');
    this.calculateFrequencyBinRanges();
  }

  private calculateFrequencyBinRanges(): void {
    if (!this.analyserNode || !this.audioContext) return;

    const nyquist = this.audioContext.sampleRate / 2;
    const binCount = this.analyserNode.frequencyBinCount;

    this.frequencyBinRanges = [];
    for (let i = 0; i < binCount; i++) {
      const freq = (i * nyquist) / binCount;
      this.frequencyBinRanges.push({
        start: freq,
        end: ((i + 1) * nyquist) / binCount
      });
    }
  }

  startSpectrumAnalysis(): void {
    if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);

    const draw = () => {
      if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

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

  stopSpectrumAnalysis(): void {
    if (this.spectrumAnimationFrame) {
      cancelAnimationFrame(this.spectrumAnimationFrame);
      this.spectrumAnimationFrame = null;
    }
  }

  // Audio capture functionality
  async startCapture(): Promise<CaptureResult> {
    console.log('Starting audio capture...');

    if (this.capturing) {
      throw new Error('Capture already in progress');
    }

    try {
      // Check if capture is supported
      if (!this.isCaptureSupported()) {
        console.warn('Microphone capture not supported, using simulated data');
        return this.simulateCapture();
      }

      // Request microphone access
      this.mediaStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
          sampleRate: 48000
        }
      });

      if (!this.audioContext) {
        throw new Error('Audio context not initialized');
      }

      // Create media stream source
      this.mediaStreamSource = this.audioContext.createMediaStreamSource(this.mediaStream);

      // Create analyzer for capture
      this.captureAnalyser = this.audioContext.createAnalyser();
      this.captureAnalyser.fftSize = 8192;
      this.captureAnalyser.smoothingTimeConstant = 0.1;

      // Connect stream to analyzer
      this.mediaStreamSource.connect(this.captureAnalyser);

      this.capturing = true;
      this.captureController = new AbortController();

      // Perform the actual capture measurement
      const result = await this.performCaptureMeasurement();

      return result;

    } catch (error) {
      console.error('Error during audio capture:', error);
      this.stopCapture();
      return {
        frequencies: [],
        magnitudes: [],
        success: false,
        error: error instanceof Error ? error.message : 'Unknown capture error'
      };
    }
  }

  stopCapture(): void {
    console.log('Stopping audio capture...');

    this.capturing = false;

    if (this.captureController) {
      this.captureController.abort();
      this.captureController = null;
    }

    if (this.mediaStreamSource) {
      this.mediaStreamSource.disconnect();
      this.mediaStreamSource = null;
    }

    if (this.captureAnalyser) {
      this.captureAnalyser.disconnect();
      this.captureAnalyser = null;
    }

    if (this.mediaStream) {
      this.mediaStream.getTracks().forEach(track => track.stop());
      this.mediaStream = null;
    }
  }

  private async performCaptureMeasurement(): Promise<CaptureResult> {
    if (!this.captureAnalyser || !this.audioContext) {
      throw new Error('Capture not properly initialized');
    }

    const bufferLength = this.captureAnalyser.frequencyBinCount;
    const dataArray = new Float32Array(bufferLength);
    const sampleRate = this.audioContext.sampleRate;

    // Collect multiple samples for averaging
    const numSamples = 50;
    const samples: Float32Array[] = [];

    for (let i = 0; i < numSamples; i++) {
      if (this.captureController?.signal.aborted) {
        throw new Error('Capture cancelled');
      }

      this.captureAnalyser.getFloatFrequencyData(dataArray);
      samples.push(new Float32Array(dataArray));

      // Wait a bit between samples
      await new Promise(resolve => setTimeout(resolve, 20));
    }

    // Average the samples
    const averagedData = new Float32Array(bufferLength);
    for (let i = 0; i < bufferLength; i++) {
      let sum = 0;
      for (const sample of samples) {
        sum += sample[i];
      }
      averagedData[i] = sum / samples.length;
    }

    // Convert to frequency/magnitude pairs
    const frequencies: number[] = [];
    const magnitudes: number[] = [];

    for (let i = 1; i < bufferLength; i++) { // Skip DC component
      const frequency = (i * sampleRate) / (2 * bufferLength);
      if (frequency >= 20 && frequency <= 20000) { // Audio range
        frequencies.push(frequency);
        magnitudes.push(averagedData[i]);
      }
    }

    console.log(`Captured ${frequencies.length} frequency points`);

    return {
      frequencies,
      magnitudes,
      success: true
    };
  }

  private simulateCapture(): CaptureResult {
    console.log('Simulating audio capture...');

    // Generate simulated frequency response data
    const frequencies: number[] = [];
    const magnitudes: number[] = [];

    // Generate logarithmically spaced frequencies from 20Hz to 20kHz
    const startFreq = Math.log10(20);
    const endFreq = Math.log10(20000);
    const numPoints = 200;

    for (let i = 0; i < numPoints; i++) {
      const logFreq = startFreq + (i / (numPoints - 1)) * (endFreq - startFreq);
      const freq = Math.pow(10, logFreq);
      frequencies.push(freq);

      // Generate a realistic-looking frequency response with some variation
      let magnitude = -20; // Base level in dB
      magnitude += 10 * Math.sin(Math.log10(freq) * 2); // Some variation
      magnitude += (Math.random() - 0.5) * 5; // Add some noise

      magnitudes.push(magnitude);
    }

    return {
      frequencies,
      magnitudes,
      success: true
    };
  }

  isCaptureSupported(): boolean {
    return !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia);
  }

  // Audio status and UI updates
  setupAudioStatusElements(elements: {
    status?: HTMLElement;
    statusText?: HTMLElement;
    duration?: HTMLElement;
    position?: HTMLElement;
    progressFill?: HTMLElement;
  }): void {
    this.audioStatusElements = elements;
    this.updateAudioStatus();
  }

  private updateAudioStatus(): void {
    if (this.audioStatusElements.statusText) {
      const status = this.isAudioPlaying ?
        (this.eqEnabled ? 'Playing (EQ On)' : 'Playing (EQ Off)') :
        'Stopped';
      this.audioStatusElements.statusText.textContent = status;
    }

    if (this.audioStatusElements.duration && this.audioBuffer) {
      const duration = this.audioBuffer.duration;
      this.audioStatusElements.duration.textContent = this.formatTime(duration);
    }

    // Start position updates if playing
    if (this.isAudioPlaying && !this.audioAnimationFrame) {
      this.startPositionUpdates();
    }
  }

  private startPositionUpdates(): void {
    const updatePosition = () => {
      if (!this.isAudioPlaying) {
        this.audioAnimationFrame = null;
        return;
      }

      const currentTime = this.getCurrentTime();
      const duration = this.getDuration();

      if (this.audioStatusElements.position) {
        this.audioStatusElements.position.textContent = this.formatTime(currentTime);
      }

      if (this.audioStatusElements.progressFill && duration > 0) {
        const progress = (currentTime / duration) * 100;
        this.audioStatusElements.progressFill.style.width = `${Math.min(progress, 100)}%`;
      }

      this.audioAnimationFrame = requestAnimationFrame(updatePosition);
    };

    updatePosition();
  }

  private formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  }

  // Enhanced play method with status updates
  async play(): Promise<void> {
    console.log('Play method called');

    if (!this.audioContext) {
      throw new Error('Audio context not initialized');
    }

    if (!this.audioBuffer) {
      throw new Error('No audio loaded for playback');
    }

    console.log('Audio context state:', this.audioContext.state);

    // Resume audio context if suspended (required by browser autoplay policies)
    if (this.audioContext.state === 'suspended') {
      console.log('Resuming suspended audio context...');
      await this.audioContext.resume();
      console.log('Audio context resumed, new state:', this.audioContext.state);
    }

    this.stop(); // Stop any currently playing audio

    try {
      this.audioSource = this.audioContext.createBufferSource();
      this.audioSource.buffer = this.audioBuffer;

      console.log('Audio source created, connecting audio chain...');
      this.connectAudioChain();

      console.log('Starting audio playback...');
      this.audioSource.start();
      this.audioStartTime = this.audioContext.currentTime;
      this.isAudioPlaying = true;

      this.audioSource.onended = () => {
        console.log('Audio playback ended');
        this.isAudioPlaying = false;
        this.audioSource = null;
        this.updateAudioStatus();
        if (this.audioAnimationFrame) {
          cancelAnimationFrame(this.audioAnimationFrame);
          this.audioAnimationFrame = null;
        }
      };

      this.updateAudioStatus();
      console.log('Audio playback started successfully');
    } catch (error) {
      console.error('Error during audio playback:', error);
      throw error;
    }
  }

  // Enhanced stop method with status updates
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

    this.updateAudioStatus();
    console.log('Audio playback stopped');
  }

  destroy(): void {
    this.stop();
    this.stopCapture();
    this.stopSpectrumAnalysis();

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
