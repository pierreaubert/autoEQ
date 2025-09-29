// Refactored main application - streamlined and modular

import { UIManager, PlotManager, OptimizationManager, APIManager, AudioPlayer, FilterParam, LayoutManager, generateAppHTML } from "./modules";
import { OptimizationParams, OptimizationResult } from "./types";
import { AutoEQPlotAPI, PlotFiltersParams, PlotSpinParams } from "./types";

class AutoEQApplication {
  private uiManager: UIManager;
  private plotManager: PlotManager;
  private optimizationManager: OptimizationManager;
  private apiManager: APIManager;
  private audioPlayer: AudioPlayer | undefined;
  private layoutManager: LayoutManager;

  constructor() {
    console.log("Initializing AutoEQ Application...");

    // Generate and inject HTML content FIRST, before initializing managers
    this.injectHTML();

    // Initialize managers
    this.uiManager = new UIManager();
    this.apiManager = new APIManager();

    // Initialize AudioPlayer
    const audioControlsContainer = document.querySelector('.audio-testing-controls') as HTMLElement;
    if (audioControlsContainer) {
        // Clear the old controls and let the AudioPlayer component build its own UI
        audioControlsContainer.innerHTML = '';
        // Add the fixed positioning class
        audioControlsContainer.classList.add('audio-bar-fixed');

        this.audioPlayer = new AudioPlayer(
            audioControlsContainer,
          {
	    enableEQ: true,
	    enableSpectrum: true,
            showProgress: true,
            showFrequencyLabels: true,
            maxFilters: 11
	  },
          {
            onEQToggle: (enabled) => this.setEQEnabled(enabled),
            onError: (error) => this.uiManager.showError(error),
          }
        );
    } else {
        console.error("Audio controls container (.audio-testing-controls) not found!");
    }

    // Initialize plot manager with DOM elements
    const progressGraphElement = document.getElementById("progress_graph");
    const tonalPlotElement = document.getElementById("tonal_plot");
    console.log("[INIT DEBUG] Progress graph element found:", !!progressGraphElement);
    console.log("[INIT DEBUG] Tonal plot element found:", !!tonalPlotElement);

    // Debug plot element availability
    const filterPlotElement = document.getElementById("filter_plot") as HTMLElement;
    const spinPlotElement = document.getElementById("spin_plot") as HTMLElement;
    console.log("[INIT DEBUG] Filter plot element found:", !!filterPlotElement);
    console.log("[INIT DEBUG] Spin plot element found:", !!spinPlotElement);
    console.log("[INIT DEBUG] Progress graph element found:", !!progressGraphElement);
    console.log("[INIT DEBUG] Tonal plot element found:", !!tonalPlotElement);

    this.plotManager = new PlotManager(
      null, // filter_details_plot - no longer used
      filterPlotElement,
      null, // details_plot - no longer used
      spinPlotElement,
      null, // spin_plot_corrected - no longer used
      progressGraphElement as HTMLElement,
      tonalPlotElement as HTMLElement
    );

    // Initialize optimization manager
    this.optimizationManager = new OptimizationManager();

    // Initialize layout manager
    this.layoutManager = new LayoutManager();

    // Setup connections between managers
    this.setupManagerConnections();
    this.setupEventHandlers();

    // Initialize application state
    this.initialize();

    console.log("AutoEQ Application initialized successfully");
  }

  private injectHTML(): void {
    const appElement = document.getElementById('app');
    if (!appElement) {
      throw new Error('Application container element not found');
    }
    appElement.innerHTML = generateAppHTML();
    console.log("HTML content dynamically generated and injected");
  }

