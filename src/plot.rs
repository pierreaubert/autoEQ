//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use ndarray::Array1;
use plotly::common::{Mode, Title};
use plotly::layout::{AxisType, GridPattern, LayoutGrid, RowOrder};
use plotly::{Layout, Plot, Scatter};

use crate::{Biquad, BiquadFilterType};

/// Get a color from the Plotly qualitative color palette
///
/// # Arguments
/// * `index` - Index of the color to retrieve (cycles through 10 colors)
///
/// # Returns
/// * Hex color code as a static string
///
/// # Details
/// Uses a predefined set of 10 colors from Plotly's qualitative palette.
/// Cycles through the colors when index exceeds the palette size.
fn filter_color(index: usize) -> &'static str {
    // Plotly qualitative color palette (10 colors)
    // Matches expectations in tests: index 0 -> #1f77b4, index 3 -> #d62728, index 9 -> #17becf
    const COLORS: [&str; 10] = [
        "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f",
        "#bcbd22", "#17becf",
    ];
    COLORS[index % COLORS.len()]
}

// Create two horizontal reference lines at y=1 and y=-1 spanning x=100..10000 for a given subplot axes
fn make_ref_lines(x_axis: &str, y_axis: &str) -> Vec<Scatter<f64, f64>> {
    let x_ref = vec![100.0_f64, 10000.0_f64];
    let y_pos = vec![1.0_f64, 1.0_f64];
    let y_neg = vec![-1.0_f64, -1.0_f64];

    let ref_pos = *Scatter::new(x_ref.clone(), y_pos)
        .mode(Mode::Lines)
        .name("+1 dB ref")
        .x_axis(x_axis)
        .y_axis(y_axis)
        .line(plotly::common::Line::new().color("#000000").width(1.0));
    let ref_neg = *Scatter::new(x_ref, y_neg)
        .mode(Mode::Lines)
        .name("-1 dB ref")
        .x_axis(x_axis)
        .y_axis(y_axis)
        .line(plotly::common::Line::new().color("#000000").width(1.0));

    vec![ref_pos, ref_neg]
}

/// Create CEA2034 traces for the combined plot
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
///
/// # Returns
/// * Vector of Scatter traces for CEA2034 curves
///
/// # Details
/// Creates traces for standard CEA2034 curves (On Axis, Listening Window,
/// Early Reflections, Sound Power) with appropriate fallback aliases
/// for variations in dataset labels.
fn create_cea2034_traces(curves: &HashMap<String, super::Curve>) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    // Primary curve names with possible fallback aliases to handle variations in dataset labels
    let curve_aliases: [&[&str]; 4] = [
        &["On Axis"],
        &["Listening Window", "Lateral"],
        &["Early Reflections", "Vertical"],
        &["Sound Power", "Estimated In-Room Response"],
    ];
    let axes = ["x3y3", "x4y4", "x5y5", "x6y6"];

    for (i, (aliases, axis)) in curve_aliases.iter().zip(axes.iter()).enumerate() {
        // Find the first alias present in the curves map
        if let Some((name, curve)) = aliases
            .iter()
            .find_map(|candidate| curves.get_key_value(*candidate))
        {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(name.as_str())
                .x_axis(&axis[..2])
                .y_axis(&axis[2..])
                .line(plotly::common::Line::new().color(filter_color(i)));
            traces.push(*trace);
        }
    }

    traces
}

/// Create CEA2034 traces with EQ response applied
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `eq_response` - Array of EQ response values to apply
///
/// # Returns
/// * Vector of Scatter traces for CEA2034 curves with EQ applied
///
/// # Details
/// Creates traces for standard CEA2034 curves with the EQ response
/// applied, using the same alias mapping as create_cea2034_traces.
fn create_cea2034_with_eq_traces(
    curves: &HashMap<String, super::Curve>,
    eq_response: &Array1<f64>,
) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    // Same alias mapping as create_cea2034_traces
    let curve_aliases: [&[&str]; 4] = [
        &["On Axis"],
        &["Listening Window", "Lateral"],
        &["Early Reflections", "Vertical"],
        &["Sound Power", "Estimated In-Room Response"],
    ];
    let axes = ["x3y3", "x4y4", "x5y5", "x6y6"];

    for (i, (aliases, axis)) in curve_aliases.iter().zip(axes.iter()).enumerate() {
        if let Some((name, curve)) = aliases
            .iter()
            .find_map(|candidate| curves.get_key_value(*candidate))
        {
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
                        .color(filter_color(i + curve_aliases.len()))
                        .width(2.0),
                );
            traces.push(*trace);
        }
    }

    traces
}

