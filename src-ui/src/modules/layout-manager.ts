// Layout management for responsive 4-graph grid

export class LayoutManager {
  private plotsGridElement: HTMLElement | null = null;
  private isInitialized: boolean = false;

  constructor() {
    this.initialize();
  }

  private initialize(): void {
    this.plotsGridElement = document.querySelector('.plots-grid');
    if (!this.plotsGridElement) {
      console.warn('[LAYOUT] Plots grid element not found');
      return;
    }

    // Add resize listener
    window.addEventListener('resize', this.handleResize.bind(this));

    // Initial calculation
    this.calculateLayout();

    this.isInitialized = true;
    console.log('[LAYOUT] Layout manager initialized');
  }

  private handleResize = (): void => {
    // Debounce resize events
    clearTimeout((this as any).resizeTimeout);
    (this as any).resizeTimeout = setTimeout(() => {
      this.calculateLayout();
    }, 100);
  };

  public calculateLayout(): void {
    if (!this.plotsGridElement) return;

    const rightPanel = document.getElementById('right_panel');
    if (!rightPanel) return;

    // Get available dimensions
    const rightPanelRect = rightPanel.getBoundingClientRect();
    const plotsGridRect = this.plotsGridElement.getBoundingClientRect();

    // Calculate available space for plots (excluding padding, margins, headers)
    const availableWidth = rightPanelRect.width - 40; // Account for panel padding
    const availableHeight = rightPanelRect.height - 120; // Account for scores display and other elements

    // Update CSS custom properties for dynamic sizing
    document.documentElement.style.setProperty('--plots-grid-width', `${availableWidth}px`);
    document.documentElement.style.setProperty('--plots-grid-height', `${availableHeight}px`);

    // Calculate individual plot dimensions
    const isMobile = window.innerWidth <= 768;
    const isTablet = window.innerWidth <= 1024;

    if (isMobile) {
      // Single column layout
      const plotHeight = Math.max(150, (availableHeight - 60) / 4); // 60px for gaps
      document.documentElement.style.setProperty('--plot-item-height', `${plotHeight}px`);
    } else if (isTablet) {
      // Single column layout for tablet
      const plotHeight = Math.max(200, (availableHeight - 45) / 4); // 45px for gaps
      document.documentElement.style.setProperty('--plot-item-height', `${plotHeight}px`);
    } else {
      // 2x2 grid layout for desktop
      const plotHeight = Math.max(200, (availableHeight - 15) / 2); // 15px for gap between rows
      document.documentElement.style.setProperty('--plot-item-height', `${plotHeight}px`);
    }

    console.log(`[LAYOUT] Updated layout: ${availableWidth}x${availableHeight}, mobile: ${isMobile}, tablet: ${isTablet}`);
  }

  public resizePlots(): void {
    if (!this.isInitialized) return;

    // Force Plotly plots to resize
    const plotContainers = document.querySelectorAll('.plot-grid-container.has-plot');
    plotContainers.forEach(container => {
      const plotlyDiv = container.querySelector('.js-plotly-plot') as HTMLElement;
      if (plotlyDiv && (window as any).Plotly) {
        try {
          (window as any).Plotly.Plots.resize(plotlyDiv);
        } catch (e) {
          // Ignore resize errors for plots that may not be fully initialized
        }
      }
    });
  }

  public forceRecalculate(): void {
    setTimeout(() => {
      this.calculateLayout();
      this.resizePlots();
    }, 50);
  }

  public destroy(): void {
    window.removeEventListener('resize', this.handleResize);
    clearTimeout((this as any).resizeTimeout);
    this.isInitialized = false;
  }
}
