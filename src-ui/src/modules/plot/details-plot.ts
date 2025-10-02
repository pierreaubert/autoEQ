// Details plot functionality

import Plotly from 'plotly.js-dist-min';

export class DetailsPlot {
  private detailsPlotElement: HTMLElement | null;

  constructor(detailsPlotElement: HTMLElement | null) {
    this.detailsPlotElement = detailsPlotElement;
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
          height: 550,  // Fixed height for consistent display
          width: 800,  // Fixed width for consistent display
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

  private showPlotContainer(plotId: string): void {
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
}
