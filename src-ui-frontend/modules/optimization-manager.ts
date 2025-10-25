// Optimization management and progress tracking

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type {
  OptimizationParams,
  OptimizationResult,
  ProgressData,
  OptimizationStage,
  PeqModel,
} from "../types/optimization";
import { OPTIMIZATION_DEFAULTS } from "./optimization-constants";

export class OptimizationManager {
  // Optimization state
  private isOptimizationRunning: boolean = false;
  private currentOptimizationId: string | null = null;
  private optimizationStages: OptimizationStage[] = [];
  private progressData: ProgressData[] = [];
  private progressUnlisten: UnlistenFn | null = null;

  // Captured data storage
  private capturedFrequencies: number[] | null = null;
  private capturedMagnitudes: number[] | null = null;

  // Optimization result storage
  private lastFilterParams: number[] | null = null;
  private lastSampleRate: number | null = null;
  private lastPeqModel: PeqModel | null = null;
  private lastLossType: string | null = null;
  private lastSpeakerName: string | null = null;

  // Event callbacks
  private onProgressUpdate?: (
    stage: string,
    status: string,
    details: string,
    percentage: number,
  ) => void;
  private onOptimizationComplete?: (result: OptimizationResult) => void;
  private onOptimizationError?: (error: string) => void;
  private onProgressDataUpdate?: (
    iteration: number,
    fitness: number,
    convergence: number,
  ) => void;

  constructor() {
    this.setupProgressListener();
  }

  setCallbacks(callbacks: {
    onProgressUpdate?: (
      stage: string,
      status: string,
      details: string,
      percentage: number,
    ) => void;
    onOptimizationComplete?: (result: OptimizationResult) => void;
    onOptimizationError?: (error: string) => void;
    onProgressDataUpdate?: (
      iteration: number,
      fitness: number,
      convergence: number,
    ) => void;
  }): void {
    this.onProgressUpdate = callbacks.onProgressUpdate;
    this.onOptimizationComplete = callbacks.onOptimizationComplete;
    this.onOptimizationError = callbacks.onOptimizationError;
    this.onProgressDataUpdate = callbacks.onProgressDataUpdate;
  }

  private async setupProgressListener(): Promise<void> {
    try {
      console.log("[OPT DEBUG] 🔧 Setting up progress listeners...");

      // Listen to optimization-progress events
      this.progressUnlisten = await listen("optimization-progress", (event) => {
        const data = event.payload as any;
        console.log(
          "[OPT DEBUG] 📊 optimization-progress event received:",
          event,
        );
        console.log("[OPT DEBUG] Event payload:", data);
        this.handleProgressEvent(data);
      });

      // Also listen to progress events (alternative name)
      await listen("progress", (event) => {
        const data = event.payload as any;
        console.log("[OPT DEBUG] 📈 progress event received:", event);
        console.log("[OPT DEBUG] Event payload:", data);
        this.handleProgressEvent(data);
      });

      // Listen to optimization_progress events (underscore variant)
      await listen("optimization_progress", (event) => {
        const data = event.payload as any;
        console.log(
          "[OPT DEBUG] 📉 optimization_progress event received:",
          event,
        );
        console.log("[OPT DEBUG] Event payload:", data);
        this.handleProgressEvent(data);
      });

      // Listen to iteration events (another possible name)
      await listen("iteration", (event) => {
        const data = event.payload as any;
        console.log("[OPT DEBUG] 🔄 iteration event received:", event);
        console.log("[OPT DEBUG] Event payload:", data);
        this.handleProgressEvent(data);
      });

      // Listen to progress_update events (the actual event name from Rust!)
      await listen("progress_update", (event) => {
        const data = event.payload as any;
        console.log("[OPT DEBUG] 🎯 progress_update event received!");
        console.log("[OPT DEBUG] Event payload:", data);
        this.handleProgressEvent(data);
      });

      console.log("[OPT DEBUG] ✅ All progress listeners setup completed");
      console.log("[OPT DEBUG] 🎯 Primary listener: progress_update");
    } catch (error) {
      console.error("❌ Failed to setup progress listener:", error);
    }
  }

