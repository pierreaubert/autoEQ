// Tonal balance plot functionality

import Plotly from 'plotly.js-dist-min';

export class TonalPlot {
  private tonalPlotElement: HTMLElement | null = null;

  constructor(tonalPlotElement?: HTMLElement) {
    this.tonalPlotElement = tonalPlotElement || null;
  }

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
          height: 650,  // Fixed height for consistent display
          width: 800,    // Fixed width for consistent display
          grid: {
            ...(plotData.layout.grid || {}),
            rows: 2,
            columns: 4,
            pattern: 'independent'
          },
          legend: {
            ...(plotData.layout.legend || {}),
            orientation: 'h',
            x: 0.5,
            xanchor: 'center',
            y: 1.2,
            yanchor: 'top'
          }
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

  private showPlotContainer(plotId: string): void {
    const verticalItemMap: { [key: string]: string } = {
      'filter_plot': 'filter_vertical_item',
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
}
