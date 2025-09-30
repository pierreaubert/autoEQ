// UI management and interaction functionality

import { OPTIMIZATION_DEFAULTS, OPTIMIZATION_LIMITS, OPTIMIZATION_STEPS } from './optimization-constants';

export class UIManager {
  private form!: HTMLFormElement;
  private optimizeBtn!: HTMLButtonElement;
  private resetBtn!: HTMLButtonElement;
  private progressElement!: HTMLElement;
  private errorElement!: HTMLElement;

  // Modal elements
  private optimizationModal!: HTMLElement;
  private progressStatus!: HTMLElement;
  private elapsedTimeElement!: HTMLElement;
  private progressTableBody!: HTMLElement;
  private cancelOptimizationBtn!: HTMLButtonElement;
  private doneOptimizationBtn!: HTMLButtonElement;
  private modalCloseBtn!: HTMLButtonElement;
  private progressGraphElement!: HTMLElement;

  // Timing
  private optimizationStartTime: number = 0;

  // Audio testing elements
  private demoAudioSelect!: HTMLSelectElement;
  private eqOnBtn!: HTMLButtonElement;
  private eqOffBtn!: HTMLButtonElement;
  private listenBtn!: HTMLButtonElement;
  private stopBtn!: HTMLButtonElement;
  private audioStatus!: HTMLElement;
  private audioStatusText!: HTMLElement;
  private audioDuration!: HTMLElement;
  private audioPosition!: HTMLElement;
  private audioProgressFill!: HTMLElement;

  // Capture elements
  private captureBtn: HTMLButtonElement | null = null;
  private captureStatus: HTMLElement | null = null;
  private captureStatusText: HTMLElement | null = null;
  private captureProgressFill: HTMLElement | null = null;
  private captureWaveform: HTMLCanvasElement | null = null;
  private captureWaveformCtx: CanvasRenderingContext2D | null = null;
  private captureResult: HTMLElement | null = null;
  private captureClearBtn: HTMLButtonElement | null = null;
  private capturePlot: HTMLElement | null = null;

  // State
  private eqEnabled: boolean = true;
  private isResizing: boolean = false;
  private startX: number = 0;
  private startWidth: number = 0;

  constructor() {
    this.initializeElements();
    this.setupEventListeners();
    this.setupUIInteractions();
    this.setupModalEventListeners();
    this.setupResizer();
  }

