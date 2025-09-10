import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import Plotly from 'plotly.js-dist-min';

// Types for our optimization parameters and results
interface OptimizationParams {
  num_filters: number;
  curve_path?: string;
  target_path?: string;
  sample_rate: number;
  max_db: number;
  min_db: number;
  max_q: number;
  min_q: number;
  min_freq: number;
  max_freq: number;
  speaker?: string;
  version?: string;
  measurement?: string;
  curve_name: string;
  algo: string;
  population: number;
  maxeval: number;
  refine: boolean;
  local_algo: string;
  min_spacing_oct: number;
  spacing_weight: number;
  smooth: boolean;
  smooth_n: number;
  loss: string;
  iir_hp_pk: boolean;
  // DE-specific parameters
  strategy?: string;
  de_f?: number;
  de_cr?: number;
  adaptive_weight_f?: number;
  adaptive_weight_cr?: number;
  // Tolerance parameters
  tolerance: number;
  atolerance: number;
}

interface PlotData {
  frequencies: number[];
  curves: { [name: string]: number[] };
  metadata: { [key: string]: any };
}

interface OptimizationResult {
  success: boolean;
  error_message?: string;
  filter_params?: number[];
  objective_value?: number;
  preference_score_before?: number;
  preference_score_after?: number;
  filter_response?: PlotData;
  spin_details?: PlotData;
}

class AutoEQUI {
  private form: HTMLFormElement;
  private optimizeBtn: HTMLButtonElement;
  private resetBtn: HTMLButtonElement;
  private progressElement: HTMLElement;
  private scoresElement: HTMLElement;
  private errorElement: HTMLElement;
  private filterDetailsPlotElement: HTMLElement;
  private filterPlotElement: HTMLElement;
  private onAxisPlotElement: HTMLElement;
  private listeningWindowPlotElement: HTMLElement;
  private earlyReflectionsPlotElement: HTMLElement;
  private soundPowerPlotElement: HTMLElement;
  private spinPlotElement: HTMLElement;

  // API data caching
  private speakers: string[] = [];
  private selectedSpeaker: string = '';
  private selectedVersion: string = '';

  // Resizer state
  private isResizing: boolean = false;
  private startX: number = 0;
  private startWidth: number = 0;

  // Modal elements
  private optimizationModal: HTMLElement;
  private progressStatus: HTMLElement;
  private progressFill: HTMLElement;
  private progressPercentage: HTMLElement;
  private progressTableBody: HTMLElement;
  private cancelOptimizationBtn: HTMLButtonElement;
  private doneOptimizationBtn: HTMLButtonElement;
  private modalCloseBtn: HTMLButtonElement;
  private progressGraphElement: HTMLElement;

  // Optimization state
  private isOptimizationRunning: boolean = false;
  private progressUnlisten?: UnlistenFn;
  private optimizationStages: { [key: string]: { status: string, startTime?: number, endTime?: number, details?: string } } = {};

  // Progress graph data
  private progressGraphData: { iteration: number; fitness: number; convergence: number }[] = [];
  private lastGraphUpdate: number = 0;

  // Audio testing elements and state
  private demoAudioSelect: HTMLSelectElement;
  private eqOnBtn: HTMLButtonElement;
  private eqOffBtn: HTMLButtonElement;
  private listenBtn: HTMLButtonElement;
  private stopBtn: HTMLButtonElement;
  private eqEnabled: boolean = true;
  private audioStatus: HTMLElement;
  private audioStatusText: HTMLElement;
  private audioDuration: HTMLElement;
  private audioPosition: HTMLElement;
  private audioProgressFill: HTMLElement;

  // Web Audio API state
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

  constructor() {
    this.form = document.getElementById('autoeq-form') as HTMLFormElement;
    this.optimizeBtn = document.getElementById('optimize-btn') as HTMLButtonElement;
    this.resetBtn = document.getElementById('reset-btn') as HTMLButtonElement;
    this.progressElement = document.getElementById('optimization-progress') as HTMLElement;
    this.scoresElement = document.getElementById('scores-display') as HTMLElement;
    this.errorElement = document.getElementById('error-display') as HTMLElement;
    this.filterDetailsPlotElement = document.getElementById('filter-details-plot') as HTMLElement;
    this.filterPlotElement = document.getElementById('filter-plot') as HTMLElement;
    this.onAxisPlotElement = document.getElementById('on-axis-plot') as HTMLElement;
    this.listeningWindowPlotElement = document.getElementById('listening-window-plot') as HTMLElement;
    this.earlyReflectionsPlotElement = document.getElementById('early-reflections-plot') as HTMLElement;
    this.soundPowerPlotElement = document.getElementById('sound-power-plot') as HTMLElement;
    this.spinPlotElement = document.getElementById('spin-plot') as HTMLElement;

    // Initialize modal elements
    this.optimizationModal = document.getElementById('optimization-modal') as HTMLElement;
    this.progressStatus = document.getElementById('progress-status') as HTMLElement;
    this.progressFill = document.getElementById('progress-fill') as HTMLElement;
    this.progressPercentage = document.getElementById('progress-percentage') as HTMLElement;
    this.progressTableBody = document.getElementById('progress-table-body') as HTMLElement;
    this.cancelOptimizationBtn = document.getElementById('cancel-optimization') as HTMLButtonElement;
    this.doneOptimizationBtn = document.getElementById('done-optimization') as HTMLButtonElement;
    this.modalCloseBtn = document.getElementById('modal-close') as HTMLButtonElement;
    this.progressGraphElement = document.getElementById('progress-graph') as HTMLElement;

    // Initialize audio elements
    this.demoAudioSelect = document.getElementById('demo-audio-select') as HTMLSelectElement;
    this.eqOnBtn = document.getElementById('eq-on-btn') as HTMLButtonElement;
    this.eqOffBtn = document.getElementById('eq-off-btn') as HTMLButtonElement;
    this.listenBtn = document.getElementById('listen-btn') as HTMLButtonElement;
    this.stopBtn = document.getElementById('stop-btn') as HTMLButtonElement;
    this.audioStatus = document.getElementById('audio-status') as HTMLElement;
    this.audioStatusText = document.getElementById('audio-status-text') as HTMLElement;
    this.audioDuration = document.getElementById('audio-duration') as HTMLElement;
    this.audioPosition = document.getElementById('audio-position') as HTMLElement;
    this.audioProgressFill = document.getElementById('audio-progress-fill') as HTMLElement;

    this.setupEventListeners();
    this.setupUIInteractions();
    this.setupModalEventListeners();
    this.setupResizer();
    this.setupAutocomplete();
    this.setupAudioEventListeners();
    this.resetToDefaults();
    this.updateConditionalParameters();

    // Initialize EQ to Off (no filters loaded yet)
    this.setEQEnabled(false);

    // Ensure all accordion sections start collapsed
    this.collapseAllAccordion();
  }

  private setupEventListeners(): void {
    // Form submission
    this.form.addEventListener('submit', (e) => {
      e.preventDefault();
      this.runOptimization();
    });

    // Reset button
    this.resetBtn.addEventListener('click', () => {
      this.resetToDefaults();
    });


    // File browser buttons
    const browseCurveBtn = document.getElementById('browse-curve');
    console.log('Browse curve button found:', browseCurveBtn);
    browseCurveBtn?.addEventListener('click', (e) => {
      console.log('Browse curve button clicked');
      e.preventDefault();
      this.openFileDialog('curve-path');
    });

    const browseTargetBtn = document.getElementById('browse-target');
    console.log('Browse target button found:', browseTargetBtn);
    browseTargetBtn?.addEventListener('click', (e) => {
      console.log('Browse target button clicked');
      e.preventDefault();
      this.openFileDialog('target-path');
    });

    // Real-time parameter updates (debounced)
    let updateTimeout: number | null = null;
    this.form.addEventListener('input', () => {
      if (updateTimeout) {
        clearTimeout(updateTimeout);
      }
      updateTimeout = setTimeout(() => {
        this.validateForm();
      }, 300);
    });
  }

  private setupUIInteractions(): void {
    // Input source tabs
    const tabLabels = document.querySelectorAll('.tab-label');
    const tabContents = document.querySelectorAll('.tab-content');

    tabLabels.forEach(label => {
      label.addEventListener('click', () => {
        const target = label.getAttribute('data-tab');

        // Update tab states
        tabLabels.forEach(l => l.classList.remove('active'));
        tabContents.forEach(c => c.classList.remove('active'));

        label.classList.add('active');
        const targetContent = document.getElementById(target + '-inputs');
        if (targetContent) {
          targetContent.classList.add('active');
        }

        this.validateForm();
      });
    });

    // Plot accordion headers with improved behavior
    this.setupAccordionBehavior();

    // Algorithm change handler for conditional parameters
    const algoSelect = document.getElementById('algo') as HTMLSelectElement;
    algoSelect?.addEventListener('change', () => {
      this.updateConditionalParameters();
      this.clearAllPlots(); // Clear plots when algorithm changes
    });

    // Strategy change handler for adaptive parameters
    const strategySelect = document.getElementById('strategy') as HTMLSelectElement;
    strategySelect?.addEventListener('change', () => {
      this.updateConditionalParameters();
    });

    // Refinement checkbox handler for local algo dependency
    const refineCheckbox = document.getElementById('refine') as HTMLInputElement;
    refineCheckbox?.addEventListener('change', () => {
      this.updateConditionalParameters();
    });

    // Population input handler for validation warning
    const populationInput = document.getElementById('population') as HTMLInputElement;
    populationInput?.addEventListener('input', () => {
      this.validatePopulation();
    });

    // Version selection handler
    const versionSelect = document.getElementById('version') as HTMLSelectElement;
    versionSelect?.addEventListener('change', (e) => {
      const version = (e.target as HTMLSelectElement).value;
      if (version) {
        this.selectVersion(version);
      }
    });

    // Measurement selection handler
    const measurementSelect = document.getElementById('measurement') as HTMLSelectElement;
    measurementSelect?.addEventListener('change', () => {
      this.validateForm();
      // Open filter details when a measurement is selected via API
      const val = measurementSelect.value;
      if (val && val.trim() !== '') {
        this.expandPlotSection('filter-details-plot');
      }
    });
  }

