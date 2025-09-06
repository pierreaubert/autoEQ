import { invoke } from "@tauri-apps/api/core";
import { open } from '@tauri-apps/plugin-opener';
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
  private exitBtn: HTMLButtonElement;
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

  constructor() {
    this.form = document.getElementById('autoeq-form') as HTMLFormElement;
    this.optimizeBtn = document.getElementById('optimize-btn') as HTMLButtonElement;
    this.resetBtn = document.getElementById('reset-btn') as HTMLButtonElement;
    this.exitBtn = document.getElementById('exit-btn') as HTMLButtonElement;
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

    this.setupEventListeners();
    this.setupUIInteractions();
    this.setupResizer();
    this.setupAutocomplete();
    this.resetToDefaults();
    this.updateConditionalParameters();
    
    // Add test plots for debugging (remove in production)
    setTimeout(() => {
      this.createAllTestPlots();
    }, 1000);
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

    // Exit button
    this.exitBtn.addEventListener('click', async () => {
      await invoke('exit_app');
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
    });
  }

  private async openFileDialog(inputId: string): Promise<void> {
    console.log('openFileDialog called for:', inputId);
    try {
      const input = document.getElementById(inputId) as HTMLInputElement;
      console.log('Input element found:', input);
      
      console.log('Opening file dialog...');
      const result = await openDialog({
        multiple: false,
        filters: [{
          name: 'CSV Files',
          extensions: ['csv']
        }, {
          name: 'All Files',
          extensions: ['*']
        }]
      });
      
      console.log('Dialog result:', result);
      if (result) {
        console.log('Setting input value to:', result);
        input.value = result;
        this.validateForm();
      } else {
        console.log('No file selected or dialog cancelled');
      }
    } catch (error) {
      console.error('Error opening file dialog:', error);
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
    (document.getElementById('algo') as HTMLSelectElement).value = 'isres';
    (document.getElementById('loss') as HTMLSelectElement).value = 'flat';
    (document.getElementById('population') as HTMLInputElement).value = '30000';
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

    // Update conditional parameters
    this.updateConditionalParameters();

    // Clear only errors on reset, keep existing plots
    this.errorElement.style.display = 'none';
    this.scoresElement.style.display = 'none';
    this.validateForm();
  }

  private updateConditionalParameters(): void {
    const algo = (document.getElementById('algo') as HTMLSelectElement).value;
    const globalAlgoParams = document.querySelectorAll('.global-algo-param');
    
    // Show population and maxeval only for global algorithms
    const isGlobalAlgo = ['isres', 'de', 'pso', 'stogo', 'ags', 'origdirect'].includes(algo);
    
    globalAlgoParams.forEach(param => {
      if (isGlobalAlgo) {
        param.classList.add('show');
      } else {
        param.classList.remove('show');
      }
    });
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
  
  private setupAccordionBehavior(): void {
    console.log('Setting up accordion behavior');
    const plotHeaders = document.querySelectorAll('.plot-header');
    console.log('Found plot headers:', plotHeaders.length);
    
    plotHeaders.forEach((header, index) => {
      console.log('Setting up header', index, header);
      header.addEventListener('click', (e) => {
        e.preventDefault();
        const target = header.getAttribute('data-plot');
        const section = header.parentElement as HTMLElement;
        const arrow = header.querySelector('.accordion-arrow');
        
        console.log('Accordion header clicked:', target);
        
        if (section && arrow) {
          const isExpanded = section.classList.contains('expanded');
          console.log('Current state - expanded:', isExpanded);
          
          if (isExpanded) {
            // Collapse
            console.log('Collapsing section:', target);
            section.classList.remove('expanded');
            section.classList.add('collapsed');
            arrow.textContent = '▶';
          } else {
            // Expand
            console.log('Expanding section:', target);
            section.classList.remove('collapsed');
            section.classList.add('expanded');
            arrow.textContent = '▼';
          }
        }
      });
    });
    
    // Test accordion sections on startup
    this.testAccordionSections();
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
  
  private createTestPlot(containerElement: HTMLElement, title: string): void {
    console.log('Creating test plot in:', containerElement.id);
    
    // Clear and prepare container
    containerElement.innerHTML = '';
    containerElement.classList.add('has-plot');
    containerElement.style.display = 'block';
    containerElement.style.padding = '0';
    
    // Expand the section
    this.expandPlotSection(containerElement.id);
    
    // Create simple test data
    const x = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
    const y = [0, -1, 1, -0.5, 0.5, 0, -0.3, 0.8, -0.2, 0];
    
    const trace = {
      x: x,
      y: y,
      type: 'scatter' as const,
      mode: 'lines' as const,
      name: title,
      line: { width: 2, color: '#007bff' }
    };
    
    const layout = {
      title: title,
      xaxis: {
        title: 'Frequency (Hz)',
        type: 'log' as const
      },
      yaxis: {
        title: 'Magnitude (dB)'
      },
      paper_bgcolor: 'rgba(0,0,0,0)',
      plot_bgcolor: 'rgba(0,0,0,0)',
      margin: { l: 40, r: 20, t: 40, b: 40 }
    };
    
    Plotly.newPlot(containerElement, [trace], layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log('Test plot created successfully in:', containerElement.id);
    }).catch((error) => {
      console.error('Error creating test plot:', error);
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
      algo: (document.getElementById('algo') as HTMLSelectElement).value,
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
    };
  }
  
  private createAllTestPlots(): void {
    console.log('Creating test plots in all containers');
    
    // Create test plots for all containers
    if (this.filterDetailsPlotElement) {
      this.createTestPlot(this.filterDetailsPlotElement, 'Filter Details Test');
    }
    
    if (this.filterPlotElement) {
      this.createTestPlot(this.filterPlotElement, 'Filter Response Test');
    }
    
    if (this.onAxisPlotElement) {
      this.createTestPlot(this.onAxisPlotElement, 'On Axis Test');
    }
    
    if (this.listeningWindowPlotElement) {
      this.createTestPlot(this.listeningWindowPlotElement, 'Listening Window Test');
    }
    
    if (this.earlyReflectionsPlotElement) {
      this.createTestPlot(this.earlyReflectionsPlotElement, 'Early Reflections Test');
    }
    
    if (this.soundPowerPlotElement) {
      this.createTestPlot(this.soundPowerPlotElement, 'Sound Power Test');
    }
    
    if (this.spinPlotElement) {
      this.createTestPlot(this.spinPlotElement, 'Spinorama Test');
    }
  }

  private async runOptimization(): Promise<void> {
    if (!this.validateForm()) {
      return;
    }

    const params = this.getFormData();
    
    this.setOptimizationRunning(true);
    // Only clear errors, not plots
    this.errorElement.style.display = 'none';
    this.updateStatus('Running optimization...');
    this.showProgress(true);

    try {
      const result: OptimizationResult = await invoke('run_optimization', { params });
      
      if (result.success) {
        this.handleOptimizationSuccess(result);
      } else {
        this.handleOptimizationError(result.error_message || 'Unknown error occurred');
      }
    } catch (error) {
      this.handleOptimizationError(error as string);
    } finally {
      this.setOptimizationRunning(false);
      this.showProgress(false);
    }
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
    this.clearPlots();
  }

  private handleOptimizationSuccess(result: OptimizationResult): void {
    console.log('Optimization success, result:', result);
    this.updateStatus('Optimization completed successfully!');
    
    // Update scores if available
    if (result.preference_score_before !== undefined && result.preference_score_after !== undefined) {
      console.log('Updating scores:', result.preference_score_before, '->', result.preference_score_after);
      this.updateScores(result.preference_score_before, result.preference_score_after);
    }

    // Update plots if available
    if (result.filter_response) {
      console.log('Updating filter plot with data:', result.filter_response);
      this.updateFilterPlot(result.filter_response);
    } else {
      console.log('No filter_response data in result');
    }
    
    if (result.spin_details) {
      console.log('Updating spin plot with data:', result.spin_details);
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

    // Determine legend position based on screen size
    const isWideScreen = window.innerWidth > 768;
    const legendConfig = isWideScreen ? {
      x: 1.02,
      y: 1,
      xanchor: 'left' as const,
      yanchor: 'top' as const
    } : {
      x: 0.5,
      y: -0.1,
      xanchor: 'center' as const,
      yanchor: 'top' as const,
      orientation: 'h' as const
    };
    
    const rightMargin = isWideScreen ? 120 : 20; // More space for right legend
    const bottomMargin = isWideScreen ? 40 : 80; // More space for bottom legend

    const layout = {
      title: '',
      xaxis: {
        title: 'Frequency (Hz)',
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: 'Magnitude (dB)',
        range: [-5, 5]
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
      }
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
    }).catch((error) => {
      console.error('Error creating Filter Plotly plot:', error);
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
      const isDICurve = name.toLowerCase().includes('di') || name.toLowerCase().includes('directivity');
      
      return {
        x: data.frequencies,
        y: values,
        type: 'scatter' as const,
        mode: 'lines' as const,
        name: name,
        yaxis: isDICurve ? 'y2' : 'y', // Use secondary axis for DI curves
        line: {
          width: isDICurve ? 2 : 1.5
        }
      };
    });

    // Determine legend position based on screen size
    const isWideScreen = window.innerWidth > 768;
    const legendConfig = isWideScreen ? {
      x: 1.02,
      y: 1,
      xanchor: 'left' as const,
      yanchor: 'top' as const
    } : {
      x: 0.5,
      y: -0.1,
      xanchor: 'center' as const,
      yanchor: 'top' as const,
      orientation: 'h' as const
    };
    
    const rightMargin = isWideScreen ? 120 : 20; // More space for right legend
    const bottomMargin = isWideScreen ? 40 : 80; // More space for bottom legend

    const layout = {
      title: '',
      xaxis: {
        title: 'Frequency (Hz)',
        type: 'log' as const,
        range: [Math.log10(20), Math.log10(20000)]
      },
      yaxis: {
        title: 'SPL (dB)',
        range: [-10, 10],
        side: 'left'
      },
      yaxis2: {
        title: 'Directivity Index (dB)',
        range: [-5, 45],
        side: 'right',
        overlaying: 'y'
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
      }
    };

    // Ensure container is visible
    console.log('Spin container dimensions:', this.spinPlotElement.offsetWidth, 'x', this.spinPlotElement.offsetHeight);
    
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
    }).catch((error) => {
      console.error('Error creating Spin Plotly plot:', error);
    });
  }

  private clearPlots(): void {
    console.log('clearPlots called');
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
          Plotly.purge(element);
          element.classList.remove('has-plot');
        }
      });
    } catch (e) {
      console.log('No existing plots to purge');
    }
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
}

// Initialize the application when the DOM is loaded
window.addEventListener('DOMContentLoaded', () => {
  new AutoEQUI();
});
