use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use eqopt::{Biquad, BiquadFilterType};
use ndarray::Array1;
use plotly::common::{Mode, Title};
use plotly::layout::{AxisType, GridPattern, Layout, LayoutGrid, RowOrder};
use plotly::{Plot, Scatter};

fn filter_color(index: usize) -> &'static str {
    const COLORS: [&str; 10] = [
        "#5c77a5", "#dc842a", "#c85857", "#89b5b1", "#71a152", "#bab0ac", "#e15759", "#b07aa1",
        "#76b7b2", "#ff9da7",
    ];
    COLORS[index % COLORS.len()]
}

fn vline_points(x: f64, y_min: f64, y_max: f64) -> (Vec<f64>, Vec<f64>) {
    (vec![x, x], vec![y_min, y_max])
}

// Clamp an Array1 of dB values to ±max_db
fn apply_db_clamp(arr: &Array1<f64>, max_db: f64) -> Array1<f64> {
    arr.mapv(|v| v.max(-max_db).min(max_db))
}

// Create CEA2034 traces for the combined plot
fn create_cea2034_traces(curves: &HashMap<String, super::Curve>) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
    ];
    let axes = ["x3y3", "x4y4", "x5y5", "x6y6"];

    for (i, (name, axis)) in curve_names.iter().zip(axes.iter()).enumerate() {
        if let Some(curve) = curves.get(*name) {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(name)
                .x_axis(&axis[..2])
                .y_axis(&axis[2..])
                .line(plotly::common::Line::new().color(filter_color(i)));
            traces.push(*trace);
        }
    }

    traces
}

// Create CEA2034 traces with EQ response applied
fn create_cea2034_with_eq_traces(
    curves: &HashMap<String, super::Curve>,
    eq_response: &Array1<f64>,
) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
    ];
    let axes = ["x3y3", "x4y4", "x5y5", "x6y6"];

    for (i, (name, axis)) in curve_names.iter().zip(axes.iter()).enumerate() {
        if let Some(curve) = curves.get(*name) {
            // Apply EQ response to the curve
            let eq_applied: Vec<f64> = curve
                .spl
                .iter()
                .zip(eq_response.iter())
                .map(|(spl, eq)| spl + eq)
                .collect();

            let trace = Scatter::new(curve.freq.to_vec(), eq_applied)
                .mode(Mode::Lines)
                .name(format!("{} + EQ", name))
                .x_axis(&axis[..2])
                .y_axis(&axis[2..])
                .line(
                    plotly::common::Line::new()
                        .color(filter_color(i + curve_names.len()))
                        .width(2.0),
                );
            traces.push(*trace);
        }
    }

    traces
}