  private initializeElements(): void {
    this.form = document.getElementById('autoeq_form') as HTMLFormElement;
    this.optimizeBtn = document.getElementById('optimize_btn') as HTMLButtonElement;
    this.resetBtn = document.getElementById('reset_btn') as HTMLButtonElement;
    this.progressElement = document.getElementById('optimization_progress') as HTMLElement;
    // Scores are now always visible in the bottom row
    this.errorElement = document.getElementById('error_display') as HTMLElement;

    // Initialize modal elements
    this.optimizationModal = document.getElementById('optimization_modal') as HTMLElement;
    this.progressStatus = document.getElementById('progress_status') as HTMLElement;
    this.elapsedTimeElement = document.getElementById('elapsed_time') as HTMLElement;
    this.progressTableBody = document.getElementById('progress_table_body') as HTMLElement;

    // Debug element initialization
    console.log('[UI INIT] Modal elements found:');
    console.log('  optimizationModal:', !!this.optimizationModal);
    console.log('  progressStatus:', !!this.progressStatus);
    console.log('  elapsedTimeElement:', !!this.elapsedTimeElement);
    console.log('  progressTableBody:', !!this.progressTableBody);
    this.cancelOptimizationBtn = document.getElementById('cancel_optimization') as HTMLButtonElement;
    this.doneOptimizationBtn = document.getElementById('done_optimization') as HTMLButtonElement;
    this.modalCloseBtn = document.getElementById('modal_close') as HTMLButtonElement;
    this.progressGraphElement = document.getElementById('progress_graph') as HTMLElement;

    // Initialize audio elements
    this.demoAudioSelect = document.getElementById('demo_audio_select') as HTMLSelectElement;
    this.eqOnBtn = document.getElementById('eq_on_btn') as HTMLButtonElement;
    this.eqOffBtn = document.getElementById('eq_off_btn') as HTMLButtonElement;
    this.listenBtn = document.getElementById('listen_btn') as HTMLButtonElement;
    console.log('Listen button found:', this.listenBtn);
    console.log('Listen button initial state:', {
      id: this.listenBtn?.id,
      className: this.listenBtn?.className,
      disabled: this.listenBtn?.disabled,
      tagName: this.listenBtn?.tagName
    });

    // Check for duplicate elements
    const allListenButtons = document.querySelectorAll('#listen_btn');
    const allListenButtonsByClass = document.querySelectorAll('.listen-button');
    console.log('Total elements with ID listen_btn:', allListenButtons.length);
    console.log('Total elements with class listen-button:', allListenButtonsByClass.length);
    if (allListenButtons.length > 1) {
      console.warn('Multiple elements found with ID listen_btn!', allListenButtons);
    }

    // Add debugging to track what's disabling the button
    if (this.listenBtn) {
      const originalDisabledSetter = Object.getOwnPropertyDescriptor(HTMLButtonElement.prototype, 'disabled')?.set;
      if (originalDisabledSetter) {
        Object.defineProperty(this.listenBtn, 'disabled', {
          set: function(value: boolean) {
            console.log(`Listen button disabled property being set to: ${value}`);
            console.trace('Stack trace for disabled setter:');
            originalDisabledSetter.call(this, value);
          },
          get: function() {
            return this.hasAttribute('disabled');
          },
          configurable: true
        });
      }
    }
    this.stopBtn = document.getElementById('stop_btn') as HTMLButtonElement;
    this.audioStatus = document.getElementById('audio_status') as HTMLElement;
    this.audioStatusText = document.getElementById('audio_status_text') as HTMLElement;
    this.audioDuration = document.getElementById('audio_duration') as HTMLElement;
    this.audioPosition = document.getElementById('audio_position') as HTMLElement;
    this.audioProgressFill = document.getElementById('audio_progress_fill') as HTMLElement;

    // Capture elements
    this.captureBtn = document.getElementById('capture_btn') as HTMLButtonElement;
    this.captureStatus = document.getElementById('capture_status') as HTMLElement;
    this.captureStatusText = document.getElementById('capture_status_text') as HTMLElement;
    this.captureProgressFill = document.getElementById('capture_progress_fill') as HTMLElement;
    this.captureWaveform = document.getElementById('capture_waveform') as HTMLCanvasElement;
    this.captureWaveformCtx = this.captureWaveform ? this.captureWaveform.getContext('2d') : null;
    this.captureResult = document.getElementById('capture_result') as HTMLElement;
    this.captureClearBtn = document.getElementById('capture_clear') as HTMLButtonElement;
    this.capturePlot = document.getElementById('capture_plot') as HTMLElement;
  }

  private setupEventListeners(): void {
    // Form submission
    this.form.addEventListener('submit', (e) => {
      e.preventDefault();
      this.onOptimizeClick();
    });

    // Reset button
    this.resetBtn.addEventListener('click', () => {
      this.resetToDefaults();
    });

    // Capture button
    this.captureBtn?.addEventListener('click', async () => {
      await this.onCaptureClick();
    });

    // Clear capture button
    this.captureClearBtn?.addEventListener('click', () => {
      this.clearCaptureResults();
    });

    // Audio control buttons
    this.eqOnBtn?.addEventListener('click', () => this.setEQEnabled(true));
    this.eqOffBtn?.addEventListener('click', () => this.setEQEnabled(false));
    this.listenBtn?.addEventListener('click', () => this.onListenClick());
    this.stopBtn?.addEventListener('click', () => this.onStopClick());
  }

