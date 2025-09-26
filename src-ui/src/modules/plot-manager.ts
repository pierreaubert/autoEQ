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
  private filterDetailsPlotElement: HTMLElement;
  private filterPlotElement: HTMLElement;
  private detailsPlotElement: HTMLElement;
  private spinPlotElement: HTMLElement;
  private spinPlotCorrectedElement: HTMLElement;
  private tonalPlotElement: HTMLElement | null = null;
  private progressGraphElement: HTMLElement | null = null;

  // Plot data caching
  private filterPlotsData: PlotData | null = null;
  private lastSpinDetails: PlotData | null = null;
  private progressData: ProgressData[] = [];

  constructor(
    filterDetailsPlotElement: HTMLElement,
    filterPlotElement: HTMLElement,
    detailsPlotElement: HTMLElement,
    spinPlotElement: HTMLElement,
    spinPlotCorrectedElement: HTMLElement,
    progressGraphElement?: HTMLElement,
    tonalPlotElement?: HTMLElement
  ) {
    this.filterDetailsPlotElement = filterDetailsPlotElement;
    this.filterPlotElement = filterPlotElement;
    this.detailsPlotElement = detailsPlotElement;
    this.spinPlotElement = spinPlotElement;
    this.spinPlotCorrectedElement = spinPlotCorrectedElement;
    this.progressGraphElement = progressGraphElement || null;
    this.tonalPlotElement = tonalPlotElement || null;
  }

  clearAllPlots(): void {
    const allPlotElements = [
      this.filterDetailsPlotElement,
      this.filterPlotElement,
      this.detailsPlotElement,
      this.spinPlotElement,
      this.spinPlotCorrectedElement
    ];

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
          element.innerHTML = '';
          element.classList.remove('has-plot');
          element.style.display = 'none';
        }
      });
    } catch (error) {
      console.error('Error clearing plots:', error);
    }
  }

  showPlotContainer(plotId: string): void {
    const element = document.getElementById(plotId) as HTMLElement;
    if (element) {
      // Show the plot element itself
      element.style.display = 'block';

      // Find and show the parent accordion/container
      const container = element.closest('.plot-section, .accordion-item, .plot-container') as HTMLElement;
      if (container) {
        container.style.display = 'block';
      }

      // Find and show the header if it exists
      const header = document.querySelector(`[data-target="${plotId}"], .plot-header[onclick*="${plotId}"]`) as HTMLElement;
      if (header) {
        header.style.display = 'block';
        const headerContainer = header.closest('.plot-section, .accordion-item') as HTMLElement;
        if (headerContainer) {
          headerContainer.style.display = 'block';
        }
      }

      console.log(`[TS DEBUG] Showed plot container: ${plotId}`);
    } else {
      console.warn(`[TS DEBUG] Plot element not found: ${plotId}`);
    }
  }

  hidePlotContainer(plotId: string): void {
    const element = document.getElementById(plotId) as HTMLElement;
    if (element) {
      // Hide the plot element itself
      element.style.display = 'none';

      // Find and hide the parent accordion/container
      const container = element.closest('.plot-section, .accordion-item, .plot-container') as HTMLElement;
      if (container) {
        container.style.display = 'none';
      }

      // Find and hide the header if it exists
      const header = document.querySelector(`[data-target="${plotId}"], .plot-header[onclick*="${plotId}"]`) as HTMLElement;
      if (header) {
        header.style.display = 'none';
        const headerContainer = header.closest('.plot-section, .accordion-item') as HTMLElement;
        if (headerContainer) {
          headerContainer.style.display = 'none';
        }
      }

      console.log(`[TS DEBUG] Hid plot container: ${plotId}`);
    } else {
      console.warn(`[TS DEBUG] Plot element not found: ${plotId}`);
    }
  }

  expandPlotSection(plotElementId: string): void {
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
        this.showPlotContainer('details_plot');
        this.expandPlotSection('details_plot');
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
    this.spinPlotElement.style.display = 'block';
    this.spinPlotElement.style.padding = '0';

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

    this.expandPlotSection('spin_plot');

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

    // Expand the accordion section for this plot
    this.expandPlotSection('filter-plot');

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

  configureAccordionVisibility(hasSpinData: boolean): void {
    console.log('[TS DEBUG] Configuring accordion visibility, hasSpinData:', hasSpinData);

    if (hasSpinData) {
      // Speaker-based: show spinorama sections, hide response curve
      console.log('[TS DEBUG] Showing spinorama sections for speaker-based optimization');
      this.showPlotContainer('details_plot');
      this.showPlotContainer('spin_plot');
      this.hidePlotContainer('tonal_plot');
    } else {
      // Curve+target: hide spinorama sections, show response curve
      console.log('[TS DEBUG] Hiding spinorama sections for curve+target optimization');
      this.hidePlotContainer('details_plot');
      this.hidePlotContainer('spin_plot');
      this.hidePlotContainer('tonal_plot');
    }
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

  // Filter details plot methods
  async updateFilterDetailsPlot(optimizationResult: any): Promise<void> {
    if (!this.filterDetailsPlotElement) {
      console.warn('[PLOT DEBUG] Filter details plot element not available');
      return;
    }

    console.log('[PLOT DEBUG] Updating filter details with optimization result:', optimizationResult);

    try {
      // Update the filter details table first
      if (optimizationResult.filter_params) {
        this.updateFilterDetailsTable(optimizationResult.filter_params);
      }

      // Generate the filter plot using Tauri backend if we have the required data
      if (optimizationResult.input_curve && optimizationResult.target_curve && optimizationResult.filter_params) {
        console.log('[PLOT DEBUG] Calling generate_plot_filters via Tauri backend');

        const plotParams: PlotFiltersParams = {
          input_curve: optimizationResult.input_curve,
          target_curve: optimizationResult.target_curve,
          deviation_curve: optimizationResult.deviation_curve || optimizationResult.input_curve, // fallback
          optimized_params: optimizationResult.filter_params,
          sample_rate: optimizationResult.sample_rate || 44100,
          num_filters: optimizationResult.filter_params.length,
          iir_hp_pk: optimizationResult.iir_hp_pk || false
        };

        const filterPlotData = await AutoEQPlotAPI.generatePlotFilters(plotParams);
        console.log('[PLOT DEBUG] Generated filter plot data:', filterPlotData);

        // Update the filter details graph
        const graphElement = document.getElementById('filter_details_graph');
        if (graphElement && filterPlotData && filterPlotData.data && filterPlotData.layout) {
          await Plotly.newPlot(graphElement, filterPlotData.data, filterPlotData.layout, {
            responsive: true,
            displayModeBar: false
          });
          console.log('[PLOT DEBUG] ✅ Filter details plot created successfully');
          this.showPlotContainer('filter_details_plot');
          this.expandPlotSection('filter_details_plot');
        }
      } else {
        console.warn('[PLOT DEBUG] Missing required data for filter plot generation. Available keys:', Object.keys(optimizationResult));
        // Still show the table even if we can't generate the plot
        if (optimizationResult.filter_params) {
          this.showPlotContainer('filter_details_plot');
          this.expandPlotSection('filter_details_plot');
        }
      }
    } catch (error) {
      console.error('[PLOT DEBUG] ❌ Error updating filter details plot:', error);
      // Show error in the table
      const tableElement = document.getElementById('filter_details_table');
      if (tableElement) {
        tableElement.innerHTML = `<div style="color: #dc3545; padding: 10px;">Error generating filter details: ${error}</div>`;
      }
    }
  }

  private updateFilterDetailsTable(filterParams: any[]): void {
    const tableElement = document.getElementById('filter_details_table');
    if (!tableElement) {
      console.warn('[PLOT DEBUG] Filter details table element not found');
      return;
    }

    if (!filterParams || filterParams.length === 0) {
      tableElement.innerHTML = '<div style="padding: 10px;">No filter parameters to display</div>';
      return;
    }

    // Create table HTML
    let tableHTML = `
      <table style="width: 100%; border-collapse: collapse; margin: 10px 0;">
        <thead>
          <tr style="background-color: #f5f5f5;">
            <th style="border: 1px solid #ddd; padding: 8px; text-align: left;">Filter</th>
            <th style="border: 1px solid #ddd; padding: 8px; text-align: right;">Frequency (Hz)</th>
            <th style="border: 1px solid #ddd; padding: 8px; text-align: right;">Q Factor</th>
            <th style="border: 1px solid #ddd; padding: 8px; text-align: right;">Gain (dB)</th>
          </tr>
        </thead>
        <tbody>
    `;

    filterParams.forEach((param, index) => {
      const frequency = param.frequency || param.freq || 0;
      const q = param.q || param.Q || 1;
      const gain = param.gain || 0;

      tableHTML += `
        <tr>
          <td style="border: 1px solid #ddd; padding: 8px;">Filter ${index + 1}</td>
          <td style="border: 1px solid #ddd; padding: 8px; text-align: right;">${frequency.toFixed(1)}</td>
          <td style="border: 1px solid #ddd; padding: 8px; text-align: right;">${q.toFixed(2)}</td>
          <td style="border: 1px solid #ddd; padding: 8px; text-align: right;">${gain > 0 ? '+' : ''}${gain.toFixed(2)}</td>
        </tr>
      `;
    });

    tableHTML += `
        </tbody>
      </table>
    `;

    tableElement.innerHTML = tableHTML;
    console.log('[PLOT DEBUG] ✅ Filter details table updated with', filterParams.length, 'filters');
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
          this.showPlotContainer('tonal_plot');
          this.expandPlotSection('tonal_plot');
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
