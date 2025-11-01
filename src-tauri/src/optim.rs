use crate::plot::{OptimizationPlotParams, PlotData, generate_optimization_plots};
use autoeq::{LossType, cli::Args as AutoEQArgs};
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// Global cancellation state
pub struct CancellationState {
    pub cancelled: AtomicBool,
}

impl Clone for CancellationState {
    fn clone(&self) -> Self {
        Self {
            cancelled: AtomicBool::new(self.cancelled.load(Ordering::Relaxed)),
        }
    }
}

impl Default for CancellationState {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationState {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptimizationParams {
    pub num_filters: usize,
    pub curve_path: Option<String>,
    pub target_path: Option<String>,
    pub sample_rate: f64,
    pub max_db: f64,
    pub min_db: f64,
    pub max_q: f64,
    pub min_q: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub speaker: Option<String>,
    pub version: Option<String>,
    pub measurement: Option<String>,
    pub curve_name: String,
    pub algo: String,
    pub population: usize,
    pub maxeval: usize,
    pub refine: bool,
    pub local_algo: String,
    pub min_spacing_oct: f64,
    pub spacing_weight: f64,
    pub smooth: bool,
    pub smooth_n: usize,
    pub loss: String,              // "flat", "score", or "mixed"
    pub peq_model: Option<String>, // New PEQ model system: "pk", "hp-pk", "hp-pk-lp", etc.
    // DE-specific parameters
    pub strategy: Option<String>,
    pub de_f: Option<f64>,
    pub de_cr: Option<f64>,
    pub adaptive_weight_f: Option<f64>,
    pub adaptive_weight_cr: Option<f64>,
    // Tolerance parameters
    pub tolerance: Option<f64>,
    pub atolerance: Option<f64>,
    // Captured/Target curve data (alternative to file paths)
    pub captured_frequencies: Option<Vec<f64>>,
    pub captured_magnitudes: Option<Vec<f64>>,
    pub target_frequencies: Option<Vec<f64>>,
    pub target_magnitudes: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub filter_params: Option<Vec<f64>>,
    pub objective_value: Option<f64>,
    pub preference_score_before: Option<f64>,
    pub preference_score_after: Option<f64>,
    pub filter_response: Option<PlotData>,
    pub spin_details: Option<PlotData>,
    pub filter_plots: Option<PlotData>, // Individual filter responses and sum
    pub input_curve: Option<PlotData>,  // Original normalized input curve
    pub deviation_curve: Option<PlotData>, // Target - Input (what needs to be corrected)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub iteration: usize,
    pub fitness: f64,
    pub params: Vec<f64>,
    pub convergence: f64,
}

pub fn validate_params(
    params: &OptimizationParams,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate number of filters
    if params.num_filters == 0 {
        return Err("Number of filters must be at least 1".into());
    }
    if params.num_filters > 50 {
        return Err(format!(
            "Number of filters must be between 1 and 50 (got: {})",
            params.num_filters
        )
        .into());
    }

    // Validate tolerance (lower bound 1e-12, no upper bound)
    if let Some(tol) = params.tolerance
        && tol < 1e-12
    {
        return Err(format!("Tolerance must be >= 1e-12 (got: {})", tol).into());
    }

    // Validate absolute tolerance (lower bound 0, no upper bound)
    if let Some(atol) = params.atolerance
        && atol < 1e-15
    {
        return Err(format!("Absolute tolerance must be >= 1e-15 (got: {})", atol).into());
    }

    // Validate frequency range
    if params.min_freq >= params.max_freq {
        return Err(format!(
            "Minimum frequency ({} Hz) must be less than maximum frequency ({} Hz)",
            params.min_freq, params.max_freq
        )
        .into());
    }
    if params.min_freq < 20.0 {
        return Err(format!(
            "Minimum frequency must be >= 20 Hz (got: {} Hz)",
            params.min_freq
        )
        .into());
    }
    if params.max_freq > 20000.0 {
        return Err(format!(
            "Maximum frequency must be <= 20,000 Hz (got: {} Hz)",
            params.max_freq
        )
        .into());
    }

    // Validate Q range
    if params.min_q >= params.max_q {
        return Err(format!(
            "Minimum Q ({}) must be less than maximum Q ({})",
            params.min_q, params.max_q
        )
        .into());
    }
    if params.min_q < 0.1 {
        return Err(format!("Minimum Q must be >= 0.1 (got: {})", params.min_q).into());
    }
    if params.max_q > 20.0 {
        return Err(format!("Maximum Q must be <= 100 (got: {})", params.max_q).into());
    }

    // Validate dB range
    if params.min_db > params.max_db {
        return Err(format!(
            "Minimum dB ({}) must be <= maximum dB ({})",
            params.min_db, params.max_db
        )
        .into());
    }
    if params.min_db < 0.25 {
        return Err(format!("Minimum dB must be >= 0.25 (got: {})", params.min_db).into());
    }
    if params.max_db > 20.0 {
        return Err(format!("Maximum dB must be <= 20 (got: {})", params.max_db).into());
    }

    // Validate sample rate
    if params.sample_rate < 8000.0 || params.sample_rate > 192000.0 {
        return Err(format!(
            "Sample rate must be between 8,000 and 192,000 Hz (got: {} Hz)",
            params.sample_rate
        )
        .into());
    }

    // Validate population size
    if params.population == 0 {
        return Err("Population size must be at least 1".into());
    }
    if params.population > 10000 {
        return Err(format!(
            "Population size must be between 1 and 10,000 (got: {})",
            params.population
        )
        .into());
    }

    // Validate max evaluations
    if params.maxeval == 0 {
        return Err("Maximum evaluations must be at least 1".into());
    }

    // Validate smoothing N
    if params.smooth_n < 1 || params.smooth_n > 24 {
        return Err(format!(
            "Smoothing N must be between 1 and 24 (got: {})",
            params.smooth_n
        )
        .into());
    }

    // Validate DE parameters if present
    if let Some(de_f) = params.de_f
        && !(0.0..=2.0).contains(&de_f)
    {
        return Err(format!(
            "Mutation factor (F) must be between 0 and 2 (got: {})",
            de_f
        )
        .into());
    }

    if let Some(de_cr) = params.de_cr
        && !(0.0..=1.0).contains(&de_cr)
    {
        return Err(format!(
            "Recombination probability (CR) must be between 0 and 1 (got: {})",
            de_cr
        )
        .into());
    }

    // Validate adaptive weights
    if let Some(w) = params.adaptive_weight_f
        && !(0.0..=1.0).contains(&w)
    {
        return Err(format!("Adaptive weight F must be between 0 and 1 (got: {})", w).into());
    }

    if let Some(w) = params.adaptive_weight_cr
        && !(0.0..=1.0).contains(&w)
    {
        return Err(format!("Adaptive weight CR must be between 0 and 1 (got: {})", w).into());
    }

    Ok(())
}

/// Trait for receiving progress updates during optimization
pub trait ProgressCallback: Send + Sync {
    fn on_progress(&self, update: ProgressUpdate) -> bool;
}

/// Helper function to run metaheuristics optimization with progress callbacks
fn run_mh_optimization_with_callback<P: ProgressCallback + 'static>(
    args: &AutoEQArgs,
    objective_data: &autoeq::optim::ObjectiveData,
    progress_callback: Arc<P>,
    cancellation_state: Arc<CancellationState>,
) -> Result<Vec<f64>, Box<dyn std::error::Error + Send + Sync>> {
    use autoeq::optim::AlgorithmCategory;
    use autoeq::optim::parse_algorithm_name;
    use autoeq::optim_mh::{MHIntermediate, optimize_filters_mh_with_callback};
    use autoeq::workflow::{initial_guess, setup_bounds};

    let (lower_bounds, upper_bounds) = setup_bounds(args);
    let mut x = initial_guess(args, &lower_bounds, &upper_bounds);

    // Parse algorithm name to extract MH algorithm type
    let algo_name = if let Some(AlgorithmCategory::Metaheuristics(mh_name)) =
        parse_algorithm_name(&args.algo)
    {
        mh_name
    } else {
        return Err(format!("Invalid metaheuristics algorithm: {}", args.algo).into());
    };

    let mut progress_count = 0;
    let callback = Box::new(move |intermediate: &MHIntermediate| {
        // Check for cancellation
        if cancellation_state.is_cancelled() {
            println!(
                "[RUST DEBUG] MH optimization cancelled during iteration {}",
                intermediate.iter
            );
            return autoeq::de::CallbackAction::Stop;
        }

        progress_count += 1;
        if progress_count % 5 == 0 || progress_count <= 5 {
            println!(
                "[RUST DEBUG] MH Progress update #{}: iter={}, fitness={:.6}",
                progress_count, intermediate.iter, intermediate.fun
            );
        }

        // Emit progress update via callback
        // Note: MHIntermediate doesn't have convergence, so we use 0.0 as a placeholder
        let continue_optimization = progress_callback.on_progress(ProgressUpdate {
            iteration: intermediate.iter,
            fitness: intermediate.fun,
            params: intermediate.x.to_vec(),
            convergence: 0.0, // MH doesn't provide convergence info
        });

        if !continue_optimization {
            println!("[RUST DEBUG] MH optimization stopped by progress callback");
            return autoeq::de::CallbackAction::Stop;
        }

        if progress_count % 25 == 0 {
            println!(
                "[RUST DEBUG] MH Progress callback invoked successfully (count: {})",
                progress_count
            );
        }

        autoeq::de::CallbackAction::Continue
    });

    // Run metaheuristics optimization with callback
    let result = optimize_filters_mh_with_callback(
        &mut x,
        &lower_bounds,
        &upper_bounds,
        objective_data.clone(),
        &algo_name,
        args.population,
        args.maxeval,
        callback,
    );

    match result {
        Ok((_status, _val)) => Ok(x),
        Err((e, _final_value)) => Err(Box::new(std::io::Error::other(e))),
    }
}

pub async fn run_optimization_internal<P: ProgressCallback + 'static>(
    params: OptimizationParams,
    progress_callback: Arc<P>,
    cancellation_state: Arc<CancellationState>,
) -> Result<OptimizationResult, Box<dyn std::error::Error + Send + Sync>> {
    println!("[RUST DEBUG] run_optimization_internal started");

    // Check for cancellation at start
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled before start".into());
    }

    // Validate parameters first
    println!("[RUST DEBUG] Validating parameters...");
    validate_params(&params)?;
    println!("[RUST DEBUG] Parameters validated successfully");

    // Convert parameters to AutoEQ Args structure
    let args = AutoEQArgs {
        num_filters: params.num_filters,
        curve: params.curve_path.map(PathBuf::from),
        target: params.target_path.map(PathBuf::from),
        sample_rate: params.sample_rate,
        max_db: params.max_db,
        min_db: params.min_db,
        max_q: params.max_q,
        min_q: params.min_q,
        min_freq: params.min_freq,
        max_freq: params.max_freq,
        output: None, // We'll handle plotting in the frontend
        speaker: params.speaker,
        version: params.version,
        measurement: params.measurement,
        curve_name: params.curve_name,
        algo: params.algo,
        population: params.population,
        maxeval: params.maxeval,
        refine: params.refine,
        local_algo: params.local_algo,
        min_spacing_oct: params.min_spacing_oct,
        spacing_weight: params.spacing_weight,
        smooth: params.smooth,
        smooth_n: params.smooth_n,
        loss: match params.loss.as_str() {
            "flat" => LossType::SpeakerFlat,
            "score" => LossType::SpeakerScore,
            "speaker-flat" => LossType::SpeakerFlat,
            "speaker-score" => LossType::SpeakerScore,
            "headphone-flat" => LossType::HeadphoneFlat,
            "headphone-score" => LossType::HeadphoneScore,
            _ => LossType::SpeakerFlat,
        },
        peq_model: match params.peq_model.as_deref() {
            Some("hp-pk") => autoeq::cli::PeqModel::HpPk,
            Some("hp-pk-lp") => autoeq::cli::PeqModel::HpPkLp,
            Some("free-pk-free") => autoeq::cli::PeqModel::FreePkFree,
            Some("free") => autoeq::cli::PeqModel::Free,
            Some("pk") => autoeq::cli::PeqModel::Pk,
            _ => autoeq::cli::PeqModel::Pk, // Default to Pk
        },
        peq_model_list: false,
        algo_list: false, // UI doesn't need to list algorithms
        tolerance: params.tolerance.unwrap_or(1e-3), // Use provided tolerance or default
        atolerance: params.atolerance.unwrap_or(1e-4), // Use provided atolerance or default
        recombination: params.de_cr.unwrap_or(0.9), // DE crossover probability
        strategy: params
            .strategy
            .unwrap_or_else(|| "currenttobest1bin".to_string()), // DE strategy
        strategy_list: false, // UI doesn't need to list strategies
        adaptive_weight_f: params.adaptive_weight_f.unwrap_or(0.8), // Adaptive weight for F
        adaptive_weight_cr: params.adaptive_weight_cr.unwrap_or(0.7), // Adaptive weight for CR
        no_parallel: false,
        parallel_threads: 0,
        seed: None, // Random seed for deterministic optimization (None = random)
        qa: None,   // Quality assurance mode disabled for UI (None = disabled)
    };

    // Load input data (following autoeq.rs pattern)
    println!("[RUST DEBUG] Loading input curve...");
    let (input_curve_raw, spin_data_raw) = if let (Some(captured_freqs), Some(captured_mags)) =
        (&params.captured_frequencies, &params.captured_magnitudes)
    {
        // Use captured audio data
        println!(
            "[RUST DEBUG] Using captured audio data with {} points",
            captured_freqs.len()
        );
        let captured_curve = autoeq::Curve {
            freq: Array1::from_vec(captured_freqs.clone()),
            spl: Array1::from_vec(captured_mags.clone()),
        };
        (captured_curve, None)
    } else {
        // Load from file or API
        autoeq::workflow::load_input_curve(&args).await.map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                println!("[RUST DEBUG] Failed to load input curve: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
            },
        )?
    };
    println!(
        "[RUST DEBUG] Input curve loaded successfully, {} frequency points",
        input_curve_raw.freq.len()
    );

    // Check for cancellation after data loading
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled during data loading".into());
    }

    // Resample everything to standard frequency grid
    println!("[RUST DEBUG] Creating standard frequency grid...");
    let standard_freq = autoeq::read::create_log_frequency_grid(200, 20.0, 20000.0);

    // Build/Get target curve
    let target_curve = if let (Some(target_freqs), Some(target_mags)) =
        (&params.target_frequencies, &params.target_magnitudes)
    {
        // Use provided target curve data
        println!(
            "[RUST DEBUG] Using provided target curve data with {} points",
            target_freqs.len()
        );
        let target_curve_raw = autoeq::Curve {
            freq: Array1::from_vec(target_freqs.clone()),
            spl: Array1::from_vec(target_mags.clone()),
        };
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &target_curve_raw)
    } else {
        // Build target using RAW input curve (before normalization)
        println!("[RUST DEBUG] Building target curve using raw input...");
        autoeq::workflow::build_target_curve(&args, &standard_freq, &input_curve_raw)
    };

    // Normalize input curve AFTER building target
    println!("[RUST DEBUG] Normalizing and interpolating input curve...");
    let input_curve =
        autoeq::read::normalize_and_interpolate_response(&standard_freq, &input_curve_raw);

    // Create deviation curve
    let deviation_curve = autoeq::Curve {
        freq: target_curve.freq.clone(),
        spl: target_curve.spl.clone() - &input_curve.spl,
    };

    // Process spin data if available (following autoeq.rs pattern)
    let spin_data = spin_data_raw.map(|spin_data| {
        println!(
            "[RUST DEBUG] Processing spin data with {} curves",
            spin_data.len()
        );
        spin_data
            .into_iter()
            .map(|(name, curve)| {
                let interpolated = autoeq::read::interpolate_log_space(&standard_freq, &curve);
                (name, interpolated)
            })
            .collect()
    });

    println!("[RUST DEBUG] Target curve and data processing completed");

    // Setup objective data
    println!("[RUST DEBUG] Setting up objective data...");
    let (objective_data, use_cea) = autoeq::workflow::setup_objective_data(
        &args,
        &input_curve,
        &target_curve,
        &deviation_curve,
        &spin_data,
    );
    println!(
        "[RUST DEBUG] Objective data setup complete, use_cea: {}",
        use_cea
    );

    // Get preference score before optimization if applicable
    let mut pref_score_before: Option<f64> = None;
    if use_cea
        && let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve.freq,
            spin_data.as_ref().unwrap(),
            None,
        )
        .await
    {
        pref_score_before = Some(metrics.pref_score);
    } else if args.loss == LossType::HeadphoneFlat || args.loss == LossType::HeadphoneScore {
        // Calculate headphone preference score using Olive et al. model
        println!("[RUST DEBUG] Calculating headphone preference score before optimization");
        let headphone_data = autoeq::loss::HeadphoneLossData::new(args.smooth, args.smooth_n);
        let loss_value =
            autoeq::loss::headphone_loss_with_target(&headphone_data, &input_curve, &target_curve);
        // Negate the loss value to convert to preference score (higher is better)
        pref_score_before = Some(-loss_value);
        println!(
            "[RUST DEBUG] Headphone preference score before: {:.2}",
            pref_score_before.unwrap()
        );
    }

    // Check for cancellation before optimization
    if cancellation_state.is_cancelled() {
        return Err("Optimization cancelled before starting".into());
    }

    // Run optimization with progress reporting
    println!(
        "[RUST DEBUG] Starting optimization with algorithm: {}",
        args.algo
    );

    // Determine if algorithm supports callbacks
    let supports_callbacks = args.algo == "autoeq:de" || args.algo.starts_with("mh:");

    let filter_params = if supports_callbacks {
        println!(
            "[RUST DEBUG] Using algorithm with progress reporting: {}",
            args.algo
        );
        let mut progress_count = 0;
        let cancellation_state_clone = Arc::clone(&cancellation_state);
        let progress_callback_clone = Arc::clone(&progress_callback);

        if args.algo == "autoeq:de" {
            // Use DE-specific callback
            autoeq::workflow::perform_optimization_with_callback(
                &args,
                &objective_data,
                Box::new(move |intermediate| {
                    // Check for cancellation in callback
                    if cancellation_state_clone.is_cancelled() {
                        println!("[RUST DEBUG] Optimization cancelled during iteration {}", intermediate.iter);
                        return autoeq::de::CallbackAction::Stop;
                    }
                    progress_count += 1;
                    if progress_count % 10 == 0 || progress_count <= 5 {
                        println!("[RUST DEBUG] Progress update #{}: iter={}, fitness={:.6}, convergence={:.4}",
                                 progress_count, intermediate.iter, intermediate.fun, intermediate.convergence);
                    }
                    let continue_opt = progress_callback_clone.on_progress(ProgressUpdate {
                        iteration: intermediate.iter,
                        fitness: intermediate.fun,
                        params: intermediate.x.to_vec(),
                        convergence: intermediate.convergence,
                    });
                    if !continue_opt {
                        println!("[RUST DEBUG] Optimization stopped by progress callback");
                        return autoeq::de::CallbackAction::Stop;
                    }
                    if progress_count % 50 == 0 {
                        println!("[RUST DEBUG] Progress callback invoked successfully (count: {})", progress_count);
                    }
                    autoeq::de::CallbackAction::Continue
                }),
            )
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                println!("[RUST DEBUG] DE optimization failed: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
            })?
        } else {
            // Use metaheuristics-specific optimization path
            println!("[RUST DEBUG] Using metaheuristics algorithm with progress reporting");
            run_mh_optimization_with_callback(
                &args,
                &objective_data,
                progress_callback,
                cancellation_state_clone,
            )?
        }
    } else {
        println!(
            "[RUST DEBUG] Using algorithm without progress reporting: {}",
            args.algo
        );
        autoeq::workflow::perform_optimization(&args, &objective_data).map_err(
            |e| -> Box<dyn std::error::Error + Send + Sync> {
                println!("[RUST DEBUG] Optimization failed: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
            },
        )?
    };
    println!(
        "[RUST DEBUG] Optimization completed, got {} filter parameters",
        filter_params.len()
    );

    // Calculate preference score after optimization
    let mut pref_score_after: Option<f64> = None;
    if use_cea {
        let peq_response = autoeq::x2peq::compute_peq_response_from_x(
            &input_curve.freq,
            &filter_params,
            args.sample_rate,
            args.peq_model,
        );
        if let Ok(metrics) = autoeq::cea2034::compute_cea2034_metrics(
            &input_curve.freq,
            spin_data.as_ref().unwrap(),
            Some(&peq_response),
        )
        .await
        {
            pref_score_after = Some(metrics.pref_score);
        }
    } else if args.loss == LossType::HeadphoneFlat || args.loss == LossType::HeadphoneScore {
        // Calculate headphone preference score after applying EQ
        println!("[RUST DEBUG] Calculating headphone preference score after optimization");
        let peq_response = autoeq::x2peq::compute_peq_response_from_x(
            &input_curve.freq,
            &filter_params,
            args.sample_rate,
            args.peq_model,
        );
        // Create corrected curve by adding PEQ response to input
        let corrected_curve = autoeq::Curve {
            freq: input_curve.freq.clone(),
            spl: &input_curve.spl + &peq_response,
        };
        let headphone_data = autoeq::loss::HeadphoneLossData::new(args.smooth, args.smooth_n);
        let loss_value = autoeq::loss::headphone_loss_with_target(
            &headphone_data,
            &corrected_curve,
            &target_curve,
        );
        // Negate the loss value to convert to preference score (higher is better)
        pref_score_after = Some(-loss_value);
        println!(
            "[RUST DEBUG] Headphone preference score after: {:.2}",
            pref_score_after.unwrap()
        );
    }

    // Generate plot data
    let plots = generate_optimization_plots(OptimizationPlotParams {
        filter_params: &filter_params,
        target_curve: &target_curve,
        input_curve: &input_curve,
        deviation_curve: &deviation_curve,
        spin_data: spin_data.as_ref(),
        sample_rate: args.sample_rate,
        num_filters: args.num_filters,
        peq_model: args.peq_model,
    });

    Ok(OptimizationResult {
        success: true,
        error_message: None,
        filter_params: Some(filter_params),
        objective_value: None, // We could calculate this if needed
        preference_score_before: pref_score_before,
        preference_score_after: pref_score_after,
        filter_response: Some(plots.filter_response),
        spin_details: plots.spin_details,
        filter_plots: Some(plots.filter_plots),
        input_curve: Some(plots.input_curve),
        deviation_curve: Some(plots.deviation_curve),
    })
}
