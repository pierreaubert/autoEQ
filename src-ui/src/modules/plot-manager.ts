// Plot management and rendering functionality

import Plotly from 'plotly.js-dist-min';
import { AutoEQPlotAPI, PlotSpinParams, PlotFiltersParams, PlotUtils, CurveData } from '../types';
import { PlotData } from '../types';

// Progress data interface
interface ProgressData {
  iteration: number;
  fitness: number;
  convergence: number;
  timestamp?: number;
}

export class PlotManager {
  private filterPlotElement: HTMLElement;
  private detailsPlotElement: HTMLElement | null;
  private spinPlotElement: HTMLElement;
  private tonalPlotElement: HTMLElement | null = null;
  private progressGraphElement: HTMLElement | null = null;

  // Plot data caching
  private filterPlotsData: PlotData | null = null;
  private lastSpinDetails: PlotData | null = null;
  private progressData: ProgressData[] = [];

  constructor(
    filterDetailsPlotElement: HTMLElement | null, // Will be ignored - kept for compatibility
    filterPlotElement: HTMLElement,
    detailsPlotElement: HTMLElement | null,
    spinPlotElement: HTMLElement,
    spinPlotCorrectedElement: HTMLElement | null, // Will be ignored - no longer used
    progressGraphElement?: HTMLElement,
    tonalPlotElement?: HTMLElement
  ) {
    this.filterPlotElement = filterPlotElement;
    this.detailsPlotElement = detailsPlotElement;
    this.spinPlotElement = spinPlotElement;
    this.progressGraphElement = progressGraphElement || null;
    this.tonalPlotElement = tonalPlotElement || null;
  }

  clearAllPlots(): void {
    const allPlotElements = [
      this.filterPlotElement,
      this.detailsPlotElement,
      this.spinPlotElement,
      this.tonalPlotElement
    ].filter(Boolean); // Filter out null elements

    // Also clear progress graph
    this.clearProgressGraph();

    try {
      allPlotElements.forEach(element => {
        if (element) {
          // Clear plotly plots if they exist
          try {
            Plotly.purge(element);
          } catch (e) {
            // Element may not have been plotted yet
          }
          element.innerHTML = '<div class="plot-placeholder">No data to display</div>';
          element.classList.remove('has-plot');
        }
      });

      // Hide spinorama vertical items by default
      this.hideSpinVerticalItems();
    } catch (error) {
      console.error('Error clearing plots:', error);
    }
  }