  private handleProgressEvent(data: any): void {
    // Handle both old format (stage/status) and new format (direct progress data)
    if (data.stage && data.status) {
      this.handleProgressUpdate(data);
    } else if (data.iteration !== undefined || data.fitness !== undefined) {
      // Direct progress data format
      console.log("[OPT DEBUG] Direct progress data detected");
      this.handleProgressUpdate(data);
    } else {
      console.log("[OPT DEBUG] Unknown event format, keys:", Object.keys(data));
    }
  }

  private handleProgressUpdate(data: any): void {
    console.log("[OPT DEBUG] handleProgressUpdate called with data:", data);
    const { stage, status, details = "", percentage } = data;

    // Calculate percentage if we have iteration data
    let calculatedPercentage = percentage || 0;

    // Update optimization stages
    this.updateOptimizationStage(stage, status, details);

    // Store progress data if it contains fitness information
    if (data.iteration !== undefined && data.fitness !== undefined) {
      const progressEntry = {
        iteration: data.iteration,
        fitness: data.fitness,
        convergence: data.convergence || 0,
      };

      console.log("[OPT DEBUG] Adding progress entry:", progressEntry);
      this.progressData.push(progressEntry);

      // Calculate percentage based on iteration (assuming max 300 iterations as seen in Rust output)
      // This is a rough estimate - could be made more accurate with max_iter from backend
      const estimatedMaxIterations = 300;
      calculatedPercentage = Math.min(
        100,
        (progressEntry.iteration / estimatedMaxIterations) * 100,
      );
      console.log(
        "[OPT DEBUG] Calculated percentage from iteration:",
        calculatedPercentage,
      );

      // Notify progress data callback for plotting
      if (this.onProgressDataUpdate) {
        console.log("[OPT DEBUG] Calling onProgressDataUpdate callback");
        this.onProgressDataUpdate(
          progressEntry.iteration,
          progressEntry.fitness,
          progressEntry.convergence,
        );
      } else {
        console.warn("[OPT DEBUG] No onProgressDataUpdate callback set!");
      }
    } else {
      console.log(
        "[OPT DEBUG] No fitness data in progress update, data keys:",
        Object.keys(data),
      );
    }

    // Notify UI with calculated percentage
    if (this.onProgressUpdate) {
      console.log(
        "[OPT DEBUG] Calling onProgressUpdate with percentage:",
        calculatedPercentage,
      );
      this.onProgressUpdate(
        stage || "Optimization",
        status || "running",
        details,
        calculatedPercentage,
      );
    }
  }

  private updateOptimizationStage(
    stage: string,
    status: string,
    details: string,
  ): void {
    const existingStageIndex = this.optimizationStages.findIndex(
      (s) => s.status === stage,
    );

    if (existingStageIndex >= 0) {
      // Update existing stage
      this.optimizationStages[existingStageIndex] = {
        ...this.optimizationStages[existingStageIndex],
        status,
        details,
        endTime: status === "completed" ? Date.now() : undefined,
      };
    } else {
      // Add new stage
      this.optimizationStages.push({
        status: stage,
        startTime: Date.now(),
        details,
      });
    }
  }

