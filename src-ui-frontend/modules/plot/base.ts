// Base plot functionality and visibility management

import Plotly from "plotly.js-dist-min";
import { PlotData } from "../../types";

export class PlotBase {
  // Plot data caching
  protected filterPlotsData: PlotData | null = null;
  protected lastSpinDetails: PlotData | null = null;

  showSpinVerticalItems(): void {
    const verticalItems = [
      "spin_vertical_item",
      "details_vertical_item",
      "tonal_vertical_item",
    ];
    verticalItems.forEach((id) => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = "flex";
        console.log(`[VERTICAL DEBUG] Showed vertical item: ${id}`);
      }
    });
  }

  hideSpinVerticalItems(): void {
    const verticalItems = [
      "spin_vertical_item",
      "details_vertical_item",
      "tonal_vertical_item",
    ];
    verticalItems.forEach((id) => {
      const element = document.getElementById(id);
      if (element) {
        element.style.display = "none";
        console.log(`[VERTICAL DEBUG] Hid vertical item: ${id}`);
      }
    });
  }

  showPlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
      spin_plot: "spin_vertical_item",
      details_plot: "details_vertical_item",
      tonal_plot: "tonal_vertical_item",
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = "flex";
        console.log(
          `[VERTICAL DEBUG] Showed plot container: ${plotId} via vertical item ${verticalItemId}`,
        );
      }
    }
  }

  hidePlotContainer(plotId: string): void {
    // For compatibility with existing code, but now we manage at vertical item level
    const verticalItemMap: { [key: string]: string } = {
      spin_plot: "spin_vertical_item",
      details_plot: "details_vertical_item",
      tonal_plot: "tonal_vertical_item",
    };

    const verticalItemId = verticalItemMap[plotId];
    if (verticalItemId) {
      const element = document.getElementById(verticalItemId);
      if (element) {
        element.style.display = "none";
        console.log(
          `[VERTICAL DEBUG] Hid plot container: ${plotId} via vertical item ${verticalItemId}`,
        );
      }
    }
  }

  configureVerticalVisibility(hasSpinData: boolean): void {
    console.log(
      "[VERTICAL DEBUG] Configuring vertical visibility, hasSpinData:",
      hasSpinData,
    );

    if (hasSpinData) {
      // Speaker-based: show all 3 graphs (Filter Response + 2 spinorama graphs)
      console.log(
        "[VERTICAL DEBUG] Showing all graphs for speaker-based optimization",
      );
      this.showSpinVerticalItems();
    } else {
      // Curve+target: only show Filter Response graph
      console.log(
        "[VERTICAL DEBUG] Showing only Filter Response for curve+target optimization",
      );
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

  protected clearPlotElement(element: HTMLElement | null): void {
    if (element) {
      try {
        Plotly.purge(element);
        } catch (_e) {
        // Element may not have been plotted yet
      }
      element.innerHTML =
        '<div class="plot-placeholder">No data to display</div>';
      element.classList.remove("has-plot");
    }
  }
}