  private setupUIInteractions(): void {
    // Algorithm change handler
    const algoSelect = document.getElementById('algo') as HTMLSelectElement;
    if (algoSelect) {
      algoSelect.addEventListener('change', () => {
        this.updateConditionalParameters();
      });
    }

    // Input source change handler and tab switching
    const inputSourceRadios = document.querySelectorAll('input[name="input_source"]');
    inputSourceRadios.forEach(radio => {
      radio.addEventListener('change', (e) => {
        const target = e.target as HTMLInputElement;
        const value = target.value;

        // Update conditional parameters
        this.updateConditionalParameters();

        // Handle tab switching
        this.switchTab(value);
      });
    });

    // Tab label click handlers
    const tabLabels = document.querySelectorAll('.tab-label');
    tabLabels.forEach(label => {
      label.addEventListener('click', (e) => {
        const tabName = (e.currentTarget as HTMLElement).getAttribute('data-tab');
        if (tabName) {
          // Find and check the corresponding radio button
          const radio = document.querySelector(`input[name="input_source"][value="${tabName}"]`) as HTMLInputElement;
          if (radio) {
            radio.checked = true;
            this.switchTab(tabName);
            this.updateConditionalParameters();
          }
        }
      });
    });

    // Grid layout - accordion functionality removed
  }

  private setupModalEventListeners(): void {
    // Modal close handlers
    this.modalCloseBtn?.addEventListener('click', () => {
      this.closeOptimizationModal();
    });

    this.doneOptimizationBtn?.addEventListener('click', () => {
      this.closeOptimizationModal();
    });

    // Cancel optimization
    this.cancelOptimizationBtn?.addEventListener('click', () => {
      this.cancelOptimization();
    });

    // Close modal when clicking outside
    this.optimizationModal?.addEventListener('click', (e) => {
      if (e.target === this.optimizationModal) {
        this.closeOptimizationModal();
      }
    });
  }

  private setupResizer(): void {
    const resizer = document.getElementById('resizer');
    const leftPanel = document.getElementById('left_panel');

    if (!resizer || !leftPanel) return;

    resizer.addEventListener('mousedown', (e) => {
      this.isResizing = true;
      this.startX = e.clientX;
      this.startWidth = parseInt(document.defaultView?.getComputedStyle(leftPanel).width || '0', 10);
      document.addEventListener('mousemove', this.handleMouseMove);
      document.addEventListener('mouseup', this.handleMouseUp);
      e.preventDefault();
    });
  }

  private handleMouseMove = (e: MouseEvent) => {
    if (!this.isResizing) return;

    const leftPanel = document.getElementById('left_panel');
    if (!leftPanel) return;

    const dx = e.clientX - this.startX;
    const newWidth = this.startWidth + dx;
    const minWidth = 300;
    const maxWidth = window.innerWidth * 0.6;

    if (newWidth >= minWidth && newWidth <= maxWidth) {
      leftPanel.style.width = newWidth + 'px';
      // Update CSS custom property for bottom-left to match
      document.documentElement.style.setProperty('--left-panel-width', newWidth + 'px');
    }
  };

  private handleMouseUp = () => {
    this.isResizing = false;
    document.removeEventListener('mousemove', this.handleMouseMove);
    document.removeEventListener('mouseup', this.handleMouseUp);
  };

  showProgress(show: boolean): void {
    if (this.progressElement) {
      this.progressElement.style.display = show ? 'block' : 'none';
    }
  }

  updateStatus(message: string): void {
    console.log('Status:', message);
  }

  showError(error: string): void {
    const errorMessageElement = document.getElementById('error_message') as HTMLElement;
    if (errorMessageElement) {
      errorMessageElement.textContent = error;
    }
    if (this.errorElement) {
      this.errorElement.style.display = 'block';
    }
  }