  async runOptimization(
    params: OptimizationParams,
  ): Promise<OptimizationResult> {
    if (this.isOptimizationRunning) {
      throw new Error("Optimization is already running");
    }

    this.isOptimizationRunning = true;
    this.currentOptimizationId = this.generateOptimizationId();
    this.optimizationStages = [];
    this.progressData = [];

    try {
      console.log("Starting optimization with params:", params);

      // Call the Tauri backend
      const result = (await invoke("run_optimization", {
        params,
      })) as OptimizationResult;

      console.log("Optimization completed:", result);

      if (result.success) {
        // Store the optimization results for later use (e.g., APO export)
        if (result.filter_params) {
          this.lastFilterParams = result.filter_params;
          this.lastSampleRate = params.sample_rate;
          this.lastPeqModel = params.peq_model || "pk";
          this.lastLossType = params.loss || "flat";
          this.lastSpeakerName = params.speaker || null;
          console.log("Stored filter params for APO export:", {
            numParams: this.lastFilterParams.length,
            sampleRate: this.lastSampleRate,
            peqModel: this.lastPeqModel,
            lossType: this.lastLossType,
            speakerName: this.lastSpeakerName,
          });
        }

        if (this.onOptimizationComplete) {
          this.onOptimizationComplete(result);
        }
      } else {
        const error = result.error_message || "Unknown optimization error";
        if (this.onOptimizationError) {
          this.onOptimizationError(error);
        }
      }

      return result;
    } catch (error) {
      console.error("Optimization failed:", error);
      const errorMessage =
        error instanceof Error ? error.message : "Unknown error";

      if (this.onOptimizationError) {
        this.onOptimizationError(errorMessage);
      }

      throw error;
    } finally {
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;
    }
  }

  async cancelOptimization(): Promise<void> {
    if (!this.isOptimizationRunning || !this.currentOptimizationId) {
      console.log("No optimization running to cancel");
      return;
    }

    try {
      console.log("Cancelling optimization:", this.currentOptimizationId);

      // Cancel via backend command
      await invoke("cancel_optimization");
      console.log("Optimization cancelled via backend command");

      // Always perform local cleanup regardless of backend response
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;

      // Clean up progress listener
      if (this.progressUnlisten) {
        this.progressUnlisten();
        this.progressUnlisten = null;
      }

      console.log("Optimization cancelled successfully");
    } catch (error) {
      console.error("Failed to cancel optimization:", error);
      // Even if there's an error, we should still clean up local state
      this.isOptimizationRunning = false;
      this.currentOptimizationId = null;
      if (this.progressUnlisten) {
        this.progressUnlisten();
        this.progressUnlisten = null;
      }
      throw error;
    }
  }

