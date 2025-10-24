use autoeq::Curve;
use ndarray::Array1;
use plotly::Plot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct PlotData {
    pub frequencies: Vec<f64>,
    pub curves: HashMap<String, Vec<f64>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlotFiltersParams {
    pub input_curve: CurveData,
    pub target_curve: CurveData,
    pub deviation_curve: CurveData,
    pub optimized_params: Vec<f64>,
    pub sample_rate: f64,
    pub num_filters: usize,
    pub peq_model: Option<String>, // "pk", "hp-pk", etc.
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlotSpinParams {
    pub cea2034_curves: Option<HashMap<String, CurveData>>,
    pub eq_response: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CurveData {
    pub freq: Vec<f64>,
    pub spl: Vec<f64>,
}

pub struct OptimizationPlots {
    pub filter_response: PlotData,
    pub filter_plots: PlotData,
    pub spin_details: Option<PlotData>,
    pub input_curve: PlotData,
    pub deviation_curve: PlotData,
}

pub struct OptimizationPlotParams<'a> {
    pub filter_params: &'a [f64],
    pub target_curve: &'a Curve,
    pub input_curve: &'a Curve,
    pub deviation_curve: &'a Curve,
    pub spin_data: Option<&'a HashMap<String, Curve>>,
    pub sample_rate: f64,
    pub num_filters: usize,
    pub peq_model: autoeq::cli::PeqModel,
}

// Helper function to convert CurveData to autoeq::Curve
pub fn curve_data_to_curve(curve_data: &CurveData) -> Curve {
    Curve {
        freq: Array1::from_vec(curve_data.freq.clone()),
        spl: Array1::from_vec(curve_data.spl.clone()),
    }
}

// Helper function to convert plotly::Plot to JSON
pub fn plot_to_json(plot: Plot) -> Result<serde_json::Value, String> {
    match serde_json::to_value(plot) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Failed to serialize plot: {}", e)),
    }
}

/// Generate all plot data for optimization results
pub fn generate_optimization_plots(params: OptimizationPlotParams) -> OptimizationPlots {
    // Generate plot frequencies
    let plot_freqs: Vec<f64> = (0..200)
        .map(|i| 20.0 * (1.0355_f64.powf(i as f64)))
        .collect();
    let plot_freqs_array = Array1::from(plot_freqs.clone());

    // Generate filter response data
    let eq_response = autoeq::x2peq::compute_peq_response_from_x(
        &plot_freqs_array,
        params.filter_params,
        params.sample_rate,
        params.peq_model,
    );

    let mut filter_curves = HashMap::new();
    filter_curves.insert("EQ Response".to_string(), eq_response.to_vec());
    let target_curve_for_plot = autoeq::Curve {
        freq: params.target_curve.freq.clone(),
        spl: params.target_curve.spl.clone(),
    };
    let target_interpolated = autoeq::read::interpolate(&plot_freqs_array, &target_curve_for_plot);
    filter_curves.insert("Target".to_string(), target_interpolated.spl.to_vec());

    let filter_response = PlotData {
        frequencies: plot_freqs.clone(),
        curves: filter_curves,
        metadata: HashMap::new(),
    };

    // Generate individual filter plots
    let mut individual_filter_curves = HashMap::new();
    let mut combined_response = Array1::zeros(plot_freqs_array.len());

    // Sort filters by frequency for consistent display
    let mut filters: Vec<(usize, f64, f64, f64)> = (0..params.num_filters)
        .map(|i| {
            (
                i,
                10f64.powf(params.filter_params[i * 3]), // Convert from log to linear frequency
                params.filter_params[i * 3 + 1],         // Q
                params.filter_params[i * 3 + 2],         // Gain
            )
        })
        .collect();
    filters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Generate response for each filter
    for (orig_i, f0, q, gain) in filters.into_iter() {
        use autoeq::iir::{Biquad, BiquadFilterType};

        let (ftype, label_prefix) = match params.peq_model {
            autoeq::cli::PeqModel::HpPk if orig_i == 0 => (BiquadFilterType::Highpass, "HPQ"),
            autoeq::cli::PeqModel::HpPkLp if orig_i == 0 => (BiquadFilterType::Highpass, "HPQ"),
            autoeq::cli::PeqModel::HpPkLp if orig_i == params.num_filters - 1 => {
                (BiquadFilterType::Lowpass, "LP")
            }
            _ => (BiquadFilterType::Peak, "PK"),
        };

        let filter = Biquad::new(ftype, f0, params.sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs_array);
        combined_response = &combined_response + &filter_response;

        let label = format!("{} {} at {:.0}Hz", label_prefix, orig_i + 1, f0);

        individual_filter_curves.insert(label, filter_response.to_vec());
    }

    // Add the combined sum
    individual_filter_curves.insert("Sum".to_string(), combined_response.to_vec());

    let filter_plots = PlotData {
        frequencies: plot_freqs.clone(),
        curves: individual_filter_curves,
        metadata: HashMap::new(),
    };

    // Generate spin details data if available
    let spin_details = params.spin_data.map(|spin| {
        let mut spin_curves = HashMap::new();
        for (name, curve) in spin {
            let interpolated = autoeq::read::interpolate(&plot_freqs_array, curve);
            spin_curves.insert(name.clone(), interpolated.spl.to_vec());
        }
        PlotData {
            frequencies: plot_freqs.clone(),
            curves: spin_curves,
            metadata: HashMap::new(),
        }
    });

    // Add input curve data
    let input_curve_plot = PlotData {
        frequencies: plot_freqs.clone(),
        curves: {
            let mut curves = HashMap::new();
            let input_interpolated =
                autoeq::read::interpolate(&plot_freqs_array, params.input_curve);
            curves.insert("Input".to_string(), input_interpolated.spl.to_vec());
            curves
        },
        metadata: HashMap::new(),
    };

    // Add deviation curve data (target - input, this is what needs to be corrected)
    let deviation_curve_plot = PlotData {
        frequencies: plot_freqs.clone(),
        curves: {
            let mut curves = HashMap::new();
            let deviation_interpolated =
                autoeq::read::interpolate(&plot_freqs_array, params.deviation_curve);
            curves.insert("Deviation".to_string(), deviation_interpolated.spl.to_vec());
            curves
        },
        metadata: HashMap::new(),
    };

    OptimizationPlots {
        filter_response,
        filter_plots,
        spin_details,
        input_curve: input_curve_plot,
        deviation_curve: deviation_curve_plot,
    }
}