  updateScores(before: number | null | undefined, after: number | null | undefined): void {
    const scoreBeforeElement = document.getElementById('score_before') as HTMLElement;
    const scoreAfterElement = document.getElementById('score_after') as HTMLElement;
    const scoreImprovementElement = document.getElementById('score_improvement') as HTMLElement;

    // Handle null/undefined values
    if (scoreBeforeElement) {
      scoreBeforeElement.textContent = before !== null && before !== undefined ? before.toFixed(3) : '-';
    }
    if (scoreAfterElement) {
      scoreAfterElement.textContent = after !== null && after !== undefined ? after.toFixed(3) : '-';
    }
    if (scoreImprovementElement) {
      if (before !== null && before !== undefined && after !== null && after !== undefined) {
        const improvement = after - before;
        scoreImprovementElement.textContent = (improvement >= 0 ? '+' : '') + improvement.toFixed(3);
      } else {
        scoreImprovementElement.textContent = '-';
      }
    }

    // Scores are now always visible in the bottom row
  }

  clearResults(): void {
    console.log('clearResults called');
    // Reset scores to default values instead of hiding
    const scoreBeforeElement = document.getElementById('score_before') as HTMLElement;
    const scoreAfterElement = document.getElementById('score_after') as HTMLElement;
    const scoreImprovementElement = document.getElementById('score_improvement') as HTMLElement;

    if (scoreBeforeElement) {
      scoreBeforeElement.textContent = '-';
    }
    if (scoreAfterElement) {
      scoreAfterElement.textContent = '-';
    }
    if (scoreImprovementElement) {
      scoreImprovementElement.textContent = '-';
    }

    if (this.errorElement) {
      this.errorElement.style.display = 'none';
    }
  }

  setOptimizationRunning(running: boolean): void {
    if (this.optimizeBtn) {
      this.optimizeBtn.disabled = running;
      this.optimizeBtn.textContent = running ? 'Optimizing...' : 'Run Optimization';
    }

    // Update modal buttons based on optimization state
    if (running) {
      this.showCancelButton();
      // Start the timer
      this.optimizationStartTime = Date.now();
      if (this.elapsedTimeElement) {
        this.elapsedTimeElement.textContent = '00:00';
      }
    } else {
      // Reset timer
      this.optimizationStartTime = 0;
    }
  }

  showCancelButton(): void {
    if (this.cancelOptimizationBtn && this.doneOptimizationBtn) {
      this.cancelOptimizationBtn.style.display = 'inline-block';
      this.doneOptimizationBtn.style.display = 'none';
    }
  }

  showCloseButton(): void {
    if (this.cancelOptimizationBtn && this.doneOptimizationBtn) {
      this.cancelOptimizationBtn.style.display = 'none';
      this.doneOptimizationBtn.style.display = 'inline-block';

      // Update button text and styling for close functionality
      this.doneOptimizationBtn.textContent = 'Close';
      this.doneOptimizationBtn.className = 'btn btn-primary'; // Blue button
    }

    // Update progress status to show completion
    if (this.progressStatus) {
      this.progressStatus.textContent = 'Optimization Complete';
    }
  }

  openOptimizationModal(): void {
    if (this.optimizationModal) {
      this.optimizationModal.style.display = 'flex';
      document.body.style.overflow = 'hidden';
    }
  }

  closeOptimizationModal(): void {
    if (this.optimizationModal) {
      this.optimizationModal.style.display = 'none';
      document.body.style.overflow = 'auto';
    }
  }

  updateProgress(stage: string, status: string, details: string, percentage: number): void {
    console.log(`[UI DEBUG] updateProgress called: stage="${stage}", status="${status}", details="${details}"`);

    if (this.progressStatus) {
      this.progressStatus.textContent = `${stage}: ${status}`;
      console.log(`[UI DEBUG] Updated progress status text to: "${stage}: ${status}"`);
    } else {
      console.warn('[UI DEBUG] progressStatus element not found!');
    }

    // Update elapsed time
    this.updateElapsedTime();
  }