  async extractOptimizationParams(
    formData: FormData,
  ): Promise<OptimizationParams> {
    const inputType = formData.get("input_source") as string;

    const baseParams: OptimizationParams = {
      num_filters:
        parseInt(formData.get("num_filters") as string) ||
        OPTIMIZATION_DEFAULTS.num_filters,
      sample_rate:
        parseInt(formData.get("sample_rate") as string) ||
        OPTIMIZATION_DEFAULTS.sample_rate,
      max_db:
        parseFloat(formData.get("max_db") as string) ||
        OPTIMIZATION_DEFAULTS.max_db,
      min_db:
        parseFloat(formData.get("min_db") as string) ||
        OPTIMIZATION_DEFAULTS.min_db,
      max_q:
        parseFloat(formData.get("max_q") as string) ||
        OPTIMIZATION_DEFAULTS.max_q,
      min_q:
        parseFloat(formData.get("min_q") as string) ||
        OPTIMIZATION_DEFAULTS.min_q,
      min_freq:
        parseFloat(formData.get("min_freq") as string) ||
        OPTIMIZATION_DEFAULTS.min_freq,
      max_freq:
        parseFloat(formData.get("max_freq") as string) ||
        OPTIMIZATION_DEFAULTS.max_freq,
      curve_name:
        (formData.get("curve_name") as string) ||
        OPTIMIZATION_DEFAULTS.curve_name,
      algo: (formData.get("algo") as string) || OPTIMIZATION_DEFAULTS.algo,
      population:
        parseInt(formData.get("population") as string) ||
        OPTIMIZATION_DEFAULTS.population,
      maxeval:
        parseInt(formData.get("maxeval") as string) ||
        OPTIMIZATION_DEFAULTS.maxeval,
      refine: formData.get("refine") === "on",
      local_algo:
        (formData.get("local_algo") as string) ||
        OPTIMIZATION_DEFAULTS.local_algo,
      min_spacing_oct:
        parseFloat(formData.get("min_spacing_oct") as string) ||
        OPTIMIZATION_DEFAULTS.min_spacing_oct,
      spacing_weight:
        parseFloat(formData.get("spacing_weight") as string) ||
        OPTIMIZATION_DEFAULTS.spacing_weight,
      smooth: formData.get("smooth") === "on",
      smooth_n:
        parseInt(formData.get("smooth_n") as string) ||
        OPTIMIZATION_DEFAULTS.smooth_n,
      loss: (formData.get("loss") as string) || OPTIMIZATION_DEFAULTS.loss,
      peq_model: (formData.get("peq_model") as PeqModel) || "pk",
      iir_hp_pk:
        formData.get("iir_hp_pk") === "on" ||
        formData.get("peq_model") === "hp-pk", // For backward compatibility
      tolerance:
        parseFloat(formData.get("tolerance") as string) ||
        OPTIMIZATION_DEFAULTS.tolerance,
      atolerance:
        parseFloat(formData.get("abs_tolerance") as string) ||
        OPTIMIZATION_DEFAULTS.abs_tolerance,
    };

    // Add input-source specific parameters
    // IMPORTANT: Only set the parameters for the selected input type
    // to avoid backend confusion
    if (inputType === "speaker") {
      // Speaker data from API
      baseParams.speaker = formData.get("speaker") as string;
      baseParams.version = formData.get("version") as string;
      baseParams.measurement = formData.get("measurement") as string;
      // Explicitly clear file params
      baseParams.curve_path = undefined;
      baseParams.target_path = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "headphone") {
      // Headphone data from file with target curve
      baseParams.curve_path = formData.get("headphone_curve_path") as string;
      const headphoneTarget = formData.get("headphone_target") as string;

      // Load target curve data from CSV file
      if (headphoneTarget) {
        const targetData = await this.loadHeadphoneTarget(headphoneTarget);
        if (targetData) {
          baseParams.target_frequencies = targetData.frequencies;
          baseParams.target_magnitudes = targetData.magnitudes;
          console.log("[OPTIMIZATION] Loaded headphone target data:", {
            target: headphoneTarget,
            points: targetData.frequencies.length,
          });
        } else {
          console.error(
            "[OPTIMIZATION] Failed to load headphone target:",
            headphoneTarget,
          );
        }
      }

      // Keep curve_name for informational purposes
      baseParams.curve_name = headphoneTarget || baseParams.curve_name;

      // Explicitly clear other params
      baseParams.target_path = undefined; // Don't use file path anymore
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "file") {
      baseParams.curve_path = formData.get("curve_path") as string;
      baseParams.target_path = formData.get("target_path") as string;
      // Explicitly clear API params
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.captured_frequencies = undefined;
      baseParams.captured_magnitudes = undefined;
    } else if (inputType === "capture") {
      // Captured data will be added separately
      const capturedFreqs = this.getCapturedFrequencies();
      const capturedMags = this.getCapturedMagnitudes();
      if (capturedFreqs && capturedMags) {
        baseParams.captured_frequencies = capturedFreqs;
        baseParams.captured_magnitudes = capturedMags;
      }
      // Explicitly clear both API and file params
      baseParams.speaker = undefined;
      baseParams.version = undefined;
      baseParams.measurement = undefined;
      baseParams.curve_path = undefined;
      baseParams.target_path = undefined;
    }

    // Add DE-specific parameters if using DE algorithm
    if (baseParams.algo === "autoeq_de") {
      baseParams.strategy = (formData.get("strategy") as string) || "best1bin";
      baseParams.de_f = parseFloat(formData.get("de_f") as string) || 0.8;
      baseParams.de_cr = parseFloat(formData.get("de_cr") as string) || 0.9;
      baseParams.adaptive_weight_f =
        parseFloat(formData.get("adaptive_weight_f") as string) || 0.1;
      baseParams.adaptive_weight_cr =
        parseFloat(formData.get("adaptive_weight_cr") as string) || 0.1;
    }

    // Debug log the parameters being sent
    console.log("[OPTIMIZATION] Input type:", inputType);
    console.log("[OPTIMIZATION] Parameters to send:", {
      curve_path: baseParams.curve_path,
      curve_name: baseParams.curve_name,
      target_path: baseParams.target_path,
      speaker: baseParams.speaker,
      version: baseParams.version,
      measurement: baseParams.measurement,
      num_filters: baseParams.num_filters,
      algo: baseParams.algo,
    });

    return baseParams;
  }

