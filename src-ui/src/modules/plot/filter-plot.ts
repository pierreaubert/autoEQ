// Filter plot functionality

import Plotly from 'plotly.js-dist-min';

export class FilterPlot {
  private filterPlotElement: HTMLElement;

  constructor(filterPlotElement: HTMLElement) {
    this.filterPlotElement = filterPlotElement;
  }

  updateFilterPlot(plotData: any): void {
    if (!this.filterPlotElement) {
      console.error('[FILTER PLOT] Filter plot element not found!');
      return;
    }

    // Show the filter plot container first
    const filterVerticalItem = document.getElementById('filter_vertical_item');
    if (filterVerticalItem) {
      filterVerticalItem.style.display = 'flex';
    }

    try {
      if (plotData && plotData.data && plotData.layout) {
        // The backend provides configuration in the layout
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
          width: 800  // Fixed height for consistent display
        };

        Plotly.newPlot(
          this.filterPlotElement,
          plotData.data,
          layout,
          config
        ).then(() => {
          console.log('[FILTER PLOT] Filter plot created successfully');
          this.filterPlotElement.classList.add('has-plot');
          this.showPlotContainer('filter_plot');
          setTimeout(() => Plotly.Plots.resize(this.filterPlotElement), 100);
        });
      } else {
        console.warn('[FILTER PLOT] Invalid filter plot data structure:', plotData);
      }
    } catch (error) {
      console.error('[FILTER PLOT] Error creating filter plot:', error);
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