  private updateElapsedTime(): void {
    if (this.optimizationStartTime > 0 && this.elapsedTimeElement) {
      const elapsedMs = Date.now() - this.optimizationStartTime;
      const elapsedSeconds = Math.floor(elapsedMs / 1000);
      const minutes = Math.floor(elapsedSeconds / 60);
      const seconds = elapsedSeconds % 60;
      const timeString = `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;

      this.elapsedTimeElement.textContent = timeString;
      console.log(`[UI DEBUG] Updated elapsed time to: ${timeString}`);
    }
  }

  // toggleAccordion method removed - using grid layout

  collapseAllAccordion(): void {
    // Grid layout - accordion functionality removed
    console.log('[UI] Accordion collapse functionality not needed in grid layout');
  }

  showAccordionSection(sectionId: string): void {
    // Grid layout - accordion functionality removed
    console.log(`[UI] Grid layout - section ${sectionId} visibility managed automatically`);
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

    console.log(`EQ ${enabled ? 'enabled' : 'disabled'}`);
  }

  resetToDefaults(): void {
    // Reset form to default values
    const form = this.form;
    if (form) {
      form.reset();

      // Set specific default values with null checks
      const setElementValue = (id: string, value: string | number | boolean, optional: boolean = false) => {
        const element = document.getElementById(id) as HTMLInputElement | HTMLSelectElement;
        if (element) {
          if (element.type === 'checkbox') {
            (element as HTMLInputElement).checked = Boolean(value);
          } else {
            element.value = String(value);
          }
          console.log(`Set ${id} = ${value}`);
        } else if (!optional) {
          console.warn(`Element with id '${id}' not found`);
        }
      };

      // Set input source radio button
      const inputSourceRadio = document.querySelector(`input[name="input_source"][value="${OPTIMIZATION_DEFAULTS.input_source}"]`) as HTMLInputElement;
      if (inputSourceRadio) {
        inputSourceRadio.checked = true;
      }

      // Core EQ parameters
      setElementValue('num_filters', OPTIMIZATION_DEFAULTS.num_filters);
      setElementValue('sample_rate', OPTIMIZATION_DEFAULTS.sample_rate);
      setElementValue('min_db', OPTIMIZATION_DEFAULTS.min_db);
      setElementValue('max_db', OPTIMIZATION_DEFAULTS.max_db);
      setElementValue('min_q', OPTIMIZATION_DEFAULTS.min_q);
      setElementValue('max_q', OPTIMIZATION_DEFAULTS.max_q);
      setElementValue('min_freq', OPTIMIZATION_DEFAULTS.min_freq);
      setElementValue('max_freq', OPTIMIZATION_DEFAULTS.max_freq);
      setElementValue('curve_name', OPTIMIZATION_DEFAULTS.curve_name);
      setElementValue('loss', OPTIMIZATION_DEFAULTS.loss);
      setElementValue('iir_hp_pk', OPTIMIZATION_DEFAULTS.iir_hp_pk);

      // Algorithm parameters
      setElementValue('algo', OPTIMIZATION_DEFAULTS.algo);
      setElementValue('population', OPTIMIZATION_DEFAULTS.population);
      setElementValue('maxeval', OPTIMIZATION_DEFAULTS.maxeval);
      setElementValue('strategy', OPTIMIZATION_DEFAULTS.strategy, true);
      setElementValue('de_f', OPTIMIZATION_DEFAULTS.de_f, true);
      setElementValue('de_cr', OPTIMIZATION_DEFAULTS.de_cr, true);
      setElementValue('adaptive_weight_f', OPTIMIZATION_DEFAULTS.adaptive_weight_f, true);
      setElementValue('adaptive_weight_cr', OPTIMIZATION_DEFAULTS.adaptive_weight_cr, true);

      // Spacing parameters
      setElementValue('min_spacing_oct', OPTIMIZATION_DEFAULTS.min_spacing_oct);
      setElementValue('spacing_weight', OPTIMIZATION_DEFAULTS.spacing_weight);

      // Tolerance parameters
      setElementValue('tolerance', OPTIMIZATION_DEFAULTS.tolerance);
      setElementValue('abs_tolerance', OPTIMIZATION_DEFAULTS.abs_tolerance);

      // Refinement parameters
      setElementValue('refine', OPTIMIZATION_DEFAULTS.refine);
      setElementValue('local_algo', OPTIMIZATION_DEFAULTS.local_algo, true);

      // Smoothing parameters
      setElementValue('smooth', OPTIMIZATION_DEFAULTS.smooth);
      setElementValue('smooth_n', OPTIMIZATION_DEFAULTS.smooth_n);
    }

    this.updateConditionalParameters();
    console.log('Form reset to defaults');
  }

  updateConditionalParameters(): void {
    const algo = (document.getElementById('algo') as HTMLSelectElement)?.value;
    const inputType = (document.querySelector('input[name="input_source"]:checked') as HTMLInputElement)?.value;

    // Show/hide DE-specific parameters
    const deParams = document.getElementById('de-params');
    if (deParams) {
      deParams.style.display = algo === 'autoeq_de' ? 'block' : 'none';
    }

    // Show/hide speaker selection
    const speakerSelection = document.getElementById('speaker-selection');
    if (speakerSelection) {
      speakerSelection.style.display = inputType === 'speaker' ? 'block' : 'none';
    }

    // Show/hide file selection
    const fileSelection = document.getElementById('file-selection');
    if (fileSelection) {
      fileSelection.style.display = inputType === 'file' ? 'block' : 'none';
    }

    // Show/hide capture section
    const captureSection = document.getElementById('capture-section');
    if (captureSection) {
      captureSection.style.display = inputType === 'capture' ? 'block' : 'none';
    }

    // Show/hide curve selection based on input type
    const curveNameParam = document.getElementById('curve_name')?.closest('.param-item') as HTMLElement;
    if (curveNameParam) {
      // Hide curve selection for headphones (they use targets instead)
      curveNameParam.style.display = inputType === 'headphone' ? 'none' : 'block';
    }

    // Update loss function options based on input type
    const lossSelect = document.getElementById('loss') as HTMLSelectElement;
    if (lossSelect) {
      this.updateLossOptions(inputType, lossSelect);
    }
  }

  private switchTab(tabName: string): void {
    console.log('Switching to tab:', tabName);

    // Remove active class from all tab labels
    const tabLabels = document.querySelectorAll('.tab-label');
    tabLabels.forEach(label => label.classList.remove('active'));

    // Add active class to current tab label
    const activeTabLabel = document.querySelector(`.tab-label[data-tab="${tabName}"]`);
    if (activeTabLabel) {
      activeTabLabel.classList.add('active');
    }

    // Hide all tab content
    const tabContents = document.querySelectorAll('.tab-content');
    tabContents.forEach(content => content.classList.remove('active'));

    // Show current tab content
    const activeTabContent = document.getElementById(`${tabName}_inputs`);
    if (activeTabContent) {
      activeTabContent.classList.add('active');
    } else {
      console.warn(`Tab content for '${tabName}' not found`);
    }

    // Set appropriate loss function based on tab
    const lossSelect = document.getElementById('loss') as HTMLSelectElement;
    if (lossSelect) {
      if (tabName === 'speaker') {
        lossSelect.value = 'speaker-flat';
        console.log('Set loss function to speaker-flat for speaker tab');
      } else if (tabName === 'headphone') {
        lossSelect.value = 'headphone-flat';
        console.log('Set loss function to headphone-flat for headphone tab');
      }
    }
  }

  private updateLossOptions(inputType: string, lossSelect: HTMLSelectElement): void {
    // Import loss options
    import('./optimization-constants').then(({ SPEAKER_LOSS_OPTIONS, HEADPHONE_LOSS_OPTIONS }) => {
      const currentValue = lossSelect.value;

      // Clear existing options
      lossSelect.innerHTML = '';

      // Determine which options to use
      const options = inputType === 'headphone' ? HEADPHONE_LOSS_OPTIONS : SPEAKER_LOSS_OPTIONS;

      // Populate with appropriate options
      Object.entries(options).forEach(([value, label]) => {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = label;
        lossSelect.appendChild(option);
      });

      // Try to keep the current value if it's still valid, otherwise set default
      if (lossSelect.querySelector(`option[value="${currentValue}"]`)) {
        lossSelect.value = currentValue;
      } else {
        lossSelect.value = inputType === 'headphone' ? 'headphone-flat' : 'speaker-flat';
      }
    });
  }

  // Event handlers (to be connected to main application logic)
  private onOptimizeClick(): void {
    // This will be connected to the main optimization logic
    console.log('Optimize button clicked');
  }

  private async onCaptureClick(): Promise<void> {
    console.log('Capture button clicked');

    if (!this.captureBtn) return;

    const isCapturing = this.captureBtn.textContent?.includes('Stop');

    if (isCapturing) {
      // Stop capture
      this.stopCapture();
    } else {
      // Start capture
      await this.startCapture();
    }
  }

  private async startCapture(): Promise<void> {
    if (!this.captureBtn || !this.captureStatus || !this.captureStatusText) return;

    try {
      // Update UI to capturing state
      this.captureBtn.textContent = '⏹️ Stop Capture';
      this.captureBtn.classList.add('capturing');
      this.captureStatus.style.display = 'block';
      this.captureStatusText.textContent = 'Starting capture...';

      // Hide any previous results
      if (this.captureResult) {
        this.captureResult.style.display = 'none';
      }

      // Import AudioProcessor dynamically to avoid circular dependencies
      const { AudioProcessor } = await import('./audio/audio-processor');
      const audioProcessor = new AudioProcessor();

      try {
        this.captureStatusText.textContent = 'Capturing audio (please wait)...';

        // Start the capture
        const result = await audioProcessor.startCapture();

        if (result.success && result.frequencies.length > 0) {
          console.log('Capture successful:', result.frequencies.length, 'points');

          // Store the captured data for optimization
          const { OptimizationManager } = await import('./optimization-manager');
          // Note: In a real implementation, you'd get the optimization manager instance from main
          // For now, we'll just log success

          this.captureStatusText.textContent = `✅ Captured ${result.frequencies.length} frequency points`;

          // Show results
          if (this.captureResult) {
            this.captureResult.style.display = 'block';
          }

          // TODO: Plot the captured data
          this.plotCapturedData(result.frequencies, result.magnitudes);

        } else {
          throw new Error(result.error || 'Capture failed');
        }

      } finally {
        audioProcessor.destroy();
      }

    } catch (error) {
      console.error('Capture error:', error);

      if (this.captureStatusText) {
        this.captureStatusText.textContent = `❌ Capture failed: ${error instanceof Error ? error.message : 'Unknown error'}`;
      }
    } finally {
      // Reset UI
      if (this.captureBtn) {
        this.captureBtn.textContent = '🎤 Start Capture';
        this.captureBtn.classList.remove('capturing');
      }
    }
  }

  private stopCapture(): void {
    console.log('Stopping capture...');

    // Reset UI immediately
    if (this.captureBtn) {
      this.captureBtn.textContent = '🎤 Start Capture';
      this.captureBtn.classList.remove('capturing');
    }

    if (this.captureStatusText) {
      this.captureStatusText.textContent = 'Capture stopped';
    }
  }

  private plotCapturedData(frequencies: number[], magnitudes: number[]): void {
    console.log('Plotting captured data...');
    // TODO: Implement plotting using PlotManager
    // For now, just log the data
    console.log('Frequencies:', frequencies.slice(0, 10), '...');
    console.log('Magnitudes:', magnitudes.slice(0, 10), '...');
  }

  private clearCaptureResults(): void {
    // This will be connected to the capture logic
    console.log('TODO Clear capture results');
  }

  private onListenClick(): void {
    // This will be connected to the audio logic
    console.log('TODO: Listen button clicked');
  }

  private onStopClick(): void {
    // This will be connected to the audio logic
    console.log('TODO: Stop button clicked');
  }

  private cancelOptimization(): void {
    // This will be connected to the optimization logic
    console.log('TODO: Cancel optimization');
  }

  // Getters for accessing UI elements from main application
  getForm(): HTMLFormElement { return this.form; }
  getOptimizeBtn(): HTMLButtonElement { return this.optimizeBtn; }
  getResetBtn(): HTMLButtonElement { return this.resetBtn; }
  getListenBtn(): HTMLButtonElement { return this.listenBtn; }
  getStopBtn(): HTMLButtonElement { return this.stopBtn; }
  getEqOnBtn(): HTMLButtonElement { return this.eqOnBtn; }
  getEqOffBtn(): HTMLButtonElement { return this.eqOffBtn; }
  getCancelOptimizationBtn(): HTMLButtonElement { return this.cancelOptimizationBtn; }

  updateOptimizeBtn(btn: HTMLButtonElement): void { this.optimizeBtn = btn; }
  updateResetBtn(btn: HTMLButtonElement): void { this.resetBtn = btn; }
  updateListenBtn(btn: HTMLButtonElement): void { this.listenBtn = btn; }
  updateStopBtn(btn: HTMLButtonElement): void { this.stopBtn = btn; }
  updateEqOnBtn(btn: HTMLButtonElement): void { this.eqOnBtn = btn; }
  updateEqOffBtn(btn: HTMLButtonElement): void { this.eqOffBtn = btn; }
  updateCancelOptimizationBtn(btn: HTMLButtonElement): void { this.cancelOptimizationBtn = btn; }
  getAudioStatus(): HTMLElement { return this.audioStatus; }
  getAudioStatusText(): HTMLElement { return this.audioStatusText; }
  getAudioDuration(): HTMLElement { return this.audioDuration; }
  getAudioPosition(): HTMLElement { return this.audioPosition; }
  getAudioProgressFill(): HTMLElement { return this.audioProgressFill; }

  // Capture elements
  getCaptureBtn(): HTMLButtonElement | null { return this.captureBtn; }
  getCaptureStatus(): HTMLElement | null { return this.captureStatus; }
  getCaptureStatusText(): HTMLElement | null { return this.captureStatusText; }
  getCaptureProgressFill(): HTMLElement | null { return this.captureProgressFill; }
  getCaptureWaveform(): HTMLCanvasElement | null { return this.captureWaveform; }
  getCaptureWaveformCtx(): CanvasRenderingContext2D | null { return this.captureWaveformCtx; }
  getCaptureResult(): HTMLElement | null { return this.captureResult; }
  getCaptureClearBtn(): HTMLButtonElement | null { return this.captureClearBtn; }
  getCapturePlot(): HTMLElement | null { return this.capturePlot; }

  // State getters
  isEQEnabled(): boolean { return this.eqEnabled; }

  // Audio control methods
  setAudioStatus(status: string): void {
    console.log('setAudioStatus called with:', status);
    if (this.audioStatusText) {
      this.audioStatusText.textContent = status;
      console.log('Audio status updated to:', status);
    } else {
      console.warn('Audio status text element not found!');
    }
  }

  setListenButtonEnabled(enabled: boolean): void {
    if (this.listenBtn) {
      this.listenBtn.disabled = !enabled;
      if (enabled) {
        this.listenBtn.classList.remove('disabled');
      } else {
        this.listenBtn.classList.add('disabled');
      }
    } else {
      console.warn('Listen button not found in UIManager!');
    }
  }
}