pub async fn plot_results(
    input_curve: &super::Curve,
    optimized_params: &[f64],
    num_filters: usize,
    sample_rate: f64,
    _max_db: f64,
    smoothed: Option<&Array1<f64>>,
    output_path: &PathBuf,
    speaker: Option<&str>,
    measurement: Option<&str>,
    iir_hp_pk: bool,
    cea2034_curves: Option<&HashMap<String, super::Curve>>,
    eq_response: Option<&Array1<f64>>,
) -> Result<(), Box<dyn Error>> {
    // Create a dense frequency vector for smooth plotting
    let mut freqs = Vec::new();
    let mut freq = 20.0;
    while freq <= 20000.0 {
        freqs.push(freq);
        freq *= 1.0355; // Logarithmic spacing with ~200 points
    }
    let plot_freqs = Array1::from(freqs);

    // Calculate combined IIR response
    let mut combined_response: Array1<f64> = Array1::zeros(plot_freqs.len());

    // Create a single plot with subplots
    let mut plot = Plot::new();

    // If enabled: lowest-frequency filter index -> Highpass; otherwise all Peak
    let mut hp_index = usize::MAX;
    if iir_hp_pk {
        hp_index = 0usize;
        if num_filters > 0 {
            let mut min_f = optimized_params[0];
            for i in 1..num_filters {
                let f = optimized_params[i * 3];
                if f < min_f {
                    min_f = f;
                    hp_index = i;
                }
            }
        }
    }

    // ----------------------------------------------------------------------
    // First subplot: Individual filters (y axis)
    // ----------------------------------------------------------------------
    for i in 0..num_filters {
        let f0 = optimized_params[i * 3];
        let q = optimized_params[i * 3 + 1];
        let gain = optimized_params[i * 3 + 2];

        let ftype = if iir_hp_pk && i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, f0, sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs);
        combined_response = combined_response + &filter_response;

        let label = if iir_hp_pk && i == hp_index {
            "Highpass"
        } else {
            "Peak"
        };
        let individual_trace = Scatter::new(plot_freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Lines)
            .name(&format!("Filter {} ({} at {:5.0}Hz)", i + 1, label, f0))
            .y_axis("y")
            .marker(plotly::common::Marker::new().color(filter_color(i)).size(1));
        plot.add_trace(individual_trace);
    }

    // Add total combined response on the first subplot
    let total_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("Total Filter Response")
        .x_axis("x")
        .y_axis("y")
        .line(plotly::common::Line::new().color("#000000").width(2.0));
    plot.add_trace(total_trace);

    // ----------------------------------------------------------------------
    // Second subplot: Input curve and IIR response (not inverted) (y2 axis)
    // ----------------------------------------------------------------------
    let measurement_name = measurement.unwrap_or("Input Curve");

    let iir_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("IIR Response")
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("#2ca02c"));
    plot.add_trace(iir_trace);

    // If smoothing enabled, add the smoothed inverted target curve trace
    if let Some(sm) = smoothed {
        let smoothed_trace = Scatter::new(input_curve.freq.to_vec(), sm.to_vec())
            .mode(Mode::Lines)
            .name("Smoothed Inverted Target")
            .x_axis("x2")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#9467bd"));
        plot.add_trace(smoothed_trace);
    } else {
        let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
            .mode(Mode::Lines)
            .name(measurement_name)
            .x_axis("x2")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#1f77b4"));
        plot.add_trace(input_trace);
    }

    // ----------------------------------------------------------------------
    // Add CEA2034 curves if provided
    // ----------------------------------------------------------------------
    if let Some(curves) = cea2034_curves {
        // Create CEA2034 traces
        let cea2034_traces = create_cea2034_traces(curves);
        for trace in cea2034_traces {
            plot.add_trace(Box::new(trace));
        }

        // If EQ response is provided, create CEA2034 with EQ traces
        if let Some(eq_resp) = eq_response {
            let cea2034_eq_traces = create_cea2034_with_eq_traces(curves, eq_resp);
            for trace in cea2034_eq_traces {
                plot.add_trace(Box::new(trace));
            }
        }
    }

    // ----------------------------------------------------------------------
    // Title with optional speaker name
    // ----------------------------------------------------------------------
    let title_text = match speaker {
        Some(s) if !s.is_empty() => format!("{} -- #{}", s, num_filters),
        _ => "IIR Filter Optimization Results".to_string(),
    };

    // Configure layout with subplots
    let layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(3)
                .columns(2)
                .pattern(GridPattern::Independent)
                .row_order(RowOrder::BottomToTop),
        )
        .title(Title::with_text(&title_text))
        .width(1024)
        .height(1000)
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        ) // log10(20) to log10(20000)
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-5.0, 10.0]) // limit filter subplot range per request
                .domain(&[0.8, 1.0]),
        )
        .x_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        ) // log10(20) to log10(20000)
        .y_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-5.0, 10.0])
                .domain(&[0.8, 1.0]),
        )
        // CEA2034 subplot axes
        .x_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "On Axis -- Frequency (Hz)",
                ))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.4, 0.75]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "Listening Window -- Frequency (Hz)",
                ))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.4, 0.75]),
        )
        .x_axis5(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "Early Reflections -- Frequency (Hz)",
                ))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        )
        .y_axis5(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0., 0.35]),
        )
        .x_axis6(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "Sound Power -- Frequency (Hz)",
                ))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis6(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0., 0.35]),
        );
    plot.set_layout(layout);

    // Save to file
    plot.write_html(output_path);
    println!(
        "\n📊 Interactive plot with subplots saved to {:?}",
        output_path
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_db_clamp, create_cea2034_traces, create_cea2034_with_eq_traces, filter_color,
        vline_points,
    };
    use ndarray::Array1;
    use std::collections::HashMap;

    #[test]
    fn color_palette_cycles() {
        assert_eq!(filter_color(0), "#1f77b4");
        assert_eq!(filter_color(3), "#d62728");
        assert_eq!(filter_color(9), "#17becf");
        // Cycle wraps around
        assert_eq!(filter_color(10), "#1f77b4");
        assert_eq!(filter_color(13), "#d62728");
    }

    #[test]
    fn vline_points_two_points() {
        let (xs, ys) = vline_points(1000.0, -5.0, 5.0);
        assert_eq!(xs, vec![1000.0, 1000.0]);
        assert_eq!(ys, vec![-5.0, 5.0]);
    }

    #[test]
    fn clamp_limits_values() {
        let arr = Array1::from(vec![-30.0, -10.0, 0.0, 10.0, 30.0]);
        let out = apply_db_clamp(&arr, 12.0);
        assert_eq!(out.to_vec(), vec![-12.0, -10.0, 0.0, 10.0, 12.0]);
    }

    #[test]
    fn test_create_cea2034_traces() {
        // Create mock CEA2034 curves
        let mut curves = HashMap::new();

        // Create a simple frequency grid
        let freq = Array1::from(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]);
        let spl = Array1::from(vec![80.0, 85.0, 90.0, 85.0, 80.0]);

        // Add mock curves
        curves.insert(
            "On Axis".to_string(),
            super::super::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Lateral".to_string(),
            super::super::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Vertical".to_string(),
            super::super::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Estimated In-Room Response".to_string(),
            super::super::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );

        // Test creating CEA2034 traces
        let traces = create_cea2034_traces(&curves);

        // Should have 4 traces
        assert_eq!(traces.len(), 4);

        // Test creating CEA2034 traces with EQ
        let eq_response = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 1.0]);
        let eq_traces = create_cea2034_with_eq_traces(&curves, &eq_response);

        // Should have 4 traces
        assert_eq!(eq_traces.len(), 4);
    }
}