/// Generate and save an HTML plot comparing the input curve with the optimized EQ response.
///
/// # Arguments
/// * `args` - The list of args from the command line
/// * `input_curve` - The original frequency response curve
/// * `smoothed_curve` - Optional smoothed inverted target curve
/// * `target_curve` - The target curve
/// * `optimized_params` - The optimized filter parameters
/// * `output_path` - The path to save the HTML output file
/// * `cea2034_curves` - Optional CEA2034 curves to include in the plot
/// * `eq_response` - Optional EQ response to include in the plot
///
/// # Returns
/// * Result indicating success or failure
pub async fn plot_results(
    args: &super::Args,
    input_curve: &super::Curve,
    smoothed_curve: Option<&super::Curve>,
    target_curve: &Array1<f64>,
    optimized_params: &[f64],
    output_path: &PathBuf,
    cea2034_curves: Option<&HashMap<String, super::Curve>>,
    eq_response: Option<&Array1<f64>>,
) -> Result<(), Box<dyn Error>> {
    let num_filters = args.num_filters;
    let sample_rate = args.sample_rate;
    let speaker = args.speaker.as_deref();
    let iir_hp_pk = args.iir_hp_pk;
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
    // Prepare filters sorted by center frequency for display
    let mut filters: Vec<(usize, f64, f64, f64)> = (0..num_filters)
        .map(|i| {
            (
                i,
                optimized_params[i * 3],
                optimized_params[i * 3 + 1],
                optimized_params[i * 3 + 2],
            )
        })
        .collect();
    filters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for (display_idx, (orig_i, f0, q, gain)) in filters.into_iter().enumerate() {
        let ftype = if iir_hp_pk && orig_i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, f0, sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs);
        combined_response = combined_response + &filter_response;

        let label = if iir_hp_pk && orig_i == hp_index {
            "Highpass"
        } else {
            "Peak"
        };
        let individual_trace = Scatter::new(plot_freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Lines)
            .name(&format!("{} {} at {:5.0}Hz", label, orig_i + 1, f0))
            .y_axis("y")
            .marker(
                plotly::common::Marker::new()
                    .color(filter_color(display_idx))
                    .size(1),
            );
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

    let iir_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("IIR Response")
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("#2ca02c"));
    plot.add_trace(iir_trace);

    // ----------------------------------------------------------------------
    // Second subplot: inverted target curve (possibly smoothed)
    // ----------------------------------------------------------------------
    // If smoothing enabled, add the smoothed inverted target curve trace
    if let Some(sm) = smoothed_curve {
        let smoothed_trace = Scatter::new(sm.freq.to_vec(), sm.spl.to_vec())
            .mode(Mode::Lines)
            .name("Smoothed Inverted Target")
            .x_axis("x2")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#9467bd"));
        plot.add_trace(smoothed_trace);
    } else {
        let target_trace = Scatter::new(input_curve.freq.to_vec(), target_curve.to_vec())
            .mode(Mode::Lines)
            .name("Inverted Target")
            .x_axis("x2")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#1f77b4"));
        plot.add_trace(target_trace);
    }

    // ----------------------------------------------------------------------
    // Add CEA2034 curves if provided
    // ----------------------------------------------------------------------
    let mut x_axis3_title = "On Axis".to_string();
    let mut x_axis4_title = "Listening Window".to_string();
    let mut x_axis5_title = "Early Reflections".to_string();
    let mut x_axis6_title = "Sound Power".to_string();
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
    } else {
        // No CEA2034 data: show input curve (left) and input + PEQ (right) on second row
        x_axis3_title = format!("{} -- Frequency (Hz)", args.curve_name);
        x_axis4_title = format!("{} EQ -- Frequency (Hz)", args.curve_name);
        x_axis5_title = "unused".to_string();
        x_axis6_title = "unused".to_string();
        // Interpolate input to plotting freqs to align with combined_response
        let input_on_plot =
            crate::read::interpolate(&plot_freqs, &input_curve.freq, &input_curve.spl);

        // Left subplot (x3/y3): Input Curve
        let input_second_row = Scatter::new(plot_freqs.to_vec(), input_on_plot.to_vec())
            .mode(Mode::Lines)
            .name(args.curve_name.clone())
            .x_axis("x3")
            .y_axis("y3")
            .line(plotly::common::Line::new().color("#1f77b4"));
        plot.add_trace(input_second_row);

        // Right subplot (x4/y4): Input Curve + PEQ response
        let input_plus_peq = &input_on_plot + &combined_response;
        let input_plus_peq_trace = Scatter::new(plot_freqs.to_vec(), input_plus_peq.to_vec())
            .mode(Mode::Lines)
            .name(format!("{} + EQ", args.curve_name))
            .x_axis("x4")
            .y_axis("y4")
            .line(plotly::common::Line::new().color("#2ca02c"));
        plot.add_trace(input_plus_peq_trace);
    }

    // Add reference lines y=1 and y=-1 from x=100 to x=10000 (black) on both subplots
    for t in make_ref_lines("x3", "y3") {
        plot.add_trace(Box::new(t));
    }
    for t in make_ref_lines("x4", "y4") {
        plot.add_trace(Box::new(t));
    }

    // ----------------------------------------------------------------------
    // Title with optional speaker name
    // ----------------------------------------------------------------------
    let title_text = match speaker {
        Some(s) if !s.is_empty() => format!("{} -- #{} peq(s)", s, num_filters),
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
                .title(plotly::common::Title::with_text(x_axis3_title))
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
                .title(plotly::common::Title::with_text(x_axis4_title))
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
                .title(plotly::common::Title::with_text(x_axis5_title))
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
                .title(plotly::common::Title::with_text(x_axis6_title))
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

    plot.write_html(output_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        create_cea2034_traces, create_cea2034_with_eq_traces, filter_color, make_ref_lines,
    };
    use ndarray::Array1;
    use serde_json::json;
    use serde_json::to_value as to_json;
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

    #[test]
    fn test_make_ref_lines_values() {
        let lines = make_ref_lines("x3", "y3");
        assert_eq!(lines.len(), 2);
        let v0 = to_json(&lines[0]).unwrap();
        let v1 = to_json(&lines[1]).unwrap();
        assert_eq!(v0["x"], json!([100.0, 10000.0]));
        assert_eq!(v1["x"], json!([100.0, 10000.0]));
        assert_eq!(v0["y"], json!([1.0, 1.0]));
        assert_eq!(v1["y"], json!([-1.0, -1.0]));
    }
}
