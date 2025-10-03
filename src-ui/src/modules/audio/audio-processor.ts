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
  private oscillator: OscillatorNode | null = null;
  private noiseSource: AudioBufferSourceNode | null = null;
  private noiseBuffer: AudioBuffer | null = null;
  private sweepDuration: number = 10; // seconds
  private outputChannel: "left" | "right" | "both" | "default" = "both";
  private captureSampleRate: number = 48000;
  private signalType: "sweep" | "white" | "pink" = "sweep";

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
      this.audioContext = new (window.AudioContext ||
        (window as any).webkitAudioContext)();
      this.gainNode = this.audioContext.createGain();
      this.analyserNode = this.audioContext.createAnalyser();

      if (this.analyserNode) {
        this.analyserNode.fftSize = 2048;
        this.analyserNode.smoothingTimeConstant = 0.8;
      }

      // Pre-generate noise buffers
      this.generateNoiseBuffers();
    } catch (error) {
      console.error("Failed to initialize audio context:", error);
    }
  }

  private generateNoiseBuffers(): void {
    if (!this.audioContext) return;

    // We'll generate noise buffers when needed with the correct sample rate
    // This is just a placeholder for the method
  }

  private createNoiseSource(type: "white" | "pink"): AudioBufferSourceNode {
    if (!this.audioContext) {
      throw new Error("Audio context not initialized");
    }

    const bufferSize = this.audioContext.sampleRate * this.sweepDuration;
    const buffer = this.audioContext.createBuffer(
      1,
      bufferSize,
      this.audioContext.sampleRate,
    );
    const output = buffer.getChannelData(0);

    if (type === "white") {
      // White noise: random values
      for (let i = 0; i < bufferSize; i++) {
        output[i] = Math.random() * 2 - 1;
      }
    } else if (type === "pink") {
      // Pink noise using Paul Kellet's algorithm
      let b0 = 0,
        b1 = 0,
        b2 = 0,
        b3 = 0,
        b4 = 0,
        b5 = 0,
        b6 = 0;
      for (let i = 0; i < bufferSize; i++) {
        const white = Math.random() * 2 - 1;
        b0 = 0.99886 * b0 + white * 0.0555179;
        b1 = 0.99332 * b1 + white * 0.0750759;
        b2 = 0.969 * b2 + white * 0.153852;
        b3 = 0.8665 * b3 + white * 0.3104856;
        b4 = 0.55 * b4 + white * 0.5329522;
        b5 = -0.7616 * b5 - white * 0.016898;
        output[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + white * 0.5362) * 0.11;
        b6 = white * 0.115926;
      }
    }

    this.noiseSource = this.audioContext.createBufferSource();
    this.noiseSource.buffer = buffer;
    this.noiseBuffer = buffer;

    return this.noiseSource;
  }

  async loadAudioFile(file: File): Promise<void> {
    if (!this.audioContext) {
      throw new Error("Audio context not initialized");
    }

    try {
      const arrayBuffer = await file.arrayBuffer();
      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log("Audio file loaded successfully");

      // Update audio status elements if available
      this.updateAudioStatus();
    } catch (error) {
      console.error("Error loading audio file:", error);
      throw error;
    }
  }

  async loadAudioFromUrl(url: string): Promise<void> {
    if (!this.audioContext) {
      throw new Error("Audio context not initialized");
    }

    try {
      console.log("Loading audio from URL:", url);
      const response = await fetch(url);

      if (!response.ok) {
        throw new Error(
          `Failed to fetch audio: ${response.status} ${response.statusText}`,
        );
      }

      const arrayBuffer = await response.arrayBuffer();
      console.log("Audio data fetched, decoding...");

      this.audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
      console.log("Audio loaded from URL successfully:", {
        duration: this.audioBuffer.duration,
        sampleRate: this.audioBuffer.sampleRate,
        channels: this.audioBuffer.numberOfChannels,
      });

      // Update audio status elements if available
      this.updateAudioStatus();
    } catch (error) {
      console.error("Error loading audio from URL:", error);
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
    this.eqFilters.forEach((filter) => filter.disconnect());
    this.eqFilters = [];

    // Create new filters from parameters
    for (let i = 0; i < this.currentFilterParams.length; i += 3) {
      if (i + 2 < this.currentFilterParams.length) {
        const freq = this.currentFilterParams[i];
        const q = this.currentFilterParams[i + 1];
        const gain = this.currentFilterParams[i + 2];

        if (Math.abs(gain) > 0.1) {
          // Only create filter if gain is significant
          const filter = this.audioContext.createBiquadFilter();
          filter.type = "peaking";
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
      console.error("Cannot connect audio chain - missing components:", {
        audioSource: !!this.audioSource,
        gainNode: !!this.gainNode,
        audioContext: !!this.audioContext,
      });
      return;
    }

    console.log(
      "Connecting audio chain with",
      this.eqFilters.length,
      "EQ filters",
    );
    let currentNode: AudioNode = this.audioSource;

    // Connect EQ filters in series
    this.eqFilters.forEach((filter, index) => {
      console.log(`Connecting EQ filter ${index + 1}`);
      currentNode.connect(filter);
      currentNode = filter;
    });

    // Connect to gain and analyzer
    console.log("Connecting to gain node and destination");
    currentNode.connect(this.gainNode);
    if (this.analyserNode) {
      this.gainNode.connect(this.analyserNode);
      this.analyserNode.connect(this.audioContext.destination);
      console.log(
        "Audio chain connected: source -> EQ filters -> gain -> analyser -> destination",
      );
    } else {
      this.gainNode.connect(this.audioContext.destination);
      console.log(
        "Audio chain connected: source -> EQ filters -> gain -> destination",
      );
    }
  }

  setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;

    if (!this.gainNode) return;

    if (enabled && this.currentFilterParams.length > 0) {
      this.setupEQFilters();
    } else {
      // Disconnect all EQ filters but keep the audio chain connected
      this.eqFilters.forEach((filter) => filter.disconnect());
      this.eqFilters = [];

      // Reconnect audio chain without EQ filters
      if (this.audioSource && this.isAudioPlaying) {
        this.connectAudioChain();
      }
    }

    console.log(`EQ ${enabled ? "enabled" : "disabled"}`);
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
    this.spectrumCtx = canvas.getContext("2d");
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
        end: ((i + 1) * nyquist) / binCount,
      });
    }
  }

  startSpectrumAnalysis(): void {
    if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx) return;

    const dataArray = new Uint8Array(this.analyserNode.frequencyBinCount);

    const draw = () => {
      if (!this.analyserNode || !this.spectrumCanvas || !this.spectrumCtx)
        return;

      this.analyserNode.getByteFrequencyData(dataArray);

      const width = this.spectrumCanvas.width;
      const height = this.spectrumCanvas.height;

      this.spectrumCtx.fillStyle = "rgb(0, 0, 0)";
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

  // Audio device enumeration
  async enumerateAudioDevices(): Promise<MediaDeviceInfo[]> {
    try {
      // Request permission first
      await navigator.mediaDevices
        .getUserMedia({ audio: true })
        .then((stream) => {
          // Immediately stop the stream after getting permission
          stream.getTracks().forEach((track) => track.stop());
        });

      // Now enumerate devices
      const devices = await navigator.mediaDevices.enumerateDevices();
      const audioInputs = devices.filter(
        (device) => device.kind === "audioinput",
      );
      console.log("Found audio input devices:", audioInputs);
      return audioInputs;
    } catch (error) {
      console.error("Error enumerating audio devices:", error);
      return [];
    }
  }

  // Audio capture functionality
  async startCapture(deviceId?: string): Promise<CaptureResult> {
    console.log("Starting audio capture with device:", deviceId || "default");
    console.log(
      "Sample rate:",
      this.captureSampleRate,
      "Signal type:",
      this.signalType,
    );

    if (this.capturing) {
      throw new Error("Capture already in progress");
    }

    try {
      // Check if capture is supported
      if (!this.isCaptureSupported()) {
        console.warn("Microphone capture not supported, using simulated data");
        return this.simulateCapture();
      }

      // Recreate audio context with desired sample rate if needed
      if (
        !this.audioContext ||
        this.audioContext.sampleRate !== this.captureSampleRate
      ) {
        if (this.audioContext) {
          this.audioContext.close();
        }
        this.audioContext = new (window.AudioContext ||
          (window as any).webkitAudioContext)({
          sampleRate: this.captureSampleRate,
        });
        console.log(
          "Created audio context with sample rate:",
          this.audioContext.sampleRate,
        );
      }

      // Request microphone access with specific device if provided
      const audioConstraints: MediaStreamConstraints = {
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
          sampleRate: this.captureSampleRate,
          ...(deviceId && deviceId !== "default"
            ? { deviceId: { exact: deviceId } }
            : {}),
        },
      };

      this.mediaStream =
        await navigator.mediaDevices.getUserMedia(audioConstraints);

      if (!this.audioContext) {
        throw new Error("Audio context not initialized");
      }

      // Create media stream source
      this.mediaStreamSource = this.audioContext.createMediaStreamSource(
        this.mediaStream,
      );

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
      console.error("Error during audio capture:", error);
      this.stopCapture();
      return {
        frequencies: [],
        magnitudes: [],
        success: false,
        error: error instanceof Error ? error.message : "Unknown capture error",
      };
    }
  }

  stopCapture(): void {
    console.log("Stopping audio capture...");

    this.capturing = false;

    if (this.oscillator) {
      try {
        this.oscillator.stop();
      } catch (e) {
        // Already stopped
      }
      this.oscillator = null;
    }

    if (this.noiseSource) {
      try {
        this.noiseSource.stop();
      } catch (e) {
        // Already stopped
      }
      this.noiseSource = null;
    }

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
      this.mediaStream.getTracks().forEach((track) => track.stop());
      this.mediaStream = null;
    }
  }

  private async performCaptureMeasurement(): Promise<CaptureResult> {
    if (!this.captureAnalyser || !this.audioContext) {
      throw new Error("Capture not properly initialized");
    }

    console.log(`Starting ${this.signalType} capture...`);

    const duration = this.sweepDuration;
    let sourceNode: AudioNode;

    if (this.signalType === "sweep") {
      // Play frequency sweep and record response
      const startFreq = 20;
      const endFreq = Math.min(20000, this.audioContext.sampleRate / 2.1); // Respect Nyquist

      // Create oscillator for sweep
      this.oscillator = this.audioContext.createOscillator();
      this.oscillator.type = "sine";

      // Set up exponential frequency sweep
      this.oscillator.frequency.setValueAtTime(
        startFreq,
        this.audioContext.currentTime,
      );
      this.oscillator.frequency.exponentialRampToValueAtTime(
        endFreq,
        this.audioContext.currentTime + duration,
      );

      sourceNode = this.oscillator;
    } else {
      // Generate and play noise
      sourceNode = this.createNoiseSource(this.signalType);
    }

    // Connect source to output with reduced volume and channel routing
    const gainNode = this.audioContext.createGain();
    gainNode.gain.value = 0.3; // Reduce volume to avoid feedback

    // Configure channel routing based on selection
    if (
      this.outputChannel === "left" ||
      this.outputChannel === "right" ||
      this.outputChannel === "both"
    ) {
      // Use a ChannelMergerNode to control which channel gets the signal
      const merger = this.audioContext.createChannelMerger(2);

      if (this.outputChannel === "left") {
        // Connect to left channel only (input 0 of merger)
        sourceNode.connect(gainNode);
        gainNode.connect(merger, 0, 0);
      } else if (this.outputChannel === "right") {
        // Connect to right channel only (input 1 of merger)
        sourceNode.connect(gainNode);
        gainNode.connect(merger, 0, 1);
      } else if (this.outputChannel === "both") {
        // Connect to both channels
        const splitter = this.audioContext.createChannelSplitter(2);
        sourceNode.connect(gainNode);
        gainNode.connect(splitter);
        splitter.connect(merger, 0, 0); // left to left
        splitter.connect(merger, 0, 1); // left to right (mono to stereo)
      }

      merger.connect(this.audioContext.destination);
    } else {
      // Default: connect directly
      sourceNode.connect(gainNode);
      gainNode.connect(this.audioContext.destination);
    }

    // Start the signal
    if (this.signalType === "sweep" && this.oscillator) {
      this.oscillator.start();
    } else if (this.noiseSource) {
      this.noiseSource.start();
    }

    // Collect frequency response data during sweep
    const frequencyResponses: Float32Array[] = [];
    const bufferLength = this.captureAnalyser.frequencyBinCount;
    const dataArray = new Float32Array(bufferLength);
    const sampleRate = this.audioContext.sampleRate;
    const sampleInterval = 100; // ms between samples
    const numSamples = Math.floor((duration * 1000) / sampleInterval);

    for (let i = 0; i < numSamples; i++) {
      if (this.captureController?.signal.aborted) {
        this.oscillator?.stop();
        throw new Error("Capture cancelled");
      }

      // Get frequency data
      this.captureAnalyser.getFloatFrequencyData(dataArray);
      frequencyResponses.push(new Float32Array(dataArray));

      // Wait for next sample
      await new Promise((resolve) => setTimeout(resolve, sampleInterval));
    }

    // Stop the signal source
    if (this.oscillator) {
      this.oscillator.stop();
      this.oscillator = null;
    }
    if (this.noiseSource) {
      this.noiseSource.stop();
      this.noiseSource = null;
    }

    console.log(`Collected ${frequencyResponses.length} samples`);

    // Average the frequency responses
    const averagedData = new Float32Array(bufferLength);
    for (let i = 0; i < bufferLength; i++) {
      let sum = 0;
      let count = 0;
      for (const response of frequencyResponses) {
        if (!isNaN(response[i]) && isFinite(response[i])) {
          sum += response[i];
          count++;
        }
      }
      averagedData[i] = count > 0 ? sum / count : -100; // Default to -100 dB
    }

    // Apply 1/24 octave smoothing and resample to 200 points
    const result = this.smoothAndResample(averagedData, sampleRate);

    console.log(`Processed ${result.frequencies.length} frequency points`);

    return {
      frequencies: result.frequencies,
      magnitudes: result.magnitudes,
      success: true,
    };
  }

  private smoothAndResample(
    data: Float32Array,
    sampleRate: number,
  ): { frequencies: number[]; magnitudes: number[] } {
    // Create log-spaced frequency array (200 points from 20Hz to 20kHz)
    const frequencies: number[] = [];
    const magnitudes: number[] = [];
    const minFreq = 20;
    const maxFreq = 20000;
    const numPoints = 200;
    const smoothingOctaves = 24; // 1/24 octave smoothing

    const logMin = Math.log10(minFreq);
    const logMax = Math.log10(maxFreq);
    const logStep = (logMax - logMin) / (numPoints - 1);

    // Calculate bin frequencies
    const binCount = data.length;
    const nyquist = sampleRate / 2;
    const binFreqs: number[] = [];
    for (let i = 0; i < binCount; i++) {
      binFreqs.push((i / binCount) * nyquist);
    }

    // Generate target frequencies and apply smoothing
    for (let i = 0; i < numPoints; i++) {
      const logFreq = logMin + i * logStep;
      const targetFreq = Math.pow(10, logFreq);
      frequencies.push(targetFreq);

      // Calculate smoothing window
      const octaveWidth = 1.0 / smoothingOctaves;
      const lowerBound = targetFreq * Math.pow(2, -octaveWidth / 2);
      const upperBound = targetFreq * Math.pow(2, octaveWidth / 2);

      // Average values within the smoothing window
      let sum = 0;
      let count = 0;

      for (let j = 0; j < binCount; j++) {
        if (binFreqs[j] >= lowerBound && binFreqs[j] <= upperBound) {
          // Convert from dB to linear for averaging
          const linear = Math.pow(10, data[j] / 20);
          sum += linear;
          count++;
        }
      }

      if (count > 0) {
        // Convert average back to dB
        const avgLinear = sum / count;
        magnitudes.push(20 * Math.log10(avgLinear));
      } else {
        // No data in range, use nearest neighbor
        let nearestIdx = 0;
        let minDiff = Math.abs(binFreqs[0] - targetFreq);
        for (let j = 1; j < binCount; j++) {
          const diff = Math.abs(binFreqs[j] - targetFreq);
          if (diff < minDiff) {
            minDiff = diff;
            nearestIdx = j;
          }
        }
        magnitudes.push(data[nearestIdx]);
      }
    }

    return { frequencies, magnitudes };
  }

  private simulateCapture(): CaptureResult {
    console.log("Simulating audio capture...");

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
      success: true,
    };
  }

  setSweepDuration(duration: number): void {
    this.sweepDuration = duration;
  }

  setOutputChannel(channel: "left" | "right" | "both" | "default"): void {
    this.outputChannel = channel;
  }

  setSampleRate(rate: number): void {
    this.captureSampleRate = rate;
  }

  setSignalType(type: "sweep" | "white" | "pink"): void {
    this.signalType = type;
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
      const status = this.isAudioPlaying
        ? this.eqEnabled
          ? "Playing (EQ On)"
          : "Playing (EQ Off)"
        : "Stopped";
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
        this.audioStatusElements.position.textContent =
          this.formatTime(currentTime);
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
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  }

  // Enhanced play method with status updates
  async play(): Promise<void> {
    console.log("Play method called");

    if (!this.audioContext) {
      throw new Error("Audio context not initialized");
    }

    if (!this.audioBuffer) {
      throw new Error("No audio loaded for playback");
    }

    console.log("Audio context state:", this.audioContext.state);

    // Resume audio context if suspended (required by browser autoplay policies)
    if (this.audioContext.state === "suspended") {
      console.log("Resuming suspended audio context...");
      await this.audioContext.resume();
      console.log("Audio context resumed, new state:", this.audioContext.state);
    }

    this.stop(); // Stop any currently playing audio

    try {
      this.audioSource = this.audioContext.createBufferSource();
      this.audioSource.buffer = this.audioBuffer;

      console.log("Audio source created, connecting audio chain...");
      this.connectAudioChain();

      console.log("Starting audio playback...");
      this.audioSource.start();
      this.audioStartTime = this.audioContext.currentTime;
      this.isAudioPlaying = true;

      this.audioSource.onended = () => {
        console.log("Audio playback ended");
        this.isAudioPlaying = false;
        this.audioSource = null;
        this.updateAudioStatus();
        if (this.audioAnimationFrame) {
          cancelAnimationFrame(this.audioAnimationFrame);
          this.audioAnimationFrame = null;
        }
      };

      this.updateAudioStatus();
      console.log("Audio playback started successfully");
    } catch (error) {
      console.error("Error during audio playback:", error);
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
    console.log("Audio playback stopped");
  }

  destroy(): void {
    this.stop();
    this.stopCapture();
    this.stopSpectrumAnalysis();

    this.eqFilters.forEach((filter) => filter.disconnect());
    this.eqFilters = [];

    if (this.gainNode) {
      this.gainNode.disconnect();
    }

    if (this.analyserNode) {
      this.analyserNode.disconnect();
    }

    if (this.audioContext && this.audioContext.state !== "closed") {
      this.audioContext.close();
    }

    this.audioContext = null;
    this.audioBuffer = null;
    this.gainNode = null;
    this.analyserNode = null;
  }
}
