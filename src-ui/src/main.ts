// Refactored main application - streamlined and modular

import { UIManager } from "./modules";
import { PlotManager } from "./modules";
import { OptimizationManager } from "./modules";
import { APIManager } from "./modules";
import { AudioProcessor } from "./modules";
import { OptimizationParams, OptimizationResult } from "./types";
import { AutoEQPlotAPI, PlotFiltersParams, PlotSpinParams } from "./types";

class AutoEQApplication {
  private uiManager: UIManager;
  private plotManager: PlotManager;
  private optimizationManager: OptimizationManager;
  private apiManager: APIManager;
  private audioProcessor: AudioProcessor;

  constructor() {
    console.log("Initializing AutoEQ Application...");

    // Initialize managers
    this.uiManager = new UIManager();
    this.apiManager = new APIManager();
    this.audioProcessor = new AudioProcessor();

    // Initialize plot manager with DOM elements
    const progressGraphElement = document.getElementById("progress_graph");
    const tonalPlotElement = document.getElementById("tonal_plot");
    console.log("[INIT DEBUG] Progress graph element found:", !!progressGraphElement);
    console.log("[INIT DEBUG] Tonal plot element found:", !!tonalPlotElement);

    this.plotManager = new PlotManager(
      document.getElementById("filter_details_plot") as HTMLElement,
      document.getElementById("filter_plot") as HTMLElement,
      document.getElementById("details_plot") as HTMLElement,
      document.getElementById("spin_plot") as HTMLElement,
      document.getElementById("spin_plot_corrected") as HTMLElement,
      progressGraphElement as HTMLElement,
      tonalPlotElement as HTMLElement
    );

    // Initialize optimization manager
    this.optimizationManager = new OptimizationManager();

    // Setup connections between managers
    this.setupManagerConnections();
    this.setupEventHandlers();

    // Initialize application state
    this.initialize();

    console.log("AutoEQ Application initialized successfully");
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
    const resetBtn = this.uiManager.getResetBtn();
    const captureBtn = this.uiManager.getCaptureBtn();
    const listenBtn = this.uiManager.getListenBtn();
    const stopBtn = this.uiManager.getStopBtn();
    const eqOnBtn = this.uiManager.getEqOnBtn();
    const eqOffBtn = this.uiManager.getEqOffBtn();
    const cancelBtn = this.uiManager.getCancelOptimizationBtn();

    // Remove existing event listeners and add our own
    const newOptimizeBtn = optimizeBtn.cloneNode(true) as HTMLButtonElement;
    optimizeBtn.parentNode?.replaceChild(newOptimizeBtn, optimizeBtn);
    newOptimizeBtn.addEventListener("click", (e) => {
      e.preventDefault();
      this.runOptimization();
    });

    form.addEventListener("submit", (e) => {
      e.preventDefault();
      this.runOptimization();
    });

    if (captureBtn) {
      const newCaptureBtn = captureBtn.cloneNode(true) as HTMLButtonElement;
      captureBtn.parentNode?.replaceChild(newCaptureBtn, captureBtn);
      newCaptureBtn.addEventListener("click", () => this.startCapture());
    }

    if (listenBtn) {
      const newListenBtn = listenBtn.cloneNode(true) as HTMLButtonElement;
      listenBtn.parentNode?.replaceChild(newListenBtn, listenBtn);
      newListenBtn.addEventListener("click", () => this.startAudioPlayback());
    }

    if (stopBtn) {
      const newStopBtn = stopBtn.cloneNode(true) as HTMLButtonElement;
      stopBtn.parentNode?.replaceChild(newStopBtn, stopBtn);
      newStopBtn.addEventListener("click", () => this.stopAudioPlayback());
    }

    if (eqOnBtn) {
      const newEqOnBtn = eqOnBtn.cloneNode(true) as HTMLButtonElement;
      eqOnBtn.parentNode?.replaceChild(newEqOnBtn, eqOnBtn);
      newEqOnBtn.addEventListener("click", () => this.setEQEnabled(true));
    }

    if (eqOffBtn) {
      const newEqOffBtn = eqOffBtn.cloneNode(true) as HTMLButtonElement;
      eqOffBtn.parentNode?.replaceChild(newEqOffBtn, eqOffBtn);
      newEqOffBtn.addEventListener("click", () => this.setEQEnabled(false));
    }

    if (cancelBtn) {
      const newCancelBtn = cancelBtn.cloneNode(true) as HTMLButtonElement;
      cancelBtn.parentNode?.replaceChild(newCancelBtn, cancelBtn);
      newCancelBtn.addEventListener("click", () => this.cancelOptimization());
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
    const demoAudioSelect = document.getElementById(
      "demo_audio_select",
    ) as HTMLSelectElement;

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
    } else {
      console.warn("Curve file button not found");
    }

    if (targetFileBtn) {
      targetFileBtn.addEventListener("click", () => {
        console.log("Target file button clicked");
        this.apiManager.selectTargetFile();
      });
    } else {
      console.warn("Target file button not found");
    }

    if (demoAudioSelect) {
      console.log("Demo audio select element found, adding event listener");
      demoAudioSelect.addEventListener("change", async (e) => {
        const audioName = (e.target as HTMLSelectElement).value;
        console.log("Demo audio selected:", audioName);

        if (audioName) {
          try {
            this.uiManager.setAudioStatus("Loading audio...");
            this.uiManager.setListenButtonEnabled(false);

            const url = await this.apiManager.getDemoAudioUrl(audioName);
            console.log("Demo audio URL:", url);

            if (url) {
              await this.audioProcessor.loadAudioFromUrl(url);
              console.log("Demo audio loaded successfully");

              // Enable the Listen button and update UI
              this.uiManager.setListenButtonEnabled(true);
              this.uiManager.setAudioStatus("Audio ready");
            }
          } catch (error) {
            console.error("Error loading demo audio:", error);
            this.uiManager.setAudioStatus("Failed to load audio");
            this.uiManager.setListenButtonEnabled(false);
            this.uiManager.showError("Failed to load demo audio: " + error);
          }
        } else {
          // No audio selected, disable Listen button
          this.uiManager.setListenButtonEnabled(false);
          this.uiManager.setAudioStatus("No audio selected");
        }
      });
    } else {
      console.warn("Demo audio select element not found!");
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

      // Setup audio spectrum analyzer if canvas exists
      const spectrumCanvas = document.getElementById(
        "spectrum_canvas",
      ) as HTMLCanvasElement;
      if (spectrumCanvas) {
        this.audioProcessor.setupSpectrumAnalyzer(spectrumCanvas);
      }

      // Setup audio status elements
      this.audioProcessor.setupAudioStatusElements({
        status: document.getElementById("audio_status") || undefined,
        statusText: document.getElementById("audio_status_text") || undefined,
        duration: document.getElementById("audio_duration") || undefined,
        position: document.getElementById("audio_position") || undefined,
        progressFill:
          document.getElementById("audio_progress_fill") || undefined,
      });

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

      // Update audio processor with new filter parameters
      if (result.filter_params) {
        this.audioProcessor.updateFilterParams(result.filter_params);
        this.uiManager.setEQEnabled(true);

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
      this.plotManager.configureAccordionVisibility(hasSpinData);

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
        if (result.filter_response) {
          console.log(
            "Updating filter plot with response curve data:",
            result.filter_response,
          );
          this.plotManager.updateFilterPlot(result.filter_response);
        } else {
          console.warn(
            "No filter_response data available for curve+target optimization",
          );
        }
      }

      console.log("All plots updated successfully");
    } catch (error) {
      console.error("Error processing optimization results:", error);
      this.uiManager.showError("Error processing results: " + error);
    }
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

  private async startCapture(): Promise<void> {
    console.log("Starting audio capture...");

    try {
      // Update UI to show capture is starting
      const captureBtn = this.uiManager.getCaptureBtn();
      const captureStatus = this.uiManager.getCaptureStatus();
      const captureStatusText = this.uiManager.getCaptureStatusText();
      const captureResult = this.uiManager.getCaptureResult();

      if (captureBtn) {
        captureBtn.textContent = "⏹️ Stop Capture";
        captureBtn.classList.add("capturing");
      }

      if (captureStatus) {
        captureStatus.style.display = "block";
      }

      if (captureStatusText) {
        captureStatusText.textContent = "Starting capture...";
      }

      // Hide previous results
      if (captureResult) {
        captureResult.style.display = "none";
      }

      // Start the actual capture
      if (captureStatusText) {
        captureStatusText.textContent = "Capturing audio (please wait)...";
      }

      const result = await this.audioProcessor.startCapture();

      if (result.success && result.frequencies.length > 0) {
        console.log("Capture successful:", result.frequencies.length, "points");

        // Store the captured data in optimization manager
        this.optimizationManager.setCapturedData(
          result.frequencies,
          result.magnitudes,
        );

        if (captureStatusText) {
          captureStatusText.textContent = `✅ Captured ${result.frequencies.length} frequency points`;
        }

        // Show results
        if (captureResult) {
          captureResult.style.display = "block";
        }

        // Plot the captured data
        this.plotCapturedData(result.frequencies, result.magnitudes);
      } else {
        throw new Error(result.error || "Capture failed");
      }
    } catch (error) {
      console.error("Capture error:", error);

      const captureStatusText = this.uiManager.getCaptureStatusText();
      if (captureStatusText) {
        captureStatusText.textContent = `❌ Capture failed: ${error instanceof Error ? error.message : "Unknown error"}`;
      }

      this.uiManager.showError(
        "Audio capture failed: " +
          (error instanceof Error ? error.message : "Unknown error"),
      );
    } finally {
      // Reset UI
      const captureBtn = this.uiManager.getCaptureBtn();
      if (captureBtn) {
        captureBtn.textContent = "🎤 Start Capture";
        captureBtn.classList.remove("capturing");
      }
    }
  }

  private plotCapturedData(frequencies: number[], magnitudes: number[]): void {
    console.log("Plotting captured frequency response...");

    try {
      // Create plot data structure
      const plotData = {
        frequencies: frequencies,
        curves: {
          "Captured Response": magnitudes,
        },
        metadata: {
          title: "Captured Frequency Response",
          type: "capture",
        },
      };

      // Use the plot manager to display the captured data
      this.plotManager.updateFilterPlot(plotData);

      console.log("Captured data plotted successfully");
    } catch (error) {
      console.error("Error plotting captured data:", error);
    }
  }

  private async startAudioPlayback(): Promise<void> {
    try {
      await this.audioProcessor.play();
      this.audioProcessor.startSpectrumAnalysis();
    } catch (error) {
      console.error("Error starting audio playback:", error);
      this.uiManager.showError("Failed to start audio playback: " + error);
    }
  }

  private stopAudioPlayback(): void {
    this.audioProcessor.stop();
    this.audioProcessor.stopSpectrumAnalysis();
  }

  private setEQEnabled(enabled: boolean): void {
    this.uiManager.setEQEnabled(enabled);
    this.audioProcessor.setEQEnabled(enabled);
  }

  // Cleanup method
  destroy(): void {
    this.optimizationManager.destroy();
    this.audioProcessor.destroy();
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
