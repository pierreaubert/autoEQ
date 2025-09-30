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
    const verticalItems = ['spin_vertical_item', 'details_vertical_item', 'tonal_vertical_item'];
    verticalItems.forEach(id => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = 'flex';
        console.log(`[VERTICAL DEBUG] Showed vertical item: ${id}`);
      }
    });
  }

  hideSpinVerticalItems(): void {
    const verticalItems = ['spin_vertical_item', 'details_vertical_item', 'tonal_vertical_item'];
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
      'details_plot': 'details_vertical_item',
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
      'details_plot': 'details_vertical_item',
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
    // This method is deprecated - details plot should be passed directly
    console.warn('tryUpdateDetailsPlot is deprecated, pass plot data directly to generateDetailsPlot');
  }

  async generateDetailsPlot(plotData: any): Promise<void> {
    if (!this.detailsPlotElement) {
      console.warn('Details plot element not available');
      return;
    }

    // Show the details plot container first
    const detailsVerticalItem = document.getElementById('details_vertical_item');
    if (detailsVerticalItem) {
      detailsVerticalItem.style.display = 'flex';
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // The backend provides subplot configuration in the layout
        // Ensure responsive sizing while maintaining subplot grid
        const config = {
          responsive: true,
          displayModeBar: false,
          displaylogo: false
        };

        // Adjust layout for responsive display if needed
        const layout = {
          ...plotData.layout,
          autosize: true,
          height: 550  // Fixed height for consistent display
        };

        await Plotly.newPlot(
          this.detailsPlotElement,
          plotData.data,
          layout,
          config
        );
        this.detailsPlotElement.classList.add('has-plot');
        this.showPlotContainer('details_plot');
        setTimeout(() => Plotly.Plots.resize(this.detailsPlotElement!), 100);
        console.log('Details subplot grid (2x2) generated successfully');
      } else {
        console.warn('Invalid details plot data structure:', plotData);
      }
    } catch (error) {
      console.error('Error generating details plot:', error);
    }
  }

  updateSpinPlot(plotData: any): void {
    if (!this.spinPlotElement) {
      console.error('Spin plot element not found!');
      return;
    }

    // Show the spin plot container first
    const spinVerticalItem = document.getElementById('spin_vertical_item');
    if (spinVerticalItem) {
      spinVerticalItem.style.display = 'flex';
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // Use the Plotly JSON directly from backend
        Plotly.newPlot(
          this.spinPlotElement,
          plotData.data,
          plotData.layout,
          { responsive: true, displayModeBar: false }
        ).then(() => {
          console.log('Spin plot created successfully');
          this.spinPlotElement.classList.add('has-plot');
          setTimeout(() => Plotly.Plots.resize(this.spinPlotElement), 100);
        });
      } else {
        console.warn('Invalid spin plot data structure:', plotData);
      }
    } catch (error) {
      console.error('Error creating spin plot:', error);
    }
  }

  updateFilterPlot(plotData: any): void {
    console.log('[FILTER PLOT] updateFilterPlot called');
    console.log('[FILTER PLOT] filterPlotElement exists:', !!this.filterPlotElement);

    if (!this.filterPlotElement) {
      console.error('[FILTER PLOT] Filter plot element not found!');
      return;
    }

    // Show the filter plot container first
    const filterVerticalItem = document.getElementById('filter_vertical_item');
    console.log('[FILTER PLOT] filterVerticalItem found:', !!filterVerticalItem);
    if (filterVerticalItem) {
      filterVerticalItem.style.display = 'flex';
      console.log('[FILTER PLOT] Set filterVerticalItem display to flex');
    }

    try {
      console.log('[FILTER PLOT] plotData structure:', {
        hasData: !!plotData?.data,
        hasLayout: !!plotData?.layout,
        dataLength: plotData?.data?.length
      });

      if (plotData && plotData.data && plotData.layout) {
        console.log('[FILTER PLOT] Creating Plotly plot...');
        // Use the Plotly JSON directly from backend
        Plotly.newPlot(
          this.filterPlotElement,
          plotData.data,
          plotData.layout,
          { responsive: true, displayModeBar: false }
        ).then(() => {
          console.log('[FILTER PLOT] Filter plot created successfully');
          this.filterPlotElement.classList.add('has-plot');
          setTimeout(() => {
            console.log('[FILTER PLOT] Resizing filter plot');
            Plotly.Plots.resize(this.filterPlotElement);
          }, 100);
        }).catch((error: any) => {
          console.error('[FILTER PLOT] Plotly.newPlot failed:', error);
        });
      } else {
        console.warn('[FILTER PLOT] Invalid filter plot data structure:', plotData);
      }
    } catch (error) {
      console.error('[FILTER PLOT] Error creating filter plot:', error);
    }
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


  // Tonal balance plot methods
  updateTonalPlot(plotData: any): void {
    if (!this.tonalPlotElement) {
      console.warn('Tonal plot element not available');
      return;
    }

    // Show the tonal plot container first
    const tonalVerticalItem = document.getElementById('tonal_vertical_item');
    if (tonalVerticalItem) {
      tonalVerticalItem.style.display = 'flex';
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // The backend provides subplot configuration in the layout
        const config = {
          responsive: true,
          displayModeBar: false,
          displaylogo: false
        };

        // Adjust layout for responsive display
        const layout = {
          ...plotData.layout,
          autosize: true,
          height: 550  // Fixed height for consistent display
        };

        Plotly.newPlot(
          this.tonalPlotElement,
          plotData.data,
          layout,
          config
        ).then(() => {
          console.log('Tonal subplot grid (2x2) created successfully');
          this.tonalPlotElement!.classList.add('has-plot');
          this.showPlotContainer('tonal_plot');
          setTimeout(() => Plotly.Plots.resize(this.tonalPlotElement!), 100);
        });
      } else {
        console.warn('Invalid tonal plot data structure:', plotData);
      }
    } catch (error) {
      console.error('Error creating tonal plot:', error);
    }
  }
}