  showSpinVerticalItems(): void {
    const verticalItems = ['spin_vertical_item', 'tonal_vertical_item'];
    verticalItems.forEach(id => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = 'flex';
        console.log(`[VERTICAL DEBUG] Showed vertical item: ${id}`);
      }
    });
  }

  hideSpinVerticalItems(): void {
    const verticalItems = ['spin_vertical_item', 'tonal_vertical_item'];
    verticalItems.forEach(id => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = 'none';
        console.log(`[VERTICAL DEBUG] Hid vertical item: ${id}`);
      }
    });
  }

  showPlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
      'spin_plot': 'spin_vertical_item',
      'tonal_plot': 'tonal_vertical_item'
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = 'flex';
        console.log(`[VERTICAL DEBUG] Showed plot container: ${plotId} via vertical item ${verticalItemId}`);
      }
    }
  }

  hidePlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
      'spin_plot': 'spin_vertical_item',
      'tonal_plot': 'tonal_vertical_item'
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = 'none';
        console.log(`[VERTICAL DEBUG] Hid plot container: ${plotId} via vertical item ${verticalItemId}`);
      }
    }
  }

  // Removed expandPlotSection - no longer needed with grid layout

  async tryUpdateDetailsPlot(): Promise<void> {
    if (!this.lastSpinDetails) {
      return;
    }

    try {
      await this.generateDetailsPlot(this.lastSpinDetails);
    } catch (error) {
      console.error('Error updating details plot:', error);
    }
  }

  async generateDetailsPlot(spinDetails: PlotData): Promise<void> {
    const cea2034Curves: { [key: string]: CurveData } = {};
    for (const [curveName, curveData] of Object.entries(spinDetails.curves)) {
      cea2034Curves[curveName] = {
        freq: spinDetails.frequencies,
        spl: curveData
      };
    }

    try {
      const params: PlotSpinParams = { cea2034_curves: cea2034Curves };
      const plotData = await AutoEQPlotAPI.generatePlotSpinDetails(params);

      const customLayout = {
        ...PlotUtils.createResponsiveLayout(),
        title: 'Detailed CEA2034 Analysis',
        paper_bgcolor: 'rgba(0,0,0,0)',
        plot_bgcolor: 'rgba(0,0,0,0)',
        font: { color: '#333', size: 12 },
        margin: { l: 60, r: 40, t: 60, b: 60 }
      };

      const finalPlotData = PlotUtils.applyUILayout(plotData, customLayout);
      const config = PlotUtils.createDefaultConfig();

      if (this.detailsPlotElement) {
        await Plotly.newPlot(this.detailsPlotElement, finalPlotData.data, finalPlotData.layout, config);
        this.detailsPlotElement.classList.add('has-plot');
        this.showPlotContainer('details_plot');
      }

      console.log('Details plot generated successfully');
    } catch (error) {
      console.error('Error generating details plot:', error);
    }
  }

  updateSpinPlot(data: PlotData): void {
    console.log('updateSpinPlot called with:', data);

    if (!this.spinPlotElement) {
      console.error('Spin plot element not found!');
      return;
    }

    // Clear any existing content and prepare for plot
    this.spinPlotElement.innerHTML = '';
    this.spinPlotElement.classList.add('has-plot');

    const traces = Object.entries(data.curves).map(([name, values]) => ({
      x: data.frequencies,
      y: values,
      type: 'scatter' as const,
      mode: 'lines' as const,
      name: name,
      line: {
        width: name === 'On Axis' ? 3 : 2
      }
    }));

    console.log('Created traces:', traces);

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

    Plotly.newPlot(this.spinPlotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log('Spin Plotly plot created successfully');
      Plotly.Plots.resize(this.spinPlotElement);
    }).catch((error: any) => {
      console.error('Error creating Spin Plotly plot:', error);
    });
  }

  updateFilterPlot(data: PlotData): void {
    console.log('[TS DEBUG] updateFilterPlot called with:', data);

    if (!this.filterPlotElement) {
      console.error('Filter plot element not found!');
      return;
    }

    // Clear any existing content and prepare for plot
    this.filterPlotElement.innerHTML = '';
    this.filterPlotElement.classList.add('has-plot');

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
    const maxDbInput = document.getElementById('max_db') as HTMLInputElement;
    const minFreqInput = document.getElementById('min_freq') as HTMLInputElement;
    const maxFreqInput = document.getElementById('max_freq') as HTMLInputElement;
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

    console.log('[TS DEBUG] Creating Plotly plot with traces:', traces.length);
    console.log('[TS DEBUG] Layout:', layout);

    Plotly.newPlot(this.filterPlotElement, traces, layout, {
      responsive: true,
      displayModeBar: false
    }).then(() => {
      console.log('[TS DEBUG] Filter Plotly plot created successfully');
      // Force immediate resize
      Plotly.Plots.resize(this.filterPlotElement);
      console.log('[TS DEBUG] Plot resized');
    }).catch((error: any) => {
      console.error('[TS DEBUG] Error creating Filter Plotly plot:', error);
    });
  }

  configureVerticalVisibility(hasSpinData: boolean): void {
    console.log('[VERTICAL DEBUG] Configuring vertical visibility, hasSpinData:', hasSpinData);

    if (hasSpinData) {
      // Speaker-based: show all 3 graphs (Filter Response + 2 spinorama graphs)
      console.log('[VERTICAL DEBUG] Showing all graphs for speaker-based optimization');
      this.showSpinVerticalItems();
    } else {
      // Curve+target: only show Filter Response graph
      console.log('[VERTICAL DEBUG] Showing only Filter Response for curve+target optimization');
      this.hideSpinVerticalItems();
    }
  }

  configureGridVisibility(hasSpinData: boolean): void {
    this.configureVerticalVisibility(hasSpinData);
  }

  // Deprecated method - kept for compatibility
  configureAccordionVisibility(hasSpinData: boolean): void {
    this.configureGridVisibility(hasSpinData);
  }

  // Getters for cached data
  getLastSpinDetails(): PlotData | null {
    return this.lastSpinDetails;
  }

  setLastSpinDetails(data: PlotData | null): void {
    this.lastSpinDetails = data;
  }

  getFilterPlotsData(): PlotData | null {
    return this.filterPlotsData;
  }

  setFilterPlotsData(data: PlotData | null): void {
    this.filterPlotsData = data;
  }

  // Progress graph methods
  clearProgressGraph(): void {
    if (this.progressGraphElement) {
      try {
        Plotly.purge(this.progressGraphElement);
      } catch (e) {
        // Element may not have been plotted yet
      }
      this.progressGraphElement.innerHTML = '';
      // Add a placeholder to show the element exists
      this.progressGraphElement.innerHTML = '<div style="text-align: center; padding: 20px; color: #666;">Waiting for optimization data...</div>';
    }
    this.progressData = [];
    console.log('[PLOT DEBUG] Progress graph cleared and reset');
  }

  addProgressData(iteration: number, fitness: number, convergence: number): void {
    console.log(`[PLOT DEBUG] Adding progress data: iteration=${iteration}, fitness=${fitness}, convergence=${convergence}`);
    this.progressData.push({
      iteration,
      fitness,
      convergence,
      timestamp: Date.now()
    });
    console.log(`[PLOT DEBUG] Progress data array now has ${this.progressData.length} entries`);
  }

  async updateProgressGraph(): Promise<void> {
    console.log(`[PLOT DEBUG] updateProgressGraph called, element exists: ${!!this.progressGraphElement}, data length: ${this.progressData.length}`);
    if (!this.progressGraphElement) {
      console.error('[PLOT DEBUG] Progress graph element not found!');
      return;
    }
    if (this.progressData.length === 0) {
      console.log('[PLOT DEBUG] No progress data to plot yet');
      return;
    }

    const iterations = this.progressData.map(d => d.iteration);
    const fitness = this.progressData.map(d => d.fitness);
    const convergence = this.progressData.map(d => d.convergence);

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
      console.log('[PLOT DEBUG] Creating/updating progress graph with Plotly');
      console.log('[PLOT DEBUG] Fitness data:', fitness.slice(0, 5), '...');
      console.log('[PLOT DEBUG] Convergence data:', convergence.slice(0, 5), '...');

      // Clear placeholder text
      if (this.progressGraphElement.innerHTML.includes('Waiting for optimization data')) {
        this.progressGraphElement.innerHTML = '';
      }

      if (this.progressGraphElement.hasChildNodes() && this.progressGraphElement.children.length > 0) {
        // Update existing plot
        console.log('[PLOT DEBUG] Updating existing plot');
        await Plotly.react(this.progressGraphElement, [fitnessTrace, convergenceTrace], layout, config);
      } else {
        // Create new plot
        console.log('[PLOT DEBUG] Creating new plot');
        await Plotly.newPlot(this.progressGraphElement, [fitnessTrace, convergenceTrace], layout, config);
      }
      console.log('[PLOT DEBUG] ✅ Progress graph updated successfully with', this.progressData.length, 'data points');
    } catch (error) {
      console.error('[PLOT DEBUG] ❌ Error updating progress graph:', error);
      // Add error message to the element
      this.progressGraphElement.innerHTML = `<div style="text-align: center; padding: 20px; color: #dc3545;">Error creating progress graph: ${error}</div>`;
    }
  }

  getProgressData(): ProgressData[] {
    return [...this.progressData];
  }

  // Filter details plot methods - removed in grid layout
  async updateFilterDetailsPlot(optimizationResult: any): Promise<void> {
    console.log('[PLOT DEBUG] Filter details plot functionality removed in grid layout');
    // This method is deprecated but kept for compatibility
  }

  // Tonal balance plot methods
  updateTonalPlot(plotData: any): void {
    if (!this.tonalPlotElement) {
      console.warn('[PLOT DEBUG] Tonal plot element not available');
      return;
    }

    console.log('[PLOT DEBUG] Updating tonal balance plot with data:', plotData);

    try {
      if (plotData && plotData.data && plotData.layout) {
        // Use the plot data directly from backend
        Plotly.newPlot(this.tonalPlotElement, plotData.data, plotData.layout, {
          responsive: true,
          displayModeBar: false
        }).then(() => {
          console.log('[PLOT DEBUG] ✅ Tonal balance plot created successfully');
          this.tonalPlotElement!.classList.add('has-plot');
          this.showPlotContainer('tonal_plot');
        }).catch((error: any) => {
          console.error('[PLOT DEBUG] ❌ Error creating tonal balance plot:', error);
        });
      } else {
        console.warn('[PLOT DEBUG] Invalid tonal plot data structure:', plotData);
      }
    } catch (error) {
      console.error('[PLOT DEBUG] Error updating tonal balance plot:', error);
    }
  }
}