  private setupManagerConnections(): void {
    // Connect optimization manager callbacks to UI updates
    this.optimizationManager.setCallbacks({
      onProgressUpdate: (stage, status, details, percentage) => {
        this.uiManager.updateProgress(stage, status, details, percentage);
      },
      onOptimizationComplete: (result) => {
        this.handleOptimizationSuccess(result);
      },
      onOptimizationError: (error) => {
        this.handleOptimizationError(error);
      },
      onProgressDataUpdate: (iteration, fitness, convergence) => {
        console.log(`[MAIN DEBUG] 🎯 Progress data received: iteration=${iteration}, fitness=${fitness}, convergence=${convergence}`);

        // Update progress graph with new data
        this.plotManager.addProgressData(iteration, fitness, convergence);

        // Update elapsed time display
        this.uiManager.updateProgress('Optimization', `Iteration ${iteration}`, `Fitness: ${fitness.toFixed(4)}`, 0);

        // Update graph every 5 iterations or for early iterations
        if (iteration <= 5 || iteration % 5 === 0) {
          console.log(`[MAIN DEBUG] 📊 Updating progress graph at iteration ${iteration}`);
          this.plotManager.updateProgressGraph().catch(error => {
            console.error('[MAIN DEBUG] ❌ Error updating progress graph:', error);
          });
        }
      },
    });

    // Override UI manager event handlers to connect to application logic
    this.overrideUIEventHandlers();
  }

