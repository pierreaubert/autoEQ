// Refactored main application - streamlined and modular

import {
  UIManager,
  PlotManager,
  OptimizationManager,
  APIManager,
  AudioPlayer,
  FilterParam,
  LayoutManager,
  generateAppHTML,
} from "./modules";
import { OptimizationResult } from "./types";
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
    const audioControlsContainer = document.querySelector(
      ".audio-testing-controls",
    ) as HTMLElement;
    if (audioControlsContainer) {
      // Clear the old controls and let the AudioPlayer component build its own UI
      audioControlsContainer.innerHTML = "";
      // Add the fixed positioning class
      audioControlsContainer.classList.add("audio-bar-fixed");

      this.audioPlayer = new AudioPlayer(
        audioControlsContainer,
        {
          enableEQ: true,
          enableSpectrum: true,
          showProgress: true,
          showFrequencyLabels: true,
          maxFilters: 11,
        },
        {
          onEQToggle: (enabled) => this.setEQEnabled(enabled),
          onError: (error) => this.uiManager.showError(error),
        },
      );
    } else {
      console.error(
        "Audio controls container (.audio-testing-controls) not found!",
      );
    }

    // Initialize plot manager with DOM elements
    const progressGraphElement = document.getElementById("progress_graph");
    const tonalPlotElement = document.getElementById("tonal_plot");
    console.log(
      "[INIT DEBUG] Progress graph element found:",
      !!progressGraphElement,
    );
    console.log("[INIT DEBUG] Tonal plot element found:", !!tonalPlotElement);

    // Debug plot element availability
    const filterPlotElement = document.getElementById(
      "filter_plot",
    ) as HTMLElement;
    const spinPlotElement = document.getElementById("spin_plot") as HTMLElement;
    const detailsPlotElement = document.getElementById(
      "details_plot",
    ) as HTMLElement;
    console.log("[INIT DEBUG] Filter plot element found:", !!filterPlotElement);
    console.log("[INIT DEBUG] Spin plot element found:", !!spinPlotElement);
    console.log(
      "[INIT DEBUG] Details plot element found:",
      !!detailsPlotElement,
    );
    console.log(
      "[INIT DEBUG] Progress graph element found:",
      !!progressGraphElement,
    );
    console.log("[INIT DEBUG] Tonal plot element found:", !!tonalPlotElement);

    this.plotManager = new PlotManager(
      filterPlotElement,
      detailsPlotElement, // CEA2034 details plot
      spinPlotElement,
      progressGraphElement as HTMLElement,
      tonalPlotElement as HTMLElement,
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
    const appElement = document.getElementById("app");
    if (!appElement) {
      throw new Error("Application container element not found");
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
        console.log(
          `[MAIN DEBUG] 🎯 Progress data received: iteration=${iteration}, fitness=${fitness}, convergence=${convergence}`,
        );

        // Update progress graph with new data
        this.plotManager.addProgressData(iteration, fitness, convergence);

        // Update elapsed time display
        this.uiManager.updateProgress(
          "Optimization",
          `Iteration ${iteration}`,
          `Fitness: ${fitness.toFixed(4)}`,
          0,
        );

        // Update graph every 5 iterations or for early iterations
        if (iteration <= 5 || iteration % 5 === 0) {
          console.log(
            `[MAIN DEBUG] 📊 Updating progress graph at iteration ${iteration}`,
          );
          this.plotManager.updateProgressGraph().catch((error) => {
            console.error(
              "[MAIN DEBUG] ❌ Error updating progress graph:",
              error,
            );
          });
        }
      },
    });

    // Connect UI manager capture callback to optimization manager
    this.uiManager.setCaptureCompleteCallback((frequencies, magnitudes) => {
      if (frequencies.length > 0 && magnitudes.length > 0) {
        this.optimizationManager.setCapturedData(frequencies, magnitudes);
        console.log(
          "[MAIN] Captured data stored in optimization manager:",
          frequencies.length,
          "points",
        );
      } else {
        this.optimizationManager.clearCapturedData();
        console.log("[MAIN] Cleared captured data from optimization manager");
      }
    });

    // Connect UI manager output device change callback to audio player
    this.uiManager.setOutputDeviceChangeCallback((deviceId) => {
      if (this.audioPlayer) {
        this.audioPlayer.setOutputDevice(deviceId);
        console.log(`[MAIN] Audio player output device set to: ${deviceId}`);
      }
    });

    // Connect optimization result getter for APO download
    this.uiManager.setOptimizationResultGetter(() => ({
      filterParams: this.optimizationManager.getFilterParams(),
      sampleRate: this.optimizationManager.getSampleRate(),
      peqModel: this.optimizationManager.getPeqModel(),
      lossType: this.optimizationManager.getLossType(),
      speakerName: this.optimizationManager.getSpeakerName(),
    }));

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
    const headphoneCurveBtn = document.getElementById(
      "browse_headphone_curve",
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

    if (headphoneCurveBtn) {
      headphoneCurveBtn.addEventListener("click", () => {
        console.log("Headphone curve file button clicked");
        this.apiManager.selectHeadphoneCurveFile();
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
        await this.optimizationManager.extractOptimizationParams(formData);

      console.log("Starting optimization with parameters:", params);

      // Update UI state
      this.uiManager.setOptimizationRunning(true);
      this.uiManager.disableDownloadButton();
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
    console.log("Result structure:", {
      has_filter_params: !!result.filter_params,
      filter_params_length: result.filter_params?.length,
      has_filter_response: !!result.filter_response,
      has_filter_plots: !!result.filter_plots,
      has_input_curve: !!result.input_curve,
      has_deviation_curve: !!result.deviation_curve,
      has_spin_details: !!result.spin_details,
      preference_score_before: result.preference_score_before,
      preference_score_after: result.preference_score_after,
    });

    // Debug: Log curve data availability
    if (result.filter_response) {
      console.log(
        "filter_response frequencies length:",
        result.filter_response.frequencies?.length,
      );
      console.log(
        "filter_response curves:",
        Object.keys(result.filter_response.curves || {}),
      );
    }
    if (result.input_curve) {
      console.log(
        "input_curve frequencies length:",
        result.input_curve.frequencies?.length,
      );
      console.log(
        "input_curve curves:",
        Object.keys(result.input_curve.curves || {}),
      );
    }
    if (result.deviation_curve) {
      console.log(
        "deviation_curve frequencies length:",
        result.deviation_curve.frequencies?.length,
      );
      console.log(
        "deviation_curve curves:",
        Object.keys(result.deviation_curve.curves || {}),
      );
    }

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
            q: result.filter_params[i + 1],
            gain: result.filter_params[i + 2],
            enabled: true,
          });
        }
        this.audioPlayer?.updateFilterParams(filterParams);
        this.audioPlayer?.setEQEnabled(true);
      }

      // Generate plots using Tauri backend
      await this.generateOptimizationPlots(result);

      // Show close button instead of cancel button
      this.uiManager.showCloseButton();

      // Enable download button after successful optimization
      this.uiManager.enableDownloadButton();

      // Determine if this is speaker-based or curve+target optimization
      const hasSpinData = !!result.spin_details;

      // Configure plot visibility
      this.plotManager.configureVerticalVisibility(hasSpinData);

      // Force layout recalculation after plots are updated
      this.layoutManager.forceRecalculate();

      if (hasSpinData) {
        // Speaker-based optimization: show spinorama plots
        console.log("Processing speaker-based optimization plots");
        // Spin plots are generated in generateOptimizationPlots and passed as Plotly JSON
      } else {
        // Curve+target optimization: show response curve with/without EQ
        console.log("Processing curve+target optimization plots");
        // Filter plots will be generated as Plotly JSON in generateOptimizationPlots
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

    // Keep download button disabled on error
    this.uiManager.disableDownloadButton();

    // Show close button instead of cancel button even on error
    this.uiManager.showCloseButton();
  }

  private async generateOptimizationPlots(
    result: OptimizationResult,
  ): Promise<void> {
    console.log("Generating optimization plots using Tauri backend...");
    console.log("Result has filter_response:", !!result.filter_response);
    console.log("Result has filter_plots:", !!result.filter_plots);
    console.log("Result has input_curve:", !!result.input_curve);
    console.log("Result has deviation_curve:", !!result.deviation_curve);
    console.log("Result has spin_details:", !!result.spin_details);

    try {
      // ALWAYS generate the filter plot - backend always provides this data
      if (result.filter_params && result.filter_params.length > 0) {
        console.log("Generating filter response plot...");
        console.log("Filter params count:", result.filter_params.length / 3);

        // Verify we have all required curves (backend always provides these)
        if (
          !result.filter_response ||
          !result.input_curve ||
          !result.deviation_curve
        ) {
          console.error("MISSING REQUIRED CURVES FROM BACKEND!");
          console.error("  filter_response:", !!result.filter_response);
          console.error("  input_curve:", !!result.input_curve);
          console.error("  deviation_curve:", !!result.deviation_curve);
          throw new Error("Backend did not return required curve data");
        }

        // Extract data from the optimization result
        const frequencies = result.filter_response.frequencies;
        const targetResponse = result.filter_response.curves["Target"];
        const inputCurve = result.input_curve.curves["Input"];
        const deviationCurve = result.deviation_curve.curves["Deviation"];

        if (!targetResponse || !inputCurve || !deviationCurve) {
          console.error("MISSING CURVE DATA IN RESPONSE!");
          console.error("  targetResponse:", !!targetResponse);
          console.error("  inputCurve:", !!inputCurve);
          console.error("  deviationCurve:", !!deviationCurve);
          throw new Error("Required curves missing from response data");
        }

        // Prepare parameters for backend plot generation
        const plotParams: PlotFiltersParams = {
          input_curve: {
            freq: frequencies,
            spl: inputCurve,
          },
          target_curve: {
            freq: frequencies,
            spl: targetResponse,
          },
          deviation_curve: {
            freq: frequencies,
            spl: deviationCurve,
          },
          optimized_params: result.filter_params,
          sample_rate: 48000, // Use default or get from form
          num_filters: result.filter_params.length / 3,
          peq_model:
            ((document.getElementById("peq_model") as HTMLSelectElement)
              ?.value as
              | "pk"
              | "hp-pk"
              | "hp-pk-lp"
              | "free-pk-free"
              | "free") || "pk",
          iir_hp_pk: false, // Deprecated
        };

        console.log("Calling backend to generate 4-subplot filter plot...");
        // Call backend to generate the 4-subplot plot
        const filterPlot = await AutoEQPlotAPI.generatePlotFilters(plotParams);
        console.log("✅ Generated 4-subplot filter plot successfully");

        // Update the filter plot with the Plotly JSON from backend
        this.plotManager.updateFilterPlot(filterPlot);
      } else {
        console.error("No filter params in optimization result");
      }

      // Generate spinorama plots if we have spin data
      if (result.spin_details) {
        console.log("Generating spinorama plots...");

        try {
          // Convert curves data to CurveData format
          const cea2034_curves: Record<
            string,
            { freq: number[]; spl: number[] }
          > = {};
          if (result.spin_details.curves) {
            Object.keys(result.spin_details.curves).forEach((key) => {
              const curveArray = result.spin_details!.curves[key];
              if (
                Array.isArray(curveArray) &&
                result.spin_details!.frequencies
              ) {
                cea2034_curves[key] = {
                  freq: result.spin_details!.frequencies,
                  spl: curveArray,
                };
              }
            });
          }

          // Extract EQ response for the spin plots
          const eq_response =
            result.filter_response?.curves["EQ Response"] || [];

          // Generate plots as Plotly JSON from backend
          const spinParams: PlotSpinParams = {
            cea2034_curves,
            eq_response, // Add eq_response to show "with EQ" traces
            frequencies: result.spin_details.frequencies,
          };

          console.log("Generating spin plot (Plotly JSON) with eq_response...");
          const spinPlot = await AutoEQPlotAPI.generatePlotSpin(spinParams);
          console.log("Generated spin plot:", spinPlot);

          // Update spin plot with Plotly JSON
          this.plotManager.updateSpinPlot(spinPlot);

          console.log("Generating spin details plot (Plotly JSON)...");
          const detailsPlot =
            await AutoEQPlotAPI.generatePlotSpinDetails(spinParams);
          console.log("Generated spin details plot:", detailsPlot);

          // Update details plot with Plotly JSON
          await this.plotManager.generateDetailsPlot(detailsPlot);

          console.log("Generating tonal balance plot (Plotly JSON)...");
          const tonalPlot =
            await AutoEQPlotAPI.generatePlotSpinTonal(spinParams);
          console.log("Generated tonal balance plot:", tonalPlot);

          // Update tonal plot with Plotly JSON
          this.plotManager.updateTonalPlot(tonalPlot);
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
  (window as unknown as { autoEQApp: AutoEQApplication }).autoEQApp = app;

  // Cleanup on page unload
  window.addEventListener("beforeunload", () => {
    app.destroy();
  });
});

export { AutoEQApplication };