  // Methods to load headphone target curve data
  private async loadHeadphoneTarget(
    targetName: string,
  ): Promise<{ frequencies: number[]; magnitudes: number[] } | null> {
    try {
      console.log(`[OPTIMIZATION] Loading headphone target: ${targetName}`);

      // Fetch the CSV file from the public directory
      const response = await fetch(`/headphone-targets/${targetName}.csv`);
      if (!response.ok) {
        console.error(
          `[OPTIMIZATION] Failed to fetch target file: ${response.statusText}`,
        );
        return null;
      }

      const csvText = await response.text();
      const lines = csvText.trim().split("\n");

      const frequencies: number[] = [];
      const magnitudes: number[] = [];

      // Parse CSV (skip header if present)
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();
        if (!line || line.startsWith("#")) continue; // Skip empty lines and comments

        // Check if this might be a header line
        if (
          i === 0 &&
          (line.toLowerCase().includes("freq") ||
            line.toLowerCase().includes("hz"))
        ) {
          continue; // Skip header
        }

        const parts = line.split(",").map((p) => p.trim());
        if (parts.length >= 2) {
          const freq = parseFloat(parts[0]);
          const mag = parseFloat(parts[1]);

          if (!isNaN(freq) && !isNaN(mag)) {
            frequencies.push(freq);
            magnitudes.push(mag);
          }
        }
      }

      console.log(
        `[OPTIMIZATION] Loaded ${frequencies.length} points from ${targetName}.csv`,
      );
      return { frequencies, magnitudes };
    } catch (error) {
      console.error(
        `[OPTIMIZATION] Error loading headphone target ${targetName}:`,
        error,
      );
      return null;
    }
  }

  // Methods to store captured data from AudioProcessor
  setCapturedData(frequencies: number[], magnitudes: number[]): void {
    this.capturedFrequencies = [...frequencies];
    this.capturedMagnitudes = [...magnitudes];
    console.log(`Stored captured data: ${frequencies.length} frequency points`);
  }

  clearCapturedData(): void {
    this.capturedFrequencies = null;
    this.capturedMagnitudes = null;
    console.log("Cleared captured data");
  }

  hasCapturedData(): boolean {
    return (
      this.capturedFrequencies !== null && this.capturedMagnitudes !== null
    );
  }

  private getCapturedFrequencies(): number[] | null {
    return this.capturedFrequencies;
  }

  private getCapturedMagnitudes(): number[] | null {
    return this.capturedMagnitudes;
  }

  private generateOptimizationId(): string {
    return `opt_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  // Getters for state
  isRunning(): boolean {
    return this.isOptimizationRunning;
  }

  getCurrentOptimizationId(): string | null {
    return this.currentOptimizationId;
  }

  getOptimizationStages(): OptimizationStage[] {
    return [...this.optimizationStages];
  }

  getProgressData(): ProgressData[] {
    return [...this.progressData];
  }

  // Getters for optimization results (for APO export)
  getFilterParams(): number[] | null {
    return this.lastFilterParams ? [...this.lastFilterParams] : null;
  }

  getSampleRate(): number | null {
    return this.lastSampleRate;
  }

  getPeqModel(): PeqModel | null {
    return this.lastPeqModel;
  }

  getLossType(): string | null {
    return this.lastLossType;
  }

  getSpeakerName(): string | null {
    return this.lastSpeakerName;
  }

  hasOptimizationResult(): boolean {
    return this.lastFilterParams !== null && this.lastSampleRate !== null;
  }

  // Cleanup
  destroy(): void {
    if (this.progressUnlisten) {
      this.progressUnlisten();
      this.progressUnlisten = null;
    }

    if (this.isOptimizationRunning) {
      this.cancelOptimization().catch(console.error);
    }
  }
}