  private overrideUIEventHandlers(): void {
    // Override the UI manager's event handlers to connect to our logic
    const form = this.uiManager.getForm();
    const optimizeBtn = this.uiManager.getOptimizeBtn();
    const cancelBtn = this.uiManager.getCancelOptimizationBtn();

    // Add event listeners directly
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      this.runOptimization();
    });

    optimizeBtn.addEventListener("click", (e) => {
        e.preventDefault();
        this.runOptimization();
    });

    if (cancelBtn) {
      cancelBtn.addEventListener("click", () => this.cancelOptimization());
    }
  }

  private setupEventHandlers(): void {
    // Setup API-related event handlers
    const speakerSelect = document.getElementById(
      "speaker",
    ) as HTMLSelectElement;
    const versionSelect = document.getElementById(
      "version",
    ) as HTMLSelectElement;
    const curveFileBtn = document.getElementById(
      "browse_curve",
    ) as HTMLButtonElement;
    const targetFileBtn = document.getElementById(
      "browse_target",
    ) as HTMLButtonElement;

    if (speakerSelect) {
      speakerSelect.addEventListener("change", (e) => {
        const speaker = (e.target as HTMLSelectElement).value;
        console.log("Speaker changed to:", speaker);
        this.apiManager.handleSpeakerChange(speaker);
      });
    }

    if (versionSelect) {
      versionSelect.addEventListener("change", (e) => {
        const version = (e.target as HTMLSelectElement).value;
        console.log("Version changed to:", version);
        this.apiManager.handleVersionChange(version);
      });
    } else {
      console.warn("Version select element not found");
    }

    if (curveFileBtn) {
      curveFileBtn.addEventListener("click", () => {
        console.log("Curve file button clicked");
        this.apiManager.selectCurveFile();
      });
    }

    if (targetFileBtn) {
      targetFileBtn.addEventListener("click", () => {
        console.log("Target file button clicked");
        this.apiManager.selectTargetFile();
      });
    }
  }

  private async initialize(): Promise<void> {
    try {
      // Initialize UI state
      this.uiManager.resetToDefaults();
      this.uiManager.setEQEnabled(false);
      this.uiManager.collapseAllAccordion();

      // Clear any existing plots
      this.plotManager.clearAllPlots();

      // Load initial data
      await this.apiManager.loadDemoAudioList();

      // Setup autocomplete
      this.apiManager.setupAutocomplete();

      console.log("Application initialization completed");
    } catch (error) {
      console.error("Error during application initialization:", error);
      this.uiManager.showError("Failed to initialize application: " + error);
    }
  }

  private async runOptimization(): Promise<void> {
    if (this.optimizationManager.isRunning()) {
      console.log("Optimization already running");
      return;
    }

    try {
      // Clear previous results
      this.uiManager.clearResults();
      this.plotManager.clearAllPlots();

      // Get form data
      const formData = new FormData(this.uiManager.getForm());

      // Validate parameters
      const validation = this.apiManager.validateOptimizationParams(formData);
      if (!validation.isValid) {
        this.uiManager.showError(
          "Validation errors:\n" + validation.errors.join("\n"),
        );
        return;
      }

      // Extract optimization parameters
      const params =
        this.optimizationManager.extractOptimizationParams(formData);

      console.log("Starting optimization with parameters:", params);

      // Update UI state
      this.uiManager.setOptimizationRunning(true);
      this.uiManager.openOptimizationModal();

      // Clear any existing progress data
      this.plotManager.clearProgressGraph();

      // Run optimization
      await this.optimizationManager.runOptimization(params);
    } catch (error) {
      console.error("Optimization failed:", error);
      this.handleOptimizationError(
        error instanceof Error ? error.message : "Unknown error",
      );
    } finally {
      this.uiManager.setOptimizationRunning(false);
    }
  }

  private async handleOptimizationSuccess(
    result: OptimizationResult,
  ): Promise<void> {
    console.log("Optimization completed successfully:", result);

    try {
      // Update scores if available
      if (
        result.preference_score_before !== undefined &&
        result.preference_score_after !== undefined
      ) {
        this.uiManager.updateScores(
          result.preference_score_before,
          result.preference_score_after,
        );
      }

      // Update audio player with new filter parameters
      if (result.filter_params) {
        const filterParams: FilterParam[] = [];
        for (let i = 0; i < result.filter_params.length; i += 3) {
            // Convert frequency from log space to linear space
            const logFreq = result.filter_params[i];
            const linearFreq = Math.pow(10, logFreq);
            filterParams.push({
                frequency: linearFreq,
                q: result.filter_params[i+1],
                gain: result.filter_params[i+2],
                enabled: true
            });
        }
        this.audioPlayer?.updateFilterParams(filterParams);
        this.audioPlayer?.setEQEnabled(true);

        // Update filter details plot and table
        console.log("Updating filter details plot with optimization result");
        this.plotManager.updateFilterDetailsPlot(result);
      }

      // Generate plots using Tauri backend
      await this.generateOptimizationPlots(result);

      // Show close button instead of cancel button
      this.uiManager.showCloseButton();

      // Determine if this is speaker-based or curve+target optimization
      const hasSpinData = !!result.spin_details;

      // Configure plot visibility
      this.plotManager.configureVerticalVisibility(hasSpinData);

      // Force layout recalculation after plots are updated
      this.layoutManager.forceRecalculate();

      if (hasSpinData) {
        // Speaker-based optimization: show spinorama plots
        console.log("Processing speaker-based optimization plots");
        if (result.spin_details) {
          console.log(
            "Updating spinorama plots with data:",
            result.spin_details,
          );
          this.plotManager.setLastSpinDetails(result.spin_details);
          await this.plotManager.tryUpdateDetailsPlot();
          this.plotManager.updateSpinPlot(result.spin_details);
        }
      } else {
        // Curve+target optimization: show response curve with/without EQ
        console.log("Processing curve+target optimization plots");
        console.log("Full optimization result:", result);
        console.log("result.filter_response:", result.filter_response);
        console.log("result.filter_plots:", result.filter_plots);

        if (result.filter_response) {
          console.log(
            "Updating filter plot with response curve data:",
            result.filter_response,
          );
          this.plotManager.updateFilterPlot(result.filter_response);
        } else if (result.filter_plots) {
          console.log(
            "Using filter_plots data instead:",
            result.filter_plots,
          );
          this.plotManager.updateFilterPlot(result.filter_plots);
        } else {
          console.warn(
            "No filter_response or filter_plots data available for curve+target optimization",
          );
        }
      }

      console.log("All plots updated successfully");
    } catch (error) {
      console.error("Error processing optimization results:", error);
      this.uiManager.showError("Error processing results: " + error);
    }
  }

  private setEQEnabled(enabled: boolean): void {
    // This method is called by the AudioPlayer callback
    // It can be used to sync EQ state with other parts of the application if needed
    console.log(`[MAIN] EQ state changed to: ${enabled}`);
  }


  private handleOptimizationError(error: string): void {
    console.error("Optimization error:", error);
    this.uiManager.showError(error);
    this.uiManager.setOptimizationRunning(false);

    // Show close button instead of cancel button even on error
    this.uiManager.showCloseButton();
  }

  private async generateOptimizationPlots(result: OptimizationResult): Promise<void> {
    console.log("Generating optimization plots using Tauri backend...");

    try {
      // Generate filter response plots if we have filter parameters
      if (result.filter_params && result.filter_params.length > 0) {
        console.log("Generating filter response plots...");

        // We need to get the original input data to generate filter plots
        // For now, we'll use the existing filter_response data if available
        if (result.filter_response) {
          // The backend should have already generated this data
          console.log("Filter response data already available from backend");
        }
      }

      // Generate spinorama plots if we have spin data
      if (result.spin_details) {
        console.log("Generating spinorama plots...");

        try {
          // Convert curves data to CurveData format
          const cea2034_curves: { [key: string]: any } = {};
          if (result.spin_details.curves) {
            Object.keys(result.spin_details.curves).forEach(key => {
              const curveArray = result.spin_details!.curves[key];
              if (Array.isArray(curveArray) && result.spin_details!.frequencies) {
                cea2034_curves[key] = {
                  freq: result.spin_details!.frequencies,
                  spl: curveArray
                };
              }
            });
          }

          // Generate detailed spin plot
          const spinParams: PlotSpinParams = {
            cea2034_curves,
            frequencies: result.spin_details.frequencies
          };

          console.log("Calling generatePlotSpinDetails with params:", spinParams);
          const detailsPlot = await AutoEQPlotAPI.generatePlotSpinDetails(spinParams);
          console.log("Generated spin details plot:", detailsPlot);

          // Generate tonal balance plot
          console.log("Calling generatePlotSpinTonal with params:", spinParams);
          const tonalPlot = await AutoEQPlotAPI.generatePlotSpinTonal(spinParams);
          console.log("Generated tonal balance plot:", tonalPlot);

          // Store the generated plots in metadata for later use
          result.spin_details.metadata = {
            ...result.spin_details.metadata,
            detailsPlot,
            tonalPlot
          };

          // Update tonal balance plot if generated successfully
          if (tonalPlot) {
            console.log("Updating tonal balance plot in UI");
            this.plotManager.updateTonalPlot(tonalPlot);
          }

        } catch (plotError) {
          console.error("Error generating spinorama plots:", plotError);
        }
      }

      console.log("Plot generation completed");
    } catch (error) {
      console.error("Error in generateOptimizationPlots:", error);
      // Don't throw - we want optimization to continue even if plot generation fails
    }
  }

  private async cancelOptimization(): Promise<void> {
    try {
      await this.optimizationManager.cancelOptimization();
      this.uiManager.closeOptimizationModal();
      this.uiManager.setOptimizationRunning(false);
      console.log("Optimization cancelled by user");
    } catch (error) {
      console.error("Error cancelling optimization:", error);
      // Even if cancellation fails, we should still update the UI
      this.uiManager.closeOptimizationModal();
      this.uiManager.setOptimizationRunning(false);
      this.uiManager.showError(
        "Failed to cancel optimization, but stopping locally",
      );
    }
  }



  // Cleanup method
  destroy(): void {
    this.optimizationManager.destroy();
    this.audioPlayer?.destroy();
    this.layoutManager.destroy();
  }
}

// Initialize the application when DOM is loaded
document.addEventListener("DOMContentLoaded", () => {
  const app = new AutoEQApplication();

  // Store app instance globally for debugging
  (window as any).autoEQApp = app;

  // Cleanup on page unload
  window.addEventListener("beforeunload", () => {
    app.destroy();
  });
});

export { AutoEQApplication };