  private setupModalEventListeners(): void {
    // Cancel optimization button
    this.cancelOptimizationBtn?.addEventListener('click', () => {
      this.cancelOptimization();
    });

    // Done optimization button
    this.doneOptimizationBtn?.addEventListener('click', () => {
      this.closeOptimizationModal();
    });

    // Modal close button
    this.modalCloseBtn?.addEventListener('click', () => {
      if (this.isOptimizationRunning) {
        // Show confirmation before closing during optimization
        if (confirm('Optimization is still running. Do you want to cancel it?')) {
          this.cancelOptimization();
        }
      } else {
        this.closeOptimizationModal();
      }
    });

    // Close modal when clicking outside (optional)
    this.optimizationModal?.addEventListener('click', (e) => {
      if (e.target === this.optimizationModal) {
        if (this.isOptimizationRunning) {
          // Don't close during optimization
          return;
        }
        this.closeOptimizationModal();
      }
    });

    // Handle escape key
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.optimizationModal.style.display !== 'none') {
        if (this.isOptimizationRunning) {
          if (confirm('Optimization is still running. Do you want to cancel it?')) {
            this.cancelOptimization();
          }
        } else {
          this.closeOptimizationModal();
        }
      }
    });
  }

  private async openFileDialog(inputId: string): Promise<void> {
    console.log('openFileDialog called for:', inputId);
    try {
      const input = document.getElementById(inputId) as HTMLInputElement;
      if (!input) {
        console.error('Input element not found:', inputId);
        return;
      }

      console.log('Input element found:', input);
      console.log('Opening file dialog...');

      // Enhanced dialog options for better macOS compatibility
      const result = await openDialog({
        multiple: false,
        directory: false,
        filters: [{
          name: 'CSV Files',
          extensions: ['csv']
        }, {
          name: 'All Files',
          extensions: ['*']
        }],
        defaultPath: undefined,
        title: inputId.includes('target') ? 'Select Target CSV File (Optional)' : 'Select Input CSV File'
      });

      console.log('Dialog result:', result);
      if (result && typeof result === 'string') {
        console.log('Setting input value to:', result);
        input.value = result;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.validateForm();
        // Clear plots when curve changes
        this.clearAllPlots();
        // Open filter details when a curve file is selected
        if (inputId === 'curve-path') {
          this.expandPlotSection('filter-details-plot');
        }
        // Show success feedback
        this.showFileSelectionSuccess(inputId, result);
      } else if (result === null) {
        console.log('Dialog cancelled by user');
      } else if (Array.isArray(result) && result.length > 0) {
        // Handle array result (shouldn't happen with multiple: false, but just in case)
        const filePath = result[0];
        console.log('Setting input value to (from array):', filePath);
        input.value = filePath;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.validateForm();
        // Clear plots when curve changes
        this.clearAllPlots();
        if (inputId === 'curve-path') {
          this.expandPlotSection('filter-details-plot');
        }
        const baseName = (filePath.split('/').pop() || filePath);
        this.showFallbackWarning(inputId, baseName);
      } else {
        console.log('No file selected or unexpected result format:', result);
      }
    } catch (error) {
      console.error('Error opening file dialog:', error);
      // Show error message to user
      this.showFileDialogError(error);
      // Fallback: try to trigger a native file input
      this.fallbackFileDialog(inputId);
    }
  }

  private fallbackFileDialog(inputId: string): void {
    console.log('Using fallback file dialog for:', inputId);
    const input = document.getElementById(inputId) as HTMLInputElement;
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.accept = '.csv,text/csv';
    fileInput.style.display = 'none';

    fileInput.onchange = (event) => {
      const file = (event.target as HTMLInputElement).files?.[0];
      if (file) {
        // In fallback mode, we can only get the filename, not the full path
        // This is a browser security limitation
        input.value = file.name; // Note: This gives filename, not full path
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
        this.validateForm();
        // Clear plots when curve changes
        this.clearAllPlots();
        if (inputId === 'curve-path') {
          this.expandPlotSection('filter-details-plot');
        }
        this.showFallbackWarning(inputId, file.name);
      }
    };

    document.body.appendChild(fileInput);
    fileInput.click();
    document.body.removeChild(fileInput);
  }

  private showFileSelectionSuccess(inputId: string, filePath: string): void {
    const fileName = filePath.split('/').pop() || filePath;
    const message = `Selected file: ${fileName}`;
    console.log('File selection success:', message);

    // Add visual feedback to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = '#28a745'; // Green border for success
      input.title = `Selected: ${filePath}`;
      setTimeout(() => {
        input.style.borderColor = ''; // Reset border after 2 seconds
      }, 2000);
    }
  }

  private showFileDialogError(error: any): void {
    console.error('File dialog error details:', error);
    const message = `File dialog failed: ${error?.message || error}. Using fallback file picker.`;
    console.warn(message);

    // Show temporary error message to user
    this.showTemporaryMessage(message, 'error');
  }

  private showFallbackWarning(inputId: string, fileName: string): void {
    const message = `Using fallback file picker. Selected: ${fileName}. Note: Full file path not available in browser mode.`;
    console.warn(`Fallback mode for ${inputId}:`, message);

    // Show warning message
    this.showTemporaryMessage(message, 'warning');

    // Add visual indication to the input
    const input = document.getElementById(inputId) as HTMLInputElement;
    if (input) {
      input.style.borderColor = '#ffc107'; // Yellow border for warning
      input.title = `Fallback mode: ${fileName} (full path not available)`;
      setTimeout(() => {
        input.style.borderColor = ''; // Reset border after 3 seconds
      }, 3000);
    }
  }

  private showTemporaryMessage(message: string, type: 'error' | 'warning' | 'success' = 'error'): void {
    // Create temporary message element
    const messageDiv = document.createElement('div');
    messageDiv.textContent = message;
    messageDiv.style.cssText = `
      position: fixed;
      top: 20px;
      right: 20px;
      max-width: 400px;
      padding: 12px 16px;
      border-radius: 6px;
      font-size: 14px;
      z-index: 10000;
      box-shadow: 0 4px 12px rgba(0,0,0,0.2);
      animation: slideIn 0.3s ease-out;
      ${type === 'error' ? 'background-color: #dc3545; color: white;' :
        type === 'warning' ? 'background-color: #ffc107; color: black;' :
        'background-color: #28a745; color: white;'}
    `;

    // Add animation keyframes if not already added
    if (!document.getElementById('temp-message-styles')) {
      const style = document.createElement('style');
      style.id = 'temp-message-styles';
      style.textContent = `
        @keyframes slideIn {
          from { transform: translateX(100%); opacity: 0; }
          to { transform: translateX(0); opacity: 1; }
        }
        @keyframes slideOut {
          from { transform: translateX(0); opacity: 1; }
          to { transform: translateX(100%); opacity: 0; }
        }
      `;
      document.head.appendChild(style);
    }

    document.body.appendChild(messageDiv);

    // Remove after 4 seconds
    setTimeout(() => {
      messageDiv.style.animation = 'slideOut 0.3s ease-in forwards';
      setTimeout(() => {
        if (messageDiv.parentNode) {
          messageDiv.parentNode.removeChild(messageDiv);
        }
      }, 300);
    }, 4000);
  }

  private showOptimizationModal(): void {
    this.optimizationModal.style.display = 'flex';
    this.resetModalState();
  }

  private closeOptimizationModal(): void {
    this.optimizationModal.style.display = 'none';
    this.isOptimizationRunning = false;
  }

  private resetModalState(): void {
    this.progressStatus.textContent = 'Initializing...';
    this.progressFill.style.width = '0%';
    this.progressPercentage.textContent = '0%';
    this.progressTableBody.innerHTML = '';
    this.cancelOptimizationBtn.style.display = 'inline-flex';
    this.doneOptimizationBtn.style.display = 'none';
    this.optimizationStages = {};

    // Reset progress graph data
    this.progressGraphData = [];
    this.lastGraphUpdate = 0;
    this.clearProgressGraph();
  }

  private updateProgress(stage: string, status: 'pending' | 'running' | 'completed' | 'error', details?: string, progress?: number): void {
    const now = Date.now();

    if (!this.optimizationStages[stage]) {
      this.optimizationStages[stage] = { status: 'pending' };
    }

    const stageData = this.optimizationStages[stage];
    const oldStatus = stageData.status;

    stageData.status = status;
    stageData.details = details || '';

    if (status === 'running' && oldStatus !== 'running') {
      stageData.startTime = now;
    } else if ((status === 'completed' || status === 'error') && stageData.startTime) {
      stageData.endTime = now;
    }

    this.updateProgressTable();

    if (progress !== undefined) {
      this.updateProgressBar(progress);
    }

    // Update status text
    if (status === 'running') {
      this.progressStatus.textContent = `${stage}...`;
    }
  }

  private updateProgressBar(percentage: number): void {
    const clampedPercentage = Math.max(0, Math.min(100, percentage));
    this.progressFill.style.width = `${clampedPercentage}%`;
    this.progressPercentage.textContent = `${Math.round(clampedPercentage)}%`;
  }

  private updateProgressTable(): void {
    const stages = Object.keys(this.optimizationStages);

    this.progressTableBody.innerHTML = '';

    stages.forEach(stageName => {
      const stage = this.optimizationStages[stageName];
      const row = document.createElement('tr');

      let duration = '';
      if (stage.startTime) {
        const endTime = stage.endTime || Date.now();
        const durationMs = endTime - stage.startTime;
        duration = `${(durationMs / 1000).toFixed(1)}s`;
      }

      row.innerHTML = `
        <td>${stageName}</td>
        <td class="status-${stage.status}">${stage.status.charAt(0).toUpperCase() + stage.status.slice(1)}</td>
        <td>${duration}</td>
        <td>${stage.details || ''}</td>
      `;

      this.progressTableBody.appendChild(row);
    });
  }

  private cancelOptimization(): void {
    if (this.isOptimizationRunning) {
      this.isOptimizationRunning = false;
      this.updateProgress('Optimization', 'error', 'Cancelled by user', 100);
      this.progressStatus.textContent = 'Optimization cancelled';
      this.cancelOptimizationBtn.style.display = 'none';
      this.doneOptimizationBtn.style.display = 'inline-flex';
    }
  }

  private optimizationCompleted(success: boolean, message?: string): void {
    this.isOptimizationRunning = false;
    this.updateProgress('Optimization', success ? 'completed' : 'error', message || (success ? 'Completed successfully' : 'Failed'), 100);
    this.progressStatus.textContent = success ? 'Optimization completed!' : 'Optimization failed';
    this.cancelOptimizationBtn.style.display = 'none';
    this.doneOptimizationBtn.style.display = 'inline-flex';
    // Final graph update
    if (this.progressGraphData.length > 0) {
      this.updateProgressGraph();
    }
  }

  private clearProgressGraph(): void {
    if (this.progressGraphElement) {
      try {
        Plotly.purge(this.progressGraphElement);
      } catch (e) {
        // Element may not have been plotted yet
      }
      this.progressGraphElement.innerHTML = '';
    }
  }

  private updateProgressGraph(): void {
    if (!this.progressGraphElement || this.progressGraphData.length === 0) {
      return;
    }

    const iterations = this.progressGraphData.map(d => d.iteration);
    const fitness = this.progressGraphData.map(d => d.fitness);
    const convergence = this.progressGraphData.map(d => d.convergence);

    const fitnessTrace = {
      x: iterations,
      y: fitness,
      type: 'scatter' as const,
      mode: 'lines+markers' as const,
      name: 'Fitness (f)',
      yaxis: 'y',
      line: { color: '#007bff', width: 2 },
      marker: { size: 4 }
    };

    const convergenceTrace = {
      x: iterations,
      y: convergence,
      type: 'scatter' as const,
      mode: 'lines+markers' as const,
      name: 'Convergence',
      yaxis: 'y2',
      line: { color: '#ff7f0e', width: 2 },
      marker: { size: 4 }
    };

    const layout = {
      title: {
        text: 'Optimization Progress',
        font: { size: 14 }
      },
      width: 400,
      height: 400,
      margin: { l: 60, r: 60, t: 40, b: 40 },
      xaxis: {
        title: { text: 'Iterations' },
        showgrid: true,
        zeroline: false
      },
      yaxis: {
        title: {
          text: 'Fitness (f)',
          font: { color: '#007bff' }
        },
        side: 'left' as const,
        showgrid: true,
        zeroline: false,
        tickfont: { color: '#007bff' }
      },
      yaxis2: {
        title: {
          text: 'Convergence',
          font: { color: '#ff7f0e' }
        },
        side: 'right' as const,
        overlaying: 'y' as const,
        showgrid: false,
        zeroline: false,
        tickfont: { color: '#ff7f0e' }
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 11
      },
      showlegend: true,
      legend: {
        x: 0,
        y: 1,
        bgcolor: 'rgba(0,0,0,0)'
      },
      hovermode: 'x unified' as const
    };

    const config = {
      responsive: false,
      displayModeBar: false,
      staticPlot: false
    };

    try {
      if (this.progressGraphElement.hasChildNodes()) {
        // Update existing plot
        Plotly.react(this.progressGraphElement, [fitnessTrace, convergenceTrace], layout, config);
      } else {
        // Create new plot
        Plotly.newPlot(this.progressGraphElement, [fitnessTrace, convergenceTrace], layout, config);
      }
    } catch (error) {
      console.error('Error updating progress graph:', error);
    }
  }

  private resetToDefaults(): void {
    // Reset to API input mode (default)
    const fileTab = document.querySelector('[data-tab="file"]') as HTMLElement;
    const apiTab = document.querySelector('[data-tab="api"]') as HTMLElement;
    const fileContent = document.getElementById('file-inputs');
    const apiContent = document.getElementById('api-inputs');

    fileTab?.classList.remove('active');
    apiTab?.classList.add('active');
    fileContent?.classList.remove('active');
    apiContent?.classList.add('active');

    // Reset form fields
    (document.getElementById('num-filters') as HTMLInputElement).value = '7';
    (document.getElementById('sample-rate') as HTMLInputElement).value = '48000';
    (document.getElementById('curve-name') as HTMLSelectElement).value = 'Listening Window';
    (document.getElementById('max-db') as HTMLInputElement).value = '3.0';
    (document.getElementById('min-db') as HTMLInputElement).value = '1.0';
    (document.getElementById('max-q') as HTMLInputElement).value = '3.0';
    (document.getElementById('min-q') as HTMLInputElement).value = '1.0';
    (document.getElementById('min-freq') as HTMLInputElement).value = '60';
    (document.getElementById('max-freq') as HTMLInputElement).value = '16000';
    (document.getElementById('algo') as HTMLSelectElement).value = 'autoeq:de';
    (document.getElementById('loss') as HTMLSelectElement).value = 'flat';
    (document.getElementById('population') as HTMLInputElement).value = '300';
    (document.getElementById('maxeval') as HTMLInputElement).value = '200000';
    (document.getElementById('refine') as HTMLInputElement).checked = false;
    (document.getElementById('local-algo') as HTMLSelectElement).value = 'cobyla';
    (document.getElementById('min-spacing-oct') as HTMLInputElement).value = '0.5';
    (document.getElementById('spacing-weight') as HTMLInputElement).value = '20.0';
    (document.getElementById('smooth') as HTMLInputElement).checked = true;
    (document.getElementById('smooth-n') as HTMLInputElement).value = '2';
    (document.getElementById('iir-hp-pk') as HTMLInputElement).checked = false;
    (document.getElementById('curve-path') as HTMLInputElement).value = '';
    (document.getElementById('target-path') as HTMLInputElement).value = '';
    (document.getElementById('speaker') as HTMLInputElement).value = '';
    (document.getElementById('version') as HTMLInputElement).value = '';
    (document.getElementById('measurement') as HTMLSelectElement).value = '';
    (document.getElementById('strategy') as HTMLSelectElement).value = 'currenttobest1bin';
    (document.getElementById('de-f') as HTMLInputElement).value = '0.8';
    (document.getElementById('de-cr') as HTMLInputElement).value = '0.9';
    (document.getElementById('adaptive-weight-f') as HTMLInputElement).value = '0.8';
    (document.getElementById('adaptive-weight-cr') as HTMLInputElement).value = '0.7';
    (document.getElementById('tolerance') as HTMLInputElement).value = '1e-6';
    (document.getElementById('abs-tolerance') as HTMLInputElement).value = '1e-6';

    // Update conditional parameters
    this.updateConditionalParameters();

    // Clear only errors on reset, keep existing plots
    this.errorElement.style.display = 'none';
    this.scoresElement.style.display = 'none';
    this.validateForm();
  }

  private updateConditionalParameters(): void {
    const algo = (document.getElementById('algo') as HTMLSelectElement).value;
    const refineEnabled = (document.getElementById('refine') as HTMLInputElement).checked;
    const localAlgoSelect = document.getElementById('local-algo') as HTMLSelectElement;
    const globalAlgoParams = document.querySelectorAll('.global-algo-param');
    const deParams = document.querySelectorAll('.de-param');
    const adaptiveParams = document.querySelectorAll('.adaptive-param');

    // Enable/disable local algo based on refinement checkbox
    localAlgoSelect.disabled = !refineEnabled;
    if (refineEnabled) {
      localAlgoSelect.style.color = 'var(--text-primary)';
    } else {
      localAlgoSelect.style.color = 'var(--text-secondary)';
    }

    // Show population and maxeval for global algorithms
    const isGlobalAlgo = [
      'nlopt:isres', 'nlopt:ags', 'nlopt:origdirect', 'nlopt:crs2lm',
      'nlopt:direct', 'nlopt:directl', 'nlopt:gmlsl', 'nlopt:gmlsllds',
      'nlopt:stogo', 'nlopt:stogorand', 'mh:de', 'mh:pso', 'mh:rga',
      'mh:tlbo', 'mh:firefly', 'autoeq:de',
      // Legacy support
      'isres', 'de', 'pso', 'stogo', 'ags', 'origdirect'
    ].includes(algo);

    globalAlgoParams.forEach(param => {
      if (isGlobalAlgo) {
        param.classList.add('show');
      } else {
        param.classList.remove('show');
      }
    });

    // Show DE parameters only for autoeq:de algorithm
    const isAutoEQDE = algo === 'autoeq:de';
    deParams.forEach(param => {
      (param as HTMLElement).style.display = isAutoEQDE ? 'flex' : 'none';
    });

    // Show adaptive parameters only for adaptive strategies
    if (isAutoEQDE) {
      const strategy = (document.getElementById('strategy') as HTMLSelectElement).value;
      const isAdaptive = strategy.includes('adaptive');
      adaptiveParams.forEach(param => {
        (param as HTMLElement).style.display = isAdaptive ? 'flex' : 'none';
      });
    } else {
      adaptiveParams.forEach(param => {
        (param as HTMLElement).style.display = 'none';
      });
    }

    // Validate population after parameter updates
    this.validatePopulation();
  }

  private validatePopulation(): void {
    const populationInput = document.getElementById('population') as HTMLInputElement;
    const yellowWarningElement = document.getElementById('population-warning-yellow') as HTMLElement;
    const redWarningElement = document.getElementById('population-warning-red') as HTMLElement;

    if (!populationInput || !yellowWarningElement || !redWarningElement) return;

    const population = parseInt(populationInput.value);

    // Hide all warnings initially
    yellowWarningElement.style.display = 'none';
    redWarningElement.style.display = 'none';

    // Show appropriate warning based on population value
    if (population > 30000) {
      redWarningElement.style.display = 'block';
    } else if (population > 3000) {
      yellowWarningElement.style.display = 'block';
    }

    // Ensure minimum value is 1 (positive integer validation)
    if (population < 1) {
      populationInput.value = '1';
    }
  }

  private expandPlotSection(plotElementId: string): void {
    console.log('Expanding plot section for:', plotElementId);
    const plotElement = document.getElementById(plotElementId);
    if (plotElement) {
      const plotSection = plotElement.closest('.plot-section');
      if (plotSection) {
        plotSection.classList.remove('collapsed');
        plotSection.classList.add('expanded');
        const arrow = plotSection.querySelector('.accordion-arrow');
        if (arrow) arrow.textContent = '▼';
        console.log('Plot section expanded:', plotElementId);
      }
    }
  }

  private collapseAllAccordion(): void {
    const sections = document.querySelectorAll('.plot-section');
    sections.forEach(section => {
      section.classList.remove('expanded');
      section.classList.add('collapsed');
      const header = section.querySelector('.plot-header');
      const arrow = section.querySelector('.accordion-arrow');
      if (header) (header as HTMLElement).setAttribute('aria-expanded', 'false');
      if (arrow) (arrow as HTMLElement).textContent = '▶';
    });
  }

  private clearAllPlots(): void {
    const allPlotElements = [
      this.filterDetailsPlotElement,
      this.filterPlotElement,
      this.onAxisPlotElement,
      this.listeningWindowPlotElement,
      this.earlyReflectionsPlotElement,
      this.soundPowerPlotElement,
      this.spinPlotElement
    ];

    try {
      allPlotElements.forEach(element => {
        if (element) {
          // Clear plotly plots if they exist
          try {
            Plotly.purge(element);
          } catch (e) {
            // Plot may not exist, ignore error
          }
          // Clear content and remove has-plot class
          element.innerHTML = '';
          element.classList.remove('has-plot');
        }
      });
    } catch (e) {
      console.log('No existing plots to purge');
    }

    // Collapse all accordion sections
    this.collapseAllAccordion();
  }

  private setupAccordionBehavior(): void {
    console.log('Setting up accordion behavior');
    const plotHeaders = document.querySelectorAll('.plot-header');
    console.log('Found plot headers:', plotHeaders.length);

    plotHeaders.forEach((header, index) => {
      console.log('Setting up header', index, header);

      // Make header focusable for keyboard navigation
      (header as HTMLElement).tabIndex = 0;
      (header as HTMLElement).setAttribute('role', 'button');
      (header as HTMLElement).setAttribute('aria-expanded', 'false');

      // Handle click events
      header.addEventListener('click', (e) => {
        e.preventDefault();
        this.toggleAccordionSection(header as HTMLElement);
      });

      // Handle keyboard events
      (header as HTMLElement).addEventListener('keydown', (e: KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          this.toggleAccordionSection(header as HTMLElement);
        } else if (e.key === 'ArrowDown') {
          e.preventDefault();
          this.focusNextAccordionHeader(header as HTMLElement);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          this.focusPreviousAccordionHeader(header as HTMLElement);
        }
      });
    });

    // Add scroll navigation support
    this.setupScrollNavigation();

    // Test accordion sections on startup
    this.testAccordionSections();
  }

  private toggleAccordionSection(header: HTMLElement): void {
    const target = header.getAttribute('data-plot');
    const section = header.parentElement as HTMLElement;
    const arrow = header.querySelector('.accordion-arrow');

    console.log('Toggling accordion section:', target);

    if (section && arrow) {
      const isExpanded = section.classList.contains('expanded');
      console.log('Current state - expanded:', isExpanded);

      if (isExpanded) {
        // Collapse
        console.log('Collapsing section:', target);
        section.classList.remove('expanded');
        section.classList.add('collapsed');
        arrow.textContent = '▶';
        header.setAttribute('aria-expanded', 'false');
      } else {
        // Expand
        console.log('Expanding section:', target);
        section.classList.remove('collapsed');
        section.classList.add('expanded');
        arrow.textContent = '▼';
        header.setAttribute('aria-expanded', 'true');

        // Scroll the expanded section into view
        setTimeout(() => {
          section.scrollIntoView({
            behavior: 'smooth',
            block: 'nearest',
            inline: 'nearest'
          });
        }, 100);
      }
    }
  }

  private focusNextAccordionHeader(currentHeader: HTMLElement): void {
    const allHeaders = Array.from(document.querySelectorAll('.plot-header')) as HTMLElement[];
    const currentIndex = allHeaders.indexOf(currentHeader);
    const nextIndex = (currentIndex + 1) % allHeaders.length;
    allHeaders[nextIndex].focus();
  }

  private focusPreviousAccordionHeader(currentHeader: HTMLElement): void {
    const allHeaders = Array.from(document.querySelectorAll('.plot-header')) as HTMLElement[];
    const currentIndex = allHeaders.indexOf(currentHeader);
    const previousIndex = currentIndex === 0 ? allHeaders.length - 1 : currentIndex - 1;
    allHeaders[previousIndex].focus();
  }

  private setupScrollNavigation(): void {
    const accordionContainer = document.querySelector('.plots-accordion');

    if (!accordionContainer) return;

    // Add mouse wheel scroll support
    accordionContainer.addEventListener('wheel', (e) => {
      // Allow native scrolling behavior
      // The CSS overflow-y: auto will handle this automatically
    });

    // Add keyboard scroll support for the container
    (accordionContainer as HTMLElement).addEventListener('keydown', (e: KeyboardEvent) => {
      if (e.target === accordionContainer) {
        switch (e.key) {
          case 'PageUp':
            e.preventDefault();
            accordionContainer.scrollBy({ top: -300, behavior: 'smooth' });
            break;
          case 'PageDown':
            e.preventDefault();
            accordionContainer.scrollBy({ top: 300, behavior: 'smooth' });
            break;
          case 'Home':
            e.preventDefault();
            accordionContainer.scrollTo({ top: 0, behavior: 'smooth' });
            break;
          case 'End':
            e.preventDefault();
            accordionContainer.scrollTo({ top: accordionContainer.scrollHeight, behavior: 'smooth' });
            break;
        }
      }
    });

    // Make accordion container focusable for keyboard scrolling
    (accordionContainer as HTMLElement).tabIndex = -1;
  }

  private testAccordionSections(): void {
    console.log('Testing accordion sections...');
    const sections = document.querySelectorAll('.plot-section');
    sections.forEach((section, index) => {
      const header = section.querySelector('.plot-header');
      const container = section.querySelector('.plot-container');
      const target = header?.getAttribute('data-plot');
      const isExpanded = section.classList.contains('expanded');
      const isCollapsed = section.classList.contains('collapsed');

      console.log(`Section ${index} (${target}):`, {
        expanded: isExpanded,
        collapsed: isCollapsed,
        hasHeader: !!header,
        hasContainer: !!container
      });
    });
  }


  private validateForm(): boolean {
    const activeTab = document.querySelector('.tab-label.active')?.getAttribute('data-tab');

    let isValid = false;
    if (activeTab === 'file') {
      const curveInput = (document.getElementById('curve-path') as HTMLInputElement).value;
      isValid = curveInput.trim() !== '';
    } else if (activeTab === 'api') {
      const speaker = (document.getElementById('speaker') as HTMLInputElement).value;
      const version = (document.getElementById('version') as HTMLInputElement).value;
      const measurement = (document.getElementById('measurement') as HTMLSelectElement).value;
      isValid = speaker.trim() !== '' && version.trim() !== '' && measurement.trim() !== '';
    }

    this.optimizeBtn.disabled = !isValid;
    return isValid;
  }

  private getFormData(): OptimizationParams {
    const activeTab = document.querySelector('.tab-label.active')?.getAttribute('data-tab');
    const algo = (document.getElementById('algo') as HTMLSelectElement).value;

    return {
      num_filters: parseInt((document.getElementById('num-filters') as HTMLInputElement).value),
      curve_path: activeTab === 'file' ? (document.getElementById('curve-path') as HTMLInputElement).value || undefined : undefined,
      target_path: activeTab === 'file' ? (document.getElementById('target-path') as HTMLInputElement).value || undefined : undefined,
      sample_rate: parseFloat((document.getElementById('sample-rate') as HTMLInputElement).value),
      max_db: parseFloat((document.getElementById('max-db') as HTMLInputElement).value),
      min_db: parseFloat((document.getElementById('min-db') as HTMLInputElement).value),
      max_q: parseFloat((document.getElementById('max-q') as HTMLInputElement).value),
      min_q: parseFloat((document.getElementById('min-q') as HTMLInputElement).value),
      min_freq: parseFloat((document.getElementById('min-freq') as HTMLInputElement).value),
      max_freq: parseFloat((document.getElementById('max-freq') as HTMLInputElement).value),
      speaker: activeTab === 'api' ? (document.getElementById('speaker') as HTMLInputElement).value || undefined : undefined,
      version: activeTab === 'api' ? (document.getElementById('version') as HTMLInputElement).value || undefined : undefined,
      measurement: activeTab === 'api' ? (document.getElementById('measurement') as HTMLSelectElement).value || undefined : undefined,
      curve_name: (document.getElementById('curve-name') as HTMLSelectElement).value,
      algo: algo,
      population: parseInt((document.getElementById('population') as HTMLInputElement).value),
      maxeval: parseInt((document.getElementById('maxeval') as HTMLInputElement).value),
      refine: (document.getElementById('refine') as HTMLInputElement).checked,
      local_algo: (document.getElementById('local-algo') as HTMLSelectElement).value,
      min_spacing_oct: parseFloat((document.getElementById('min-spacing-oct') as HTMLInputElement).value),
      spacing_weight: parseFloat((document.getElementById('spacing-weight') as HTMLInputElement).value),
      smooth: (document.getElementById('smooth') as HTMLInputElement).checked,
      smooth_n: parseInt((document.getElementById('smooth-n') as HTMLInputElement).value),
      loss: (document.getElementById('loss') as HTMLSelectElement).value,
      iir_hp_pk: (document.getElementById('iir-hp-pk') as HTMLInputElement).checked,
      // DE parameters (only included if autoeq:de is selected)
      strategy: algo === 'autoeq:de' ? (document.getElementById('strategy') as HTMLSelectElement).value : undefined,
      de_f: algo === 'autoeq:de' ? parseFloat((document.getElementById('de-f') as HTMLInputElement).value) : undefined,
      de_cr: algo === 'autoeq:de' ? parseFloat((document.getElementById('de-cr') as HTMLInputElement).value) : undefined,
      adaptive_weight_f: algo === 'autoeq:de' ? parseFloat((document.getElementById('adaptive-weight-f') as HTMLInputElement).value) : undefined,
      adaptive_weight_cr: algo === 'autoeq:de' ? parseFloat((document.getElementById('adaptive-weight-cr') as HTMLInputElement).value) : undefined,
      // Tolerance parameters
      tolerance: parseFloat((document.getElementById('tolerance') as HTMLInputElement).value),
      atolerance: parseFloat((document.getElementById('abs-tolerance') as HTMLInputElement).value),
    };
  }


  private async runOptimization(): Promise<void> {
    if (!this.validateForm()) {
      return;
    }

    const params = this.getFormData();

    // Conditionally show modal only for algorithms that report progress (autoeq:de)
    const isAutoEQDE = params.algo === 'autoeq:de';
    if (isAutoEQDE) {
      this.showOptimizationModal();
    }
    this.isOptimizationRunning = true;
    this.setOptimizationRunning(true);

    // Clear any previous errors
    this.errorElement.style.display = 'none';

    try {
      // Setup progress listener for DE
      if (isAutoEQDE) {
        try {
          this.progressUnlisten = await listen('progress_update', (event: any) => {
            if (!this.isOptimizationRunning) return;
            const p = event.payload as { iteration: number; fitness: number; params: number[]; convergence: number };
            const details = `iter=${p.iteration}, f=${p.fitness.toFixed(8)}, conv=${p.convergence.toFixed(4)}`;
            this.updateProgress('Optimization', 'running', details);

            // Add data to progress graph
            this.progressGraphData.push({
              iteration: p.iteration,
              fitness: p.fitness,
              convergence: p.convergence
            });

            // Update graph every 5 iterations
            if (p.iteration - this.lastGraphUpdate >= 5 || p.iteration <= 5) {
              this.updateProgressGraph();
              this.lastGraphUpdate = p.iteration;
            }
          });
        } catch (e) {
          console.warn('Failed to attach progress listener:', e);
        }
      }

      // Simulate progress stages for non-DE
      if (!isAutoEQDE) {
        this.updateProgress('Initialization', 'running', 'Loading parameters and validating input', 5);
        await this.sleep(200); // Small delay to show progress
      }

      if (!this.isOptimizationRunning) return; // Check for cancellation

      if (!isAutoEQDE) {
        this.updateProgress('Initialization', 'completed', 'Parameters loaded successfully', 10);
        this.updateProgress('Data Loading', 'running', 'Fetching measurement data', 15);
        await this.sleep(300);
      }

      if (!this.isOptimizationRunning) return;

      if (!isAutoEQDE) {
        this.updateProgress('Data Loading', 'completed', 'Data loaded successfully', 25);
        this.updateProgress('Optimization', 'running', 'Running optimization algorithm', 30);
      }

      const result: OptimizationResult = await invoke('run_optimization', { params });

      if (!this.isOptimizationRunning) return; // Check for cancellation

      if (!isAutoEQDE) {
        this.updateProgress('Optimization', 'completed', 'Algorithm completed', 85);
        this.updateProgress('Results Processing', 'running', 'Processing results and generating plots', 90);
      }

      if (result.success) {
        if (!isAutoEQDE) {
          this.updateProgress('Results Processing', 'completed', 'Results processed successfully', 100);
          this.optimizationCompleted(true, 'Optimization completed successfully');
        } else {
          // For DE, mark as completed directly via modal if it was shown
          this.optimizationCompleted(true, 'Optimization completed successfully');
        }

        // Update UI with results
        this.handleOptimizationSuccess(result);
      } else {
        this.updateProgress('Results Processing', 'error', result.error_message || 'Unknown error', 100);
        this.optimizationCompleted(false, result.error_message || 'Unknown error occurred');
        this.handleOptimizationError(result.error_message || 'Unknown error occurred');
      }
    } catch (error) {
      if (this.isOptimizationRunning) {
        this.updateProgress('Optimization', 'error', `Error: ${error}`, 100);
        this.optimizationCompleted(false, `Error: ${error}`);
        this.handleOptimizationError(error as string);
      }
    } finally {
      this.setOptimizationRunning(false);
      this.showProgress(false);
      if (this.progressUnlisten) {
        try { this.progressUnlisten(); } catch {}
        this.progressUnlisten = undefined;
      }
      if (!isAutoEQDE) {
        this.closeOptimizationModal();
      }
    }
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  private setOptimizationRunning(running: boolean): void {
    this.optimizeBtn.disabled = running;
    this.optimizeBtn.textContent = running ? 'Optimizing...' : 'Run Optimization';
  }

  private updateStatus(message: string): void {
    // Status updates now show in progress text or console
    console.log('Status:', message);
  }

  private showProgress(show: boolean): void {
    this.progressElement.style.display = show ? 'block' : 'none';
  }

  private clearResults(): void {
    console.log('clearResults called');
    this.scoresElement.style.display = 'none';
    this.errorElement.style.display = 'none';
    this.clearAllPlots();
  }

  private handleOptimizationSuccess(result: OptimizationResult): void {
    console.log('Optimization success, result:', result);
    this.updateStatus('Optimization completed successfully!');

    // Update scores if available
    if (result.preference_score_before !== undefined && result.preference_score_after !== undefined) {
      console.log('Updating scores:', result.preference_score_before, '->', result.preference_score_after);
      this.updateScores(result.preference_score_before, result.preference_score_after);
    }

    // Update filter details if available
    if (result.filter_params) {
      console.log('Updating filter details with parameters:', result.filter_params);
      this.updateFilterDetailsPlot(result.filter_params);
      
      // Convert log frequencies to linear for audio filters
      const linearFilterParams = [];
      for (let i = 0; i < result.filter_params.length; i += 3) {
        if (i + 2 < result.filter_params.length) {
          linearFilterParams.push(Math.pow(10, result.filter_params[i])); // Convert log freq to linear
          linearFilterParams.push(result.filter_params[i + 1]); // Q factor
          linearFilterParams.push(result.filter_params[i + 2]); // Gain
        }
      }
      
      // Update audio filter parameters
      this.updateFilterParams(linearFilterParams);
    } else {
      console.log('No filter_params data in result');
    }

    // Update plots if available
    if (result.filter_response) {
      console.log('Updating filter plot with data:', result.filter_response);
      this.updateFilterPlot(result.filter_response);
    } else {
      console.log('No filter_response data in result');
    }

    if (result.spin_details) {
      console.log('Updating individual plots with data:', result.spin_details);
      this.updateOnAxisPlot(result.spin_details, result.filter_response);
      this.updateListeningWindowPlot(result.spin_details, result.filter_response);
      this.updateEarlyReflectionsPlot(result.spin_details, result.filter_response);
      this.updateSoundPowerPlot(result.spin_details, result.filter_response);
      this.updateSpinPlot(result.spin_details);
    } else {
      console.log('No spin_details data in result');
    }
  }

  private handleOptimizationError(error: string): void {
    this.updateStatus('Optimization failed');
    this.showError(error);
  }

  private updateScores(before: number, after: number): void {
    const improvement = after - before;

    (document.getElementById('score-before') as HTMLElement).textContent = before.toFixed(3);
    (document.getElementById('score-after') as HTMLElement).textContent = after.toFixed(3);
    (document.getElementById('score-improvement') as HTMLElement).textContent =
      (improvement >= 0 ? '+' : '') + improvement.toFixed(3);

    this.scoresElement.style.display = 'block';
  }

  private showError(error: string): void {
    (document.getElementById('error-message') as HTMLElement).textContent = error;
    this.errorElement.style.display = 'block';
  }

  private updateFilterDetailsPlot(filterParams: number[]): void {
    if (!this.filterDetailsPlotElement) {
      console.error('Filter details plot element not found!');
      return;
    }

    // Clear and prepare
    this.filterDetailsPlotElement.innerHTML = '';
    this.filterDetailsPlotElement.classList.add('has-plot');
    this.filterDetailsPlotElement.style.display = 'block';
    this.filterDetailsPlotElement.style.padding = '10px';

    // Parse filter parameters (assuming they're in groups of 3: freq, Q, gain)
    const numFilters = Math.floor(filterParams.length / 3);

    // Store original parameters for reference
    this.originalFilterParams = [...filterParams];
    
    // Create interactive table
    this.createInteractiveFilterTable(numFilters, filterParams);

    // Expand the plot section
    this.expandPlotSection('filter-details-plot');

    console.log(`Interactive filter details updated with ${numFilters} filters`);
  }

  private createInteractiveFilterTable(numFilters: number, filterParams: number[]): void {
    const container = document.createElement('div');
    container.style.cssText = `
      max-height: 500px;
      overflow-y: auto;
      overflow-x: hidden;
      border-radius: 8px;
      border: 1px solid var(--border-color);
      padding: 15px;
      font-family: monospace;
    `;

    const table = document.createElement('table');
    table.style.cssText = `
      width: 100%;
      border-collapse: collapse;
      background: var(--bg-secondary);
      border-radius: var(--radius);
    `;

    // Create header
    const thead = document.createElement('thead');
    const headerRow = document.createElement('tr');
    headerRow.style.background = 'var(--bg-accent)';

    const headers = ['Active', 'Filter #', 'Frequency (Hz)', 'Q Factor', 'Gain (dB)', 'Type'];
    headers.forEach(headerText => {
      const th = document.createElement('th');
      th.style.cssText = `
        padding: 12px;
        border: 1px solid var(--border-color);
        color: var(--text-primary);
        font-weight: 600;
        font-size: 12px;
      `;
      th.textContent = headerText;
      headerRow.appendChild(th);
    });
    thead.appendChild(headerRow);
    table.appendChild(thead);

    // Create body
    const tbody = document.createElement('tbody');
    for (let i = 0; i < numFilters; i++) {
      const row = this.createFilterRow(i, filterParams);
      tbody.appendChild(row);
    }
    table.appendChild(tbody);

    container.appendChild(table);
    this.filterDetailsPlotElement.appendChild(container);
  }

  private createFilterRow(index: number, filterParams: number[]): HTMLTableRowElement {
    const row = document.createElement('tr');
    row.style.background = index % 2 === 0 ? 'var(--bg-secondary)' : 'var(--bg-primary)';
    
    const freq = Math.pow(10, filterParams[index * 3]);
    const q = filterParams[index * 3 + 1];
    const gain = filterParams[index * 3 + 2];
    const isActive = Math.abs(gain) > 0.1;
    
    // Active checkbox
    const activeCell = document.createElement('td');
    activeCell.style.cssText = 'padding: 10px; border: 1px solid var(--border-color); text-align: center;';
    const activeCheckbox = document.createElement('input');
    activeCheckbox.type = 'checkbox';
    activeCheckbox.checked = isActive;
    activeCheckbox.id = `filter-active-${index}`;
    activeCheckbox.addEventListener('change', () => this.onFilterActiveChange(index));
    activeCell.appendChild(activeCheckbox);
    row.appendChild(activeCell);
    
    // Filter number
    const numberCell = document.createElement('td');
    numberCell.style.cssText = 'padding: 10px; border: 1px solid var(--border-color); color: var(--text-primary); text-align: center; font-weight: 500;';
    numberCell.textContent = (index + 1).toString();
    row.appendChild(numberCell);
    
    // Frequency input
    const freqCell = document.createElement('td');
    freqCell.style.cssText = 'padding: 5px; border: 1px solid var(--border-color);';
    const freqInput = document.createElement('input');
    freqInput.type = 'number';
    freqInput.value = freq.toFixed(1);
    freqInput.min = '20';
    freqInput.max = '20000';
    freqInput.step = '0.1';
    freqInput.id = `filter-freq-${index}`;
    freqInput.style.cssText = `
      width: 100%;
      border: none;
      background: transparent;
      color: var(--text-primary);
      text-align: right;
      font-family: monospace;
      font-size: 11px;
    `;
    freqInput.addEventListener('change', () => this.onFilterParamChange());
    freqCell.appendChild(freqInput);
    row.appendChild(freqCell);
    
    // Q factor input
    const qCell = document.createElement('td');
    qCell.style.cssText = 'padding: 5px; border: 1px solid var(--border-color);';
    const qInput = document.createElement('input');
    qInput.type = 'number';
    qInput.value = q.toFixed(2);
    qInput.min = '0.1';
    qInput.max = '10';
    qInput.step = '0.01';
    qInput.id = `filter-q-${index}`;
    qInput.style.cssText = `
      width: 100%;
      border: none;
      background: transparent;
      color: var(--text-primary);
      text-align: right;
      font-family: monospace;
      font-size: 11px;
    `;
    qInput.addEventListener('change', () => this.onFilterParamChange());
    qCell.appendChild(qInput);
    row.appendChild(qCell);
    
    // Gain input
    const gainCell = document.createElement('td');
    gainCell.style.cssText = 'padding: 5px; border: 1px solid var(--border-color);';
    const gainInput = document.createElement('input');
    gainInput.type = 'number';
    gainInput.value = gain.toFixed(2);
    gainInput.min = '-20';
    gainInput.max = '20';
    gainInput.step = '0.1';
    gainInput.id = `filter-gain-${index}`;
    gainInput.style.cssText = `
      width: 100%;
      border: none;
      background: transparent;
      color: ${gain > 0 ? 'var(--success-color)' : gain < 0 ? 'var(--danger-color)' : 'var(--text-primary)'};
      text-align: right;
      font-family: monospace;
      font-size: 11px;
      font-weight: 500;
    `;
    gainInput.addEventListener('change', () => {
      this.updateGainInputColor(gainInput, index);
      this.onFilterParamChange();
    });
    gainCell.appendChild(gainInput);
    row.appendChild(gainCell);
    
    // Filter type
    const typeCell = document.createElement('td');
    typeCell.style.cssText = 'padding: 10px; border: 1px solid var(--border-color); color: var(--text-secondary); text-align: center; font-size: 11px;';
    typeCell.textContent = isActive ? 'PK (Peak)' : 'Disabled';
    typeCell.id = `filter-type-${index}`;
    row.appendChild(typeCell);
    
    return row;
  }

  private onFilterActiveChange(index: number): void {
    const activeCheckbox = document.getElementById(`filter-active-${index}`) as HTMLInputElement;
    const gainInput = document.getElementById(`filter-gain-${index}`) as HTMLInputElement;
    const typeCell = document.getElementById(`filter-type-${index}`) as HTMLElement;
    
    if (!activeCheckbox.checked) {
      // Disable filter by setting gain to 0
      gainInput.value = '0.00';
      gainInput.style.color = 'var(--text-primary)';
      typeCell.textContent = 'Disabled';
    } else {
      // Restore original gain if available, or set to 1.0
      const originalGain = this.originalFilterParams[index * 3 + 2] || 1.0;
      gainInput.value = originalGain.toFixed(2);
      gainInput.style.color = originalGain > 0 ? 'var(--success-color)' : originalGain < 0 ? 'var(--danger-color)' : 'var(--text-primary)';
      typeCell.textContent = 'PK (Peak)';
    }
    
    this.onFilterParamChange();
  }

  private onFilterParamChange(): void {
    // Collect all current filter parameters
    const newFilterParams: number[] = [];
    const filterRows = this.filterDetailsPlotElement.querySelectorAll('tbody tr');
    
    filterRows.forEach((row, index) => {
      const freqInput = document.getElementById(`filter-freq-${index}`) as HTMLInputElement;
      const qInput = document.getElementById(`filter-q-${index}`) as HTMLInputElement;
      const gainInput = document.getElementById(`filter-gain-${index}`) as HTMLInputElement;
      
      if (freqInput && qInput && gainInput) {
        newFilterParams.push(parseFloat(freqInput.value));
        newFilterParams.push(parseFloat(qInput.value));
        newFilterParams.push(parseFloat(gainInput.value));
      }
    });
    
    // Update current filter parameters
    this.updateFilterParams(newFilterParams);
    
    // If audio is playing, update EQ in real-time without restarting
    if (this.isAudioPlaying) {
      this.setupEQFilters(); // Recreate filters with new parameters
      this.reconnectAudioChain(); // Reconnect the audio chain
    }
    
    console.log('Filter parameters updated in real-time:', newFilterParams);
  }

  private updateGainInputColor(gainInput: HTMLInputElement, index: number): void {
    const gain = parseFloat(gainInput.value);
    const typeCell = document.getElementById(`filter-type-${index}`) as HTMLElement;
    const activeCheckbox = document.getElementById(`filter-active-${index}`) as HTMLInputElement;
    
    if (Math.abs(gain) <= 0.1) {
      gainInput.style.color = 'var(--text-primary)';
      if (typeCell) typeCell.textContent = 'Disabled';
      if (activeCheckbox) activeCheckbox.checked = false;
    } else {
      gainInput.style.color = gain > 0 ? 'var(--success-color)' : 'var(--danger-color)';
      if (typeCell) typeCell.textContent = 'PK (Peak)';
      if (activeCheckbox) activeCheckbox.checked = true;
    }
  }

  private updateFilterPlot(data: PlotData): void {
    console.log('updateFilterPlot called with:', data);
    console.log('Filter plot element:', this.filterPlotElement);

    if (!this.filterPlotElement) {
      console.error('Filter plot element not found!');
      return;
    }

    // Clear any existing content and prepare for plot
    this.filterPlotElement.innerHTML = '';
    this.filterPlotElement.classList.add('has-plot');
    this.filterPlotElement.style.display = 'block';
    this.filterPlotElement.style.padding = '0';

    const traces = Object.entries(data.curves).map(([name, values]) => ({
      x: data.frequencies,
      y: values,
      type: 'scatter' as const,
      mode: 'lines' as const,
      name: name,
      line: {
        width: name === 'EQ Response' ? 3 : 2
      }
    }));

    console.log('Created traces:', traces);

    // Read current form parameters for axis setup
    const maxDbInput = document.getElementById('max-db') as HTMLInputElement;
    const minFreqInput = document.getElementById('min-freq') as HTMLInputElement;
    const maxFreqInput = document.getElementById('max-freq') as HTMLInputElement;
    const maxDb = maxDbInput ? parseFloat(maxDbInput.value) : 5;
    const minFreq = minFreqInput ? parseFloat(minFreqInput.value) : 20;
    const maxFreq = maxFreqInput ? parseFloat(maxFreqInput.value) : 20000;

    const yMin = -(maxDb + 2);
    const yMax = (maxDb + 2);

    // Always use horizontal legend below the plot for Filter Response
    const legendConfig = {
      x: 0.5,
      y: -0.15,
      xanchor: 'center' as const,
      yanchor: 'top' as const,
      orientation: 'h' as const
    };

    const rightMargin = 20; // Standard right margin
    const bottomMargin = 80; // More space for bottom legend

    const layout = {
      title: { text: '' },
      xaxis: {
        title: { text: 'Frequency (Hz)' },
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: { text: 'Magnitude (dB)' },
        range: [yMin, yMax]
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 12
      },
      margin: { l: 40, r: rightMargin, t: 20, b: bottomMargin },
      showlegend: true,
      legend: {
        ...legendConfig,
        bgcolor: 'rgba(0,0,0,0)'
      },
      shapes: [
        // Left green rectangle: 20 Hz to min_freq
        {
          type: 'rect' as const,
          xref: 'x' as const,
          yref: 'paper' as const,
          x0: 20,
          x1: Math.max(20, minFreq),
          y0: 0,
          y1: 1,
          fillcolor: 'rgba(0, 200, 0, 0.08)',
          line: { width: 0 }
        },
        // Right green rectangle: max_freq to 20 kHz
        {
          type: 'rect' as const,
          xref: 'x' as const,
          yref: 'paper' as const,
          x0: Math.min(maxFreq, 20000),
          x1: 20000,
          y0: 0,
          y1: 1,
          fillcolor: 'rgba(0, 200, 0, 0.08)',
          line: { width: 0 }
        }
      ]
    };

    // Ensure container is visible
    console.log('Container dimensions:', this.filterPlotElement.offsetWidth, 'x', this.filterPlotElement.offsetHeight);

    // Expand the accordion section for this plot
    this.expandPlotSection('filter-plot');

    console.log('Creating Plotly plot immediately');

    Plotly.newPlot(this.filterPlotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log('Filter Plotly plot created successfully');
      // Force immediate resize
      Plotly.Plots.resize(this.filterPlotElement);
    }).catch((error: any) => {
      console.error('Error creating Filter Plotly plot:', error);
    });
  }

  private updateOnAxisPlot(spinData: PlotData, filterData?: PlotData): void {
    this.updateIndividualPlotWithFilter(this.onAxisPlotElement, 'on-axis-plot', spinData, filterData, 'On Axis', ['On Axis']);
  }

  private updateListeningWindowPlot(spinData: PlotData, filterData?: PlotData): void {
    this.updateIndividualPlotWithFilter(this.listeningWindowPlotElement, 'listening-window-plot', spinData, filterData, 'Listening Window', ['Listening Window']);
  }

  private updateEarlyReflectionsPlot(spinData: PlotData, filterData?: PlotData): void {
    this.updateDualAxisPlotWithFilter(this.earlyReflectionsPlotElement, 'early-reflections-plot', spinData, filterData, 'Early Reflections', ['Early Reflections'], 'Early Reflections DI');
  }

  private updateSoundPowerPlot(spinData: PlotData, filterData?: PlotData): void {
    this.updateDualAxisPlotWithFilter(this.soundPowerPlotElement, 'sound-power-plot', spinData, filterData, 'Sound Power', ['Sound Power'], 'Sound Power DI');
  }

  // Plot function for On-Axis and Listening Window with original + optimized curves
  private updateIndividualPlotWithFilter(plotElement: HTMLElement | null, plotId: string, spinData: PlotData, filterData: PlotData | undefined, title: string, curveNames: string[]): void {
    if (!plotElement) {
      console.error(`Plot element not found for ${plotId}`);
      return;
    }

    // Clear and prepare
    plotElement.innerHTML = '';
    plotElement.classList.add('has-plot');
    plotElement.style.display = 'block';
    plotElement.style.padding = '0';

    const traces: any[] = [];

    // Add original measurement curves
    Object.entries(spinData.curves)
      .filter(([name]) => curveNames.some(curveName => name.includes(curveName)))
      .forEach(([name, values]) => {
        traces.push({
          x: spinData.frequencies,
          y: values,
          type: 'scatter' as const,
          mode: 'lines' as const,
          name: `${name} (Original)`,
          line: { width: 2, color: '#1f77b4' }
        });
      });

    // Add optimized curves if available
    if (filterData) {
      // Apply filter to the original curve to show optimized result
      Object.entries(spinData.curves)
        .filter(([name]) => curveNames.some(curveName => name.includes(curveName)))
        .forEach(([name, originalValues]) => {
          // For now, we'll use the EQ Response as the optimization result
          // In a more complete implementation, we'd apply the filter to each curve
          const eqResponse = filterData.curves['EQ Response'];
          if (eqResponse && originalValues.length === eqResponse.length) {
            const optimizedValues = originalValues.map((val, i) => val + eqResponse[i]);
            traces.push({
              x: spinData.frequencies,
              y: optimizedValues,
              type: 'scatter' as const,
              mode: 'lines' as const,
              name: `${name} (Optimized)`,
              line: { width: 2, dash: 'dash' as const, color: '#ff7f0e' }
            });
          }
        });
    }

    if (traces.length === 0) {
      plotElement.innerHTML = `<div style="display: flex; align-items: center; justify-content: center; height: 400px; color: var(--text-secondary);">No ${title} data available</div>`;
      return;
    }

    const layout = {
      title: { text: '' },
      xaxis: {
        title: { text: 'Frequency (Hz)' },
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: { text: 'SPL (dB)' },
        range: [-40, 10]
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 12
      },
      margin: { l: 50, r: 20, t: 20, b: 60 },
      showlegend: true,
      legend: {
        x: 0.5,
        y: -0.15,
        xanchor: 'center' as const,
        yanchor: 'top' as const,
        orientation: 'h' as const,
        bgcolor: 'rgba(0,0,0,0)'
      }
    };

    this.expandPlotSection(plotId);

    Plotly.newPlot(plotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log(`${title} plot created successfully`);
      Plotly.Plots.resize(plotElement);
    }).catch((error: any) => {
      console.error(`Error creating ${title} plot:`, error);
    });
  }

  // Plot function for Early Reflections and Sound Power with dual axes and filter correction
  private updateDualAxisPlotWithFilter(plotElement: HTMLElement | null, plotId: string, spinData: PlotData, filterData: PlotData | undefined, title: string, curveNames: string[], diCurveName: string): void {
    if (!plotElement) {
      console.error(`Plot element not found for ${plotId}`);
      return;
    }

    // Clear and prepare
    plotElement.innerHTML = '';
    plotElement.classList.add('has-plot');
    plotElement.style.display = 'block';
    plotElement.style.padding = '0';

    const traces: any[] = [];

    // Add original measurement curves (left axis) - filter out DI curves
    Object.entries(spinData.curves)
      .filter(([name]) => {
        // Include curves that match curveNames but exclude any DI curves
        const matchesCurveName = curveNames.some(curveName => name.includes(curveName));
        const isDICurve = name.includes('DI') || name.toLowerCase().includes('di');
        return matchesCurveName && !isDICurve;
      })
      .forEach(([name, values]) => {
        traces.push({
          x: spinData.frequencies,
          y: values,
          type: 'scatter' as const,
          mode: 'lines' as const,
          name: `${name} (Original)`,
          yaxis: 'y',
          line: { width: 2, color: '#1f77b4' }
        });
      });

    // Add optimized curves if available (left axis)
    if (filterData) {
      Object.entries(spinData.curves)
        .filter(([name]) => {
          // Include curves that match curveNames but exclude any DI curves
          const matchesCurveName = curveNames.some(curveName => name.includes(curveName));
          const isDICurve = name.includes('DI') || name.toLowerCase().includes('di');
          return matchesCurveName && !isDICurve;
        })
        .forEach(([name, originalValues]) => {
          // Apply filter response to show optimized result
          const eqResponse = filterData.curves['EQ Response'];
          if (eqResponse && originalValues.length === eqResponse.length) {
            const optimizedValues = originalValues.map((val, i) => val + eqResponse[i]);
            traces.push({
              x: spinData.frequencies,
              y: optimizedValues,
              type: 'scatter' as const,
              mode: 'lines' as const,
              name: `${name} (Optimized)`,
              yaxis: 'y',
              line: { width: 2, dash: 'dash' as const, color: '#ff7f0e' }
            });
          }
        });
    }

    // Add DI curve (right axis) - UNCHANGED, no filter correction applied
    Object.entries(spinData.curves)
      .filter(([name]) => {
        // Look for curves that contain the DI curve name (e.g., "Early Reflections DI", "Sound Power DI")
        return name.includes(diCurveName);
      })
      .forEach(([name, values]) => {
        traces.push({
          x: spinData.frequencies,
          y: values,
          type: 'scatter' as const,
          mode: 'lines' as const,
          name: name,
          yaxis: 'y2', // Always use right axis for DI curves
          line: {
            width: 2.5,
            dash: 'dash' as const,
            color: '#d62728' // Different color to distinguish from main curves
          }
        });
      });

    if (traces.length === 0) {
      plotElement.innerHTML = `<div style="display: flex; align-items: center; justify-content: center; height: 400px; color: var(--text-secondary);">No ${title} data available</div>`;
      return;
    }

    const layout = {
      title: { text: '' },
      xaxis: {
        title: { text: 'Frequency (Hz)' },
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: {
          text: 'SPL (dB)',
          font: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
        },
        range: [-40, 10],
        side: 'left' as const,
        tickfont: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
      },
      yaxis2: {
        title: {
          text: 'Directivity Index (dB)',
          font: { color: '#d62728' }
        },
        range: [-5, 45],
        side: 'right' as const,
        overlaying: 'y' as const,
        tickfont: { color: '#d62728' }
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 12
      },
      margin: { l: 50, r: 80, t: 20, b: 80 },
      showlegend: true,
      legend: {
        x: 0.5,
        y: -0.2,
        xanchor: 'center' as const,
        yanchor: 'top' as const,
        orientation: 'h' as const,
        bgcolor: 'rgba(0,0,0,0)'
      },
      hovermode: 'x unified' as const
    };

    this.expandPlotSection(plotId);

    Plotly.newPlot(plotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log(`${title} plot with filter correction created successfully`);
      Plotly.Plots.resize(plotElement);
    }).catch((error: any) => {
      console.error(`Error creating ${title} plot:`, error);
    });
  }

  // Original plot function for Early Reflections and Sound Power with dual axes (kept for compatibility)
  private updateDualAxisPlot(plotElement: HTMLElement | null, plotId: string, data: PlotData, title: string, curveNames: string[], diCurveName: string): void {
    if (!plotElement) {
      console.error(`Plot element not found for ${plotId}`);
      return;
    }

    // Clear and prepare
    plotElement.innerHTML = '';
    plotElement.classList.add('has-plot');
    plotElement.style.display = 'block';
    plotElement.style.padding = '0';

    const traces: any[] = [];

    // Add main measurement curves (left axis) - filter out DI curves
    Object.entries(data.curves)
      .filter(([name]) => {
        // Include curves that match curveNames but exclude any DI curves
        const matchesCurveName = curveNames.some(curveName => name.includes(curveName));
        const isDICurve = name.includes('DI') || name.toLowerCase().includes('di');
        return matchesCurveName && !isDICurve;
      })
      .forEach(([name, values]) => {
        traces.push({
          x: data.frequencies,
          y: values,
          type: 'scatter' as const,
          mode: 'lines' as const,
          name: name,
          yaxis: 'y',
          line: { width: 2 }
        });
      });

    // Add specific DI curve (right axis) - find curves containing diCurveName
    Object.entries(data.curves)
      .filter(([name]) => {
        // Look for curves that contain the DI curve name (e.g., "ERDI", "SPDI")
        return name.includes(diCurveName);
      })
      .forEach(([name, values]) => {
        traces.push({
          x: data.frequencies,
          y: values,
          type: 'scatter' as const,
          mode: 'lines' as const,
          name: name,
          yaxis: 'y2', // Always use right axis for DI curves
          line: {
            width: 2.5,
            dash: 'dash' as const,
            color: '#ff7f0e' // Orange color to match axis
          }
        });
      });

    if (traces.length === 0) {
      plotElement.innerHTML = `<div style="display: flex; align-items: center; justify-content: center; height: 400px; color: var(--text-secondary);">No ${title} data available</div>`;
      return;
    }

    const layout = {
      title: { text: '' },
      xaxis: {
        title: { text: 'Frequency (Hz)' },
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: {
          text: 'SPL (dB)',
          font: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
        },
        range: [-40, 10],
        side: 'left' as const,
        tickfont: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
      },
      yaxis2: {
        title: {
          text: 'Directivity Index (dB)',
          font: { color: '#ff7f0e' }
        },
        range: [-5, 45],
        side: 'right' as const,
        overlaying: 'y' as const,
        tickfont: { color: '#ff7f0e' }
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 12
      },
      margin: { l: 50, r: 80, t: 20, b: 80 },
      showlegend: true,
      legend: {
        x: 0.5,
        y: -0.2,
        xanchor: 'center' as const,
        yanchor: 'top' as const,
        orientation: 'h' as const,
        bgcolor: 'rgba(0,0,0,0)'
      },
      hovermode: 'x unified' as const
    };

    this.expandPlotSection(plotId);

    Plotly.newPlot(plotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log(`${title} plot created successfully`);
      Plotly.Plots.resize(plotElement);
    }).catch((error: any) => {
      console.error(`Error creating ${title} plot:`, error);
    });
  }

  private updateSpinPlot(data: PlotData): void {
    console.log('updateSpinPlot called with:', data);

    if (!this.spinPlotElement) {
      console.error('Spin plot element not found!');
      return;
    }

    // Clear any existing content and prepare for plot
    this.spinPlotElement.innerHTML = '';
    this.spinPlotElement.classList.add('has-plot');
    this.spinPlotElement.style.display = 'block';
    this.spinPlotElement.style.padding = '0';

    const traces = Object.entries(data.curves).map(([name, values]) => {
      // Check if this is a DI curve (Directivity Index)
      const isDICurve = name.toLowerCase().includes('di') ||
                       name.toLowerCase().includes('directivity') ||
                       name === 'Early Reflections DI' ||
                       name === 'Sound Power DI';

      return {
        x: data.frequencies,
        y: values,
        type: 'scatter' as const,
        mode: 'lines' as const,
        name: name,
        yaxis: isDICurve ? 'y2' : 'y', // Use secondary axis for DI curves
        line: {
          width: isDICurve ? 2.5 : 1.5,
          ...(isDICurve ? { dash: 'dash' as const } : {}) // Make DI curves dashed for clarity
        }
      };
    });

    // Always use horizontal legend below plot for Spinorama
    const legendConfig = {
      x: 0.5,
      y: -0.2,
      xanchor: 'center' as const,
      yanchor: 'top' as const,
      orientation: 'h' as const
    };

    const rightMargin = 140; // Space for dual Y-axis
    const bottomMargin = 120; // More space for horizontal legend

    const layout = {
      title: { text: '' },
      xaxis: {
        title: { text: 'Frequency (Hz)' },
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: {
          text: 'SPL (dB)',
          font: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
        },
        range: [-40, 10],
        side: 'left' as const,
        tickfont: { color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim() }
      },
      yaxis2: {
        title: {
          text: 'Directivity Index (dB)',
          font: { color: '#ff7f0e' } // Orange color for DI axis
        },
        range: [-5, 45],
        side: 'right' as const,
        overlaying: 'y' as const,
        tickfont: { color: '#ff7f0e' }
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      font: {
        color: getComputedStyle(document.documentElement).getPropertyValue('--text-primary').trim(),
        size: 12
      },
      margin: { l: 50, r: rightMargin, t: 20, b: bottomMargin },
      showlegend: true,
      legend: {
        ...legendConfig,
        bgcolor: 'rgba(0,0,0,0)'
      },
      hovermode: 'x unified' as const
    };

    // Ensure container is visible
    console.log('Spin container dimensions:', this.spinPlotElement.offsetWidth, 'x', this.spinPlotElement.offsetHeight);

    // Add extra height for Spinorama plot
    this.spinPlotElement.style.minHeight = '500px';

    // Expand the accordion section for this plot
    this.expandPlotSection('spin-plot');

    console.log('Creating Spin Plotly plot immediately');

    Plotly.newPlot(this.spinPlotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log('Spin Plotly plot created successfully');
      // Force immediate resize
      Plotly.Plots.resize(this.spinPlotElement);
    }).catch((error: any) => {
      console.error('Error creating Spin Plotly plot:', error);
    });
  }


  private setupResizer(): void {
    const resizer = document.getElementById('resizer');
    const leftPanel = document.getElementById('left-panel');
    const rightPanel = document.getElementById('right-panel');

    if (!resizer || !leftPanel || !rightPanel) return;

    resizer.addEventListener('mousedown', (e) => {
      this.isResizing = true;
      this.startX = e.clientX;
      this.startWidth = leftPanel.offsetWidth;
      resizer.classList.add('resizing');
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
      if (!this.isResizing) return;

      const diff = e.clientX - this.startX;
      const newWidth = Math.max(280, Math.min(600, this.startWidth + diff));
      document.documentElement.style.setProperty('--left-panel-width', newWidth + 'px');
    });

    document.addEventListener('mouseup', () => {
      if (this.isResizing) {
        this.isResizing = false;
        resizer.classList.remove('resizing');
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      }
    });
  }

  private async setupAutocomplete(): Promise<void> {
    // Load speakers data
    try {
      this.speakers = await invoke('get_speakers') as string[];
    } catch (error) {
      console.error('Failed to load speakers:', error);
    }

    const speakerInput = document.getElementById('speaker') as HTMLInputElement;
    const dropdown = document.getElementById('speaker-dropdown');

    if (!speakerInput || !dropdown) return;

    speakerInput.addEventListener('input', (e) => {
      const query = (e.target as HTMLInputElement).value.toLowerCase();
      this.showSpeakerSuggestions(query);
    });

    speakerInput.addEventListener('focus', () => {
      this.showSpeakerSuggestions(speakerInput.value.toLowerCase());
    });

    document.addEventListener('click', (e) => {
      if (!speakerInput.contains(e.target as Node) && !dropdown.contains(e.target as Node)) {
        dropdown.style.display = 'none';
      }
    });
  }

  private showSpeakerSuggestions(query: string): void {
    const dropdown = document.getElementById('speaker-dropdown');
    if (!dropdown) return;

    const filtered = this.speakers.filter(speaker =>
      speaker.toLowerCase().includes(query)
    ).slice(0, 10); // Limit to 10 results

    if (filtered.length === 0 || (filtered.length === 1 && filtered[0].toLowerCase() === query)) {
      dropdown.style.display = 'none';
      return;
    }

    dropdown.innerHTML = '';
    filtered.forEach(speaker => {
      const item = document.createElement('div');
      item.className = 'autocomplete-item';
      item.textContent = speaker;
      item.addEventListener('click', () => {
        this.selectSpeaker(speaker);
      });
      dropdown.appendChild(item);
    });

    dropdown.style.display = 'block';
  }

  private async selectSpeaker(speaker: string): Promise<void> {
    const speakerInput = document.getElementById('speaker') as HTMLInputElement;
    const versionSelect = document.getElementById('version') as HTMLSelectElement;
    const measurementSelect = document.getElementById('measurement') as HTMLSelectElement;
    const dropdown = document.getElementById('speaker-dropdown');

    speakerInput.value = speaker;
    this.selectedSpeaker = speaker;
    dropdown!.style.display = 'none';

    // Clear plots when speaker changes
    this.clearAllPlots();

    // Reset dependent fields
    versionSelect.innerHTML = '<option value="">Loading versions...</option>';
    versionSelect.disabled = true;
    measurementSelect.innerHTML = '<option value="">Select Measurement</option>';
    measurementSelect.disabled = true;

    try {
      const versions = await invoke('get_versions', { speaker }) as string[];
      versionSelect.innerHTML = '<option value="">Select Version</option>';
      versions.forEach(version => {
        const option = document.createElement('option');
        option.value = version;
        option.textContent = version;
        versionSelect.appendChild(option);
      });

      // Auto-select if only one version available
      if (versions.length === 1) {
        versionSelect.value = versions[0];
        versionSelect.disabled = true;
        versionSelect.style.color = 'var(--text-secondary)';
        this.selectVersion(versions[0]);
      } else {
        versionSelect.disabled = false;
        versionSelect.style.color = 'var(--text-primary)';
      }
    } catch (error) {
      console.error('Failed to load versions:', error);
      versionSelect.innerHTML = '<option value="">Error loading versions</option>';
    }

    this.validateForm();
  }

  private async selectVersion(version: string): Promise<void> {
    const measurementSelect = document.getElementById('measurement') as HTMLSelectElement;
    this.selectedVersion = version;

    // Reset measurement field
    measurementSelect.innerHTML = '<option value="">Loading measurements...</option>';
    measurementSelect.disabled = true;

    try {
      const measurements = await invoke('get_measurements', {
        speaker: this.selectedSpeaker,
        version
      }) as string[];

      measurementSelect.innerHTML = '<option value="">Select Measurement</option>';
      measurements.forEach(measurement => {
        const option = document.createElement('option');
        option.value = measurement;
        option.textContent = measurement;
        measurementSelect.appendChild(option);
      });

      // Smart measurement selection: CEA2034 > Listening Window > none
      let selectedMeasurement = '';
      if (measurements.includes('CEA2034')) {
        selectedMeasurement = 'CEA2034';
      } else if (measurements.includes('Listening Window')) {
        selectedMeasurement = 'Listening Window';
      }

      if (selectedMeasurement) {
        measurementSelect.value = selectedMeasurement;
      }

      measurementSelect.disabled = false;
    } catch (error) {
      console.error('Failed to load measurements:', error);
      measurementSelect.innerHTML = '<option value="">Error loading measurements</option>';
    }

    this.validateForm();
  }

  // Audio Testing Methods
  private setupAudioEventListeners(): void {
    // Listen button
    this.listenBtn.addEventListener('click', () => {
      this.startAudioPlayback();
    });

    // Stop button
    this.stopBtn.addEventListener('click', () => {
      this.stopAudioPlayback();
    });

    // Demo audio selection change
    this.demoAudioSelect.addEventListener('change', () => {
      this.loadAudioFile();
    });

    // EQ toggle buttons
    this.eqOnBtn.addEventListener('click', () => {
      this.setEQEnabled(true);
    });

    this.eqOffBtn.addEventListener('click', () => {
      this.setEQEnabled(false);
    });

    // Initialize audio on page load
    this.loadAudioFile();
  }

  private async initAudioContext(): Promise<void> {
    if (!this.audioContext) {
      try {
        this.audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
        console.log('Audio context initialized:', this.audioContext);
      } catch (error) {
        console.error('Failed to initialize audio context:', error);
        this.showAudioError('Audio not supported in this browser');
      }
    }

    // Resume context if suspended (required for some browsers)
    if (this.audioContext && this.audioContext.state === 'suspended') {
      await this.audioContext.resume();
    }
  }

  private async loadAudioFile(): Promise<void> {
    const selectedAudio = this.demoAudioSelect.value;
    
    // If no track selected, disable controls and clear audio
    if (!selectedAudio) {
      this.audioBuffer = null;
      this.listenBtn.disabled = true;
      this.setAudioStatus('Select a track to begin');
      this.audioDuration.textContent = '--:--';
      return;
    }
    
    const audioUrl = `/demo-audio/${selectedAudio}.wav`;
    this.setAudioStatus('Loading audio file...');
    this.listenBtn.disabled = true;

    try {
      await this.initAudioContext();

      // Fetch audio file
      const response = await fetch(audioUrl);
      if (!response.ok) {
        throw new Error(`Failed to load audio file: ${response.statusText}`);
      }

      const arrayBuffer = await response.arrayBuffer();
      
      // Decode audio data
      this.audioBuffer = await this.audioContext!.decodeAudioData(arrayBuffer);
      
      // Update UI
      const duration = this.formatTime(this.audioBuffer.duration);
      this.audioDuration.textContent = duration;
      this.setAudioStatus('Ready');
      this.listenBtn.disabled = false;
      
      console.log('Audio loaded:', {
        duration: this.audioBuffer.duration,
        sampleRate: this.audioBuffer.sampleRate,
        channels: this.audioBuffer.numberOfChannels
      });
      
    } catch (error) {
      console.error('Failed to load audio:', error);
      this.showAudioError('Failed to load audio file');
      this.listenBtn.disabled = true;
    }
  }

  private async startAudioPlayback(): Promise<void> {
    if (!this.audioContext || !this.audioBuffer) {
      console.error('Audio context or buffer not ready');
      return;
    }

    try {
      // Stop any existing playback
      this.stopAudioPlayback();

      // Create audio source
      this.audioSource = this.audioContext.createBufferSource();
      this.audioSource.buffer = this.audioBuffer;

      // Create gain node for volume control
      this.gainNode = this.audioContext.createGain();
      this.gainNode.gain.value = 0.7; // Reduce volume slightly

      // Connect source to gain
      this.audioSource.connect(this.gainNode);

      // Create and configure EQ filters
      this.setupEQFilters();

      // Connect the audio chain
      let currentNode: AudioNode = this.gainNode;
      
      // Chain EQ filters if enabled
      if (this.eqEnabled && this.eqFilters.length > 0) {
        for (const filter of this.eqFilters) {
          currentNode.connect(filter);
          currentNode = filter;
        }
      }

      // Connect to destination (speakers)
      currentNode.connect(this.audioContext.destination);

      // Set up playback tracking
      this.audioStartTime = this.audioContext.currentTime;
      this.isAudioPlaying = true;
      
      // Handle end of playback
      this.audioSource.onended = () => {
        this.handleAudioEnded();
      };

      // Start playback
      this.audioSource.start(0);
      
      // Update UI
      this.updateAudioControls(true);
      this.setAudioStatus('Playing');
      this.audioStatus.style.display = 'block';
      
      // Start position tracking
      this.startPositionTracking();
      
      console.log('Audio playback started');
      
    } catch (error) {
      console.error('Failed to start audio playback:', error);
      this.showAudioError('Failed to start audio playback');
    }
  }

  private stopAudioPlayback(): void {
    if (this.audioSource) {
      try {
        this.audioSource.stop();
      } catch (error) {
        // Source might already be stopped
      }
      this.audioSource.disconnect();
      this.audioSource = null;
    }

    // Disconnect EQ filters
    this.eqFilters.forEach(filter => {
      try {
        filter.disconnect();
      } catch (error) {
        // Filter might already be disconnected
      }
    });

    if (this.gainNode) {
      try {
        this.gainNode.disconnect();
      } catch (error) {
        // Gain node might already be disconnected
      }
      this.gainNode = null;
    }

    this.isAudioPlaying = false;
    this.stopPositionTracking();
    this.updateAudioControls(false);
    this.setAudioStatus('Stopped');
    this.audioPosition.textContent = '00:00';
    this.audioProgressFill.style.width = '0%';
    
    console.log('Audio playback stopped');
  }

  private setupEQFilters(): void {
    if (!this.audioContext) return;

    // Clear existing filters
    this.eqFilters.forEach(filter => {
      try {
        filter.disconnect();
      } catch (error) {
        // Filter might already be disconnected
      }
    });
    this.eqFilters = [];

    // Get current EQ parameters from the optimization result
    if (this.currentFilterParams.length === 0) {
      console.log('No EQ parameters available, playing without EQ');
      return;
    }

    // Create biquad filters for each EQ band
    // EQ parameters come in groups of 3: [frequency, Q, gain]
    for (let i = 0; i < this.currentFilterParams.length; i += 3) {
      if (i + 2 >= this.currentFilterParams.length) break;

      const frequency = this.currentFilterParams[i];
      const q = this.currentFilterParams[i + 1];
      const gain = this.currentFilterParams[i + 2];

      // Skip filters with very low gain to reduce processing
      if (Math.abs(gain) < 0.1) continue;

      const filter = this.audioContext.createBiquadFilter();
      filter.type = 'peaking';
      filter.frequency.value = frequency;
      filter.Q.value = q;
      filter.gain.value = gain;

      this.eqFilters.push(filter);
      
      console.log(`Created EQ filter: ${frequency.toFixed(1)}Hz, Q=${q.toFixed(2)}, Gain=${gain.toFixed(1)}dB`);
    }

    console.log(`Created ${this.eqFilters.length} EQ filters`);
  }

  private setEQEnabled(enabled: boolean): void {
    this.eqEnabled = enabled;
    
    // Update button states
    this.eqOnBtn.classList.toggle('active', enabled);
    this.eqOffBtn.classList.toggle('active', !enabled);
    
    // Update audio if playing
    this.updateAudioEQ();
  }

  private updateAudioEQ(): void {
    if (!this.isAudioPlaying || !this.audioContext || !this.gainNode) return;

    console.log('EQ toggle changed, updating audio chain in real-time');
    this.reconnectAudioChain();
  }

  private reconnectAudioChain(): void {
    if (!this.audioContext || !this.gainNode) return;

    try {
      // Disconnect current chain
      this.gainNode.disconnect();
      this.eqFilters.forEach(filter => {
        try {
          filter.disconnect();
        } catch (error) {
          // Filter might already be disconnected
        }
      });

      // Rebuild the audio chain
      let currentNode: AudioNode = this.gainNode;

      // Chain EQ filters if enabled
      if (this.eqEnabled && this.eqFilters.length > 0) {
        for (const filter of this.eqFilters) {
          currentNode.connect(filter);
          currentNode = filter;
        }
      }

      // Connect final node to destination
      currentNode.connect(this.audioContext.destination);

      console.log(`Audio chain reconnected with EQ ${this.eqEnabled ? 'enabled' : 'disabled'}`);
    } catch (error) {
      console.error('Failed to reconnect audio chain:', error);
    }
  }

  private handleAudioEnded(): void {
    this.isAudioPlaying = false;
    this.stopPositionTracking();
    this.updateAudioControls(false);
    this.setAudioStatus('Finished');
    
    // Reset progress
    this.audioPosition.textContent = this.audioDuration.textContent || '00:00';
    this.audioProgressFill.style.width = '100%';
    
    // Hide status after a delay
    setTimeout(() => {
      if (!this.isAudioPlaying) {
        this.audioStatus.style.display = 'none';
        this.audioPosition.textContent = '00:00';
        this.audioProgressFill.style.width = '0%';
      }
    }, 2000);
  }

  private startPositionTracking(): void {
    const updatePosition = () => {
      if (!this.isAudioPlaying || !this.audioContext || !this.audioBuffer) {
        return;
      }

      const currentTime = this.audioContext.currentTime - this.audioStartTime;
      const duration = this.audioBuffer.duration;
      const progress = Math.min(currentTime / duration, 1);
      
      this.audioPosition.textContent = this.formatTime(currentTime);
      this.audioProgressFill.style.width = `${progress * 100}%`;
      
      if (this.isAudioPlaying) {
        this.audioAnimationFrame = requestAnimationFrame(updatePosition);
      }
    };
    
    updatePosition();
  }

  private stopPositionTracking(): void {
    if (this.audioAnimationFrame) {
      cancelAnimationFrame(this.audioAnimationFrame);
      this.audioAnimationFrame = null;
    }
  }

  private updateAudioControls(playing: boolean): void {
    this.listenBtn.style.display = playing ? 'none' : 'block';
    this.stopBtn.style.display = playing ? 'block' : 'none';
    this.listenBtn.disabled = !this.audioBuffer;
  }

  private setAudioStatus(status: string): void {
    this.audioStatusText.textContent = status;
  }

  private showAudioError(message: string): void {
    this.setAudioStatus(`Error: ${message}`);
    this.audioStatus.style.display = 'block';
    
    // Hide error after delay
    setTimeout(() => {
      this.audioStatus.style.display = 'none';
    }, 5000);
  }

  private formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  // Update current filter parameters when optimization completes
  private updateFilterParams(filterParams: number[]): void {
    this.currentFilterParams = [...filterParams];
    console.log('Updated EQ parameters:', this.currentFilterParams);
    
    // Auto-switch EQ state based on filter availability
    if (filterParams.length === 0) {
      // No filters available - switch to EQ Off
      this.setEQEnabled(false);
      console.log('No EQ filters available - automatically switched to EQ Off');
    } else {
      // Filters available - enable EQ if not already enabled
      if (!this.eqEnabled) {
        this.setEQEnabled(true);
        console.log('EQ filters loaded - automatically switched to EQ On');
      }
    }
    
    // Enable listen button if we have both audio and EQ parameters
    if (this.audioBuffer && this.currentFilterParams.length > 0) {
      this.listenBtn.disabled = false;
    }
  }
}

// Initialize the application when the DOM is loaded
window.addEventListener('DOMContentLoaded', () => {
  new AutoEQUI();
});
