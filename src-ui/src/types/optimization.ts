// Types for optimization parameters and results

export interface OptimizationParams {
  num_filters: number;
  curve_path?: string;
  target_path?: string;
  sample_rate: number;
  max_db: number;
  min_db: number;
  max_q: number;
  min_q: number;
  min_freq: number;
  max_freq: number;
  speaker?: string;
  version?: string;
  measurement?: string;
  curve_name: string;
  algo: string;
  population: number;
  maxeval: number;
  refine: boolean;
  local_algo: string;
  min_spacing_oct: number;
  spacing_weight: number;
  smooth: boolean;
  smooth_n: number;
  loss: string;
  iir_hp_pk: boolean;
  // DE-specific parameters
  strategy?: string;
  de_f?: number;
  de_cr?: number;
  adaptive_weight_f?: number;
  adaptive_weight_cr?: number;
  // Tolerance parameters
  tolerance: number;
  atolerance: number;
  // Captured curve data
  captured_frequencies?: number[];
  captured_magnitudes?: number[];
  // Target curve data (for headphones)
  target_frequencies?: number[];
  target_magnitudes?: number[];
}

export interface PlotData {
  frequencies: number[];
  curves: { [name: string]: number[] };
  metadata: { [key: string]: any };
}

export interface OptimizationResult {
  success: boolean;
  error_message?: string;
  filter_params?: number[];
  objective_value?: number;
  preference_score_before?: number;
  preference_score_after?: number;
  filter_response?: PlotData;
  spin_details?: PlotData;
  filter_plots?: PlotData;
  input_curve?: PlotData;      // Original normalized input curve
  deviation_curve?: PlotData;  // Target - Input (what needs to be corrected)
}

export interface ProgressData {
  iteration: number;
  fitness: number;
  convergence: number;
}

export interface OptimizationStage {
  status: string;
  startTime?: number;
  endTime?: number;
  details?: string;
}
