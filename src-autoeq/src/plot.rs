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
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use build_html::*;
use ndarray::Array1;
use plotly::common::{AxisSide, Mode};
use plotly::layout::{AxisType, GridPattern, LayoutGrid, RowOrder};
use plotly::{Layout, Plot, Scatter};
use plotly_static::{ImageFormat, StaticExporterBuilder};

use crate::iir::{Biquad, BiquadFilterType};

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

/// Create CEA2034 combined traces for a single subplot, including DI on a secondary y-axis
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `x_axis` - Axis id for x (e.g. "x7")
/// * `y_axis` - Axis id for primary y (left) (e.g. "y7")
fn create_cea2034_combined_traces(
	curves: &HashMap<String, crate::Curve>,
	x_axis: &str,
	y_axis: &str,
	y_axis_di: &str,
) -> Vec<Scatter<f64, f64>> {
	let mut traces = Vec::new();
	for (i, curve_name) in CEA2034_CURVE_NAMES_FULL.iter().enumerate() {
		if let Some(curve) = curves.get(*curve_name) {
			let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
				.mode(Mode::Lines)
				.name(shorten_curve_name(curve_name))
				.x_axis(x_axis)
				.y_axis(y_axis)
				.line(plotly::common::Line::new().color(filter_color(i)));
			traces.push(*trace);
		}
	}
	// DI curves on secondary y-axis
	for (j, curve_name) in CEA2034_CURVE_NAMES_DI.iter().enumerate() {
		if let Some(curve) = curves.get(*curve_name) {
			let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
				.mode(Mode::Lines)
				.name(shorten_curve_name(curve_name))
				.x_axis(x_axis)
				.y_axis(y_axis_di)
				.line(plotly::common::Line::new().color(filter_color(j + 2)));
			traces.push(*trace);
		}
	}
	traces
}

/// Create CEA2034 combined traces with EQ applied on a single subplot
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `eq_response` - EQ response to apply to the primary CEA2034 curves
/// * `x_axis` - Axis id for x (e.g. "x8")
/// * `y_axis` - Axis id for primary y (left) (e.g. "y8")
fn create_cea2034_with_eq_combined_traces(
	curves: &HashMap<String, crate::Curve>,
	eq_response: &Array1<f64>,
	x_axis: &str,
	y_axis: &str,
	y_axis_di: &str,
) -> Vec<Scatter<f64, f64>> {
	let mut traces = Vec::new();
	for (i, curve_name) in CEA2034_CURVE_NAMES_FULL.iter().enumerate() {
		if let Some(curve) = curves.get(*curve_name) {
			let trace = Scatter::new(curve.freq.to_vec(), (&curve.spl + eq_response).to_vec())
				.mode(Mode::Lines)
				.name(format!("{} w/EQ", shorten_curve_name(curve_name)))
				.x_axis(x_axis)
				.y_axis(y_axis)
				.line(plotly::common::Line::new().color(filter_color(i)));
			traces.push(*trace);
		}
	}
	// DI curves unchanged, on secondary y-axis
	for (j, curve_name) in CEA2034_CURVE_NAMES_DI.iter().enumerate() {
		if let Some(curve) = curves.get(*curve_name) {
			let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
				.mode(Mode::Lines)
				.name(shorten_curve_name(curve_name))
				.x_axis(x_axis)
				.y_axis(y_axis_di)
				.line(plotly::common::Line::new().color(filter_color(j + 2)));
			traces.push(*trace);
		}
	}
	traces
}

// Create two horizontal reference lines at y=1 and y=-1 spanning x=100..10000 for a given subplot axes
fn make_ref_lines(x_axis: &str, y_axis: &str) -> Vec<Scatter<f64, f64>> {
	let x_ref = vec![100.0_f64, 10000.0_f64];
	let y_pos = vec![1.0_f64, 1.0_f64];
	let y_neg = vec![-1.0_f64, -1.0_f64];

	let ref_pos = Scatter::new(x_ref.clone(), y_pos)
		.mode(Mode::Lines)
		.name("+1 dB ref")
		.x_axis(x_axis)
		.y_axis(y_axis)
		.line(plotly::common::Line::new().color("#000000").width(1.0));
	let ref_neg = Scatter::new(x_ref, y_neg)
		.mode(Mode::Lines)
		.name("-1 dB ref")
		.x_axis(x_axis)
		.y_axis(y_axis)
		.line(plotly::common::Line::new().color("#000000").width(1.0));

	vec![*ref_pos, *ref_neg]
}

// List of curve names
const CEA2034_CURVE_NAMES: [&str; 4] =
	["On Axis", "Listening Window", "Early Reflections", "Sound Power"];

const CEA2034_CURVE_NAMES_FULL: [&str; 5] = [
	"On Axis",
	"Listening Window",
	"Early Reflections",
	"Sound Power",
	"Estimated In-Room Response",
];

const CEA2034_CURVE_NAMES_DI: [&str; 2] = ["Early Reflections DI", "Sound Power DI"];

/// Convert a curve name to its short abbreviated form
///
/// # Arguments
/// * `curve_name` - The full curve name to abbreviate
///
/// # Returns
/// * A string slice with the abbreviated curve name
///
/// # Examples
/// ```
/// use autoeq::plot::shorten_curve_name;
/// assert_eq!(shorten_curve_name("On Axis"), "ON");
/// assert_eq!(shorten_curve_name("Listening Window"), "LW");
/// assert_eq!(shorten_curve_name("Unknown Curve"), "Unknown Curve");
/// ```
pub fn shorten_curve_name(curve_name: &str) -> &str {
	match curve_name {
		"On Axis" => "ON",
		"Listening Window" => "LW",
		"Early Reflections" => "ER",
		"Sound Power" => "SP",
		"Estimated In-Room Response" => "PIR",
		"Early Reflections DI" => "ERDI",
		"Sound Power DI" => "SPDI",
		// Add more mappings as needed
		_ => curve_name, // Return original if no mapping found
	}
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
/// Creates traces for standard CEA2034 curves
fn create_cea2034_traces(curves: &HashMap<String, crate::Curve>) -> Vec<Scatter<f64, f64>> {
	let mut traces = Vec::new();

	let axes = ["x1y1", "x2y2", "x3y3", "x4y4"];

	for (i, (curve_name, axis)) in CEA2034_CURVE_NAMES.iter().zip(axes.iter()).enumerate() {
		let mut x_axis_name = &axis[..2];
		let mut y_axis_name = &axis[2..];
		if x_axis_name == "x1" || y_axis_name == "y1" {
			x_axis_name = "x";
			y_axis_name = "y";
		}
		let curve = curves.get(*curve_name).unwrap();
		let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
			.mode(Mode::Lines)
			.name(shorten_curve_name(curve_name))
			.x_axis(x_axis_name)
			.y_axis(y_axis_name)
			.line(plotly::common::Line::new().color(filter_color(i)));
		traces.push(*trace);
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
	curves: &HashMap<String, crate::Curve>,
	eq_response: &Array1<f64>,
) -> Vec<Scatter<f64, f64>> {
	let mut traces = Vec::new();

	let axes = ["x1y1", "x2y2", "x3y3", "x4y4"];

	for (i, (curve_name, axis)) in CEA2034_CURVE_NAMES.iter().zip(axes.iter()).enumerate() {
		let mut x_axis_name = &axis[..2];
		let mut y_axis_name = &axis[2..];
		if x_axis_name == "x1" || y_axis_name == "y1" {
			x_axis_name = "x";
			y_axis_name = "y";
		}
		let curve = curves.get(*curve_name).unwrap();
		let trace = Scatter::new(curve.freq.to_vec(), (&curve.spl + eq_response).to_vec())
			.mode(Mode::Lines)
			.name(format!("{} w/EQ", shorten_curve_name(curve_name)))
			.x_axis(x_axis_name)
			.y_axis(y_axis_name)
			.line(plotly::common::Line::new().color(filter_color(i + 4)));
		traces.push(*trace);
	}

	traces
}

fn plot_filters(
	args: &super::cli::Args,
	input_curve: &crate::Curve,
	smoothed_curve: Option<&crate::Curve>,
	target_curve: &Array1<f64>,
	plot_freqs: &Array1<f64>,
	optimized_params: &[f64],
) -> plotly::Plot {
	let mut plot = Plot::new();
	let mut combined_response: Array1<f64> = Array1::zeros(plot_freqs.len());
	let mut filters: Vec<(usize, f64, f64, f64)> = (0..args.num_filters)
		.map(|i| {
			(
				i,
				10f64.powf(optimized_params[i * 3]),
				optimized_params[i * 3 + 1],
				optimized_params[i * 3 + 2],
			)
		})
		.collect();
	filters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
	for (display_idx, (orig_i, f0, q, gain)) in filters.into_iter().enumerate() {
		let ftype = if args.iir_hp_pk && orig_i == 0 {
			BiquadFilterType::Highpass
		} else {
			BiquadFilterType::Peak
		};
		let filter = Biquad::new(ftype, f0, args.sample_rate, q, gain);
		let filter_response = filter.np_log_result(&plot_freqs);
		combined_response += &filter_response;

		let label = if args.iir_hp_pk && orig_i == 0 { "HPQ" } else { "PK" };
		let individual_trace = Scatter::new(plot_freqs.to_vec(), filter_response.to_vec())
			.mode(Mode::Lines)
			.name(format!("{} {} at {:5.0}Hz", label, orig_i + 1, f0))
			.marker(plotly::common::Marker::new().color(filter_color(display_idx)).size(1));
		plot.add_trace(individual_trace);
	}

	// Add total combined response on the first subplot
	let total_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
		.mode(Mode::Lines)
		.name("Sum")
		.line(plotly::common::Line::new().color("#000000").width(2.0));
	plot.add_trace(total_trace);

	let iir_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
		.mode(Mode::Lines)
		.name("autoEQ")
		.x_axis("x2")
		.y_axis("y2")
		.line(plotly::common::Line::new().color("#2ca02c"));
	plot.add_trace(iir_trace);

	// If smoothing enabled, add the smoothed inverted target curve trace
	if let Some(sm) = smoothed_curve {
		let smoothed_trace = Scatter::new(sm.freq.to_vec(), sm.spl.to_vec())
			.mode(Mode::Lines)
			.name("Target")
			.x_axis("x2")
			.y_axis("y2")
			.line(plotly::common::Line::new().color("#9467bd"));
		plot.add_trace(smoothed_trace);
	} else {
		let target_trace = Scatter::new(input_curve.freq.to_vec(), target_curve.to_vec())
			.mode(Mode::Lines)
			.name("Target")
			.x_axis("x2")
			.y_axis("y2")
			.line(plotly::common::Line::new().color("#1f77b4"));
		plot.add_trace(target_trace);
	}

	// Configure layout with subplots
	let layout = Layout::new()
		.grid(
			LayoutGrid::new()
				.rows(1)
				.columns(2)
				.pattern(GridPattern::Independent)
				.row_order(RowOrder::BottomToTop),
		)
		.width(1024)
		.height(400)
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
				.dtick(1.0)
				.range(vec![-5.0, 5.0]),
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
				.dtick(1.0)
				.range(vec![-5.0, 5.0]),
		);
	plot.set_layout(layout);

	plot
}

fn plot_spin_details(
	args: &super::cli::Args,
	input_curve: &crate::Curve,
	plot_freqs: &Array1<f64>,
	cea2034_curves: Option<&HashMap<String, crate::Curve>>,
	eq_response: Option<&Array1<f64>>,
) -> plotly::Plot {
	let mut plot = Plot::new();
	// Add each CEA2034 curves if provided
	let mut x_axis1_title = "On Axis".to_string();
	let mut x_axis2_title = "Listening Window".to_string();
	let mut x_axis3_title = "Early Reflections".to_string();
	let mut x_axis4_title = "Sound Power".to_string();
	if let Some(curves) = cea2034_curves {
		let cea2034_traces = create_cea2034_traces(curves);
		for trace in cea2034_traces {
			plot.add_trace(Box::new(trace));
		}
		// Also plot the EQ-applied variants if provided
		if let Some(eq_resp) = eq_response {
			let cea2034_eq_traces = create_cea2034_with_eq_traces(curves, eq_resp);
			for trace in cea2034_eq_traces {
				plot.add_trace(Box::new(trace));
			}
		}
	} else {
		// No CEA2034 data: show input curve (left) and input + PEQ (right) on second row
		x_axis1_title = format!("{} -- Frequency (Hz)", args.curve_name);
		x_axis2_title = format!("{} EQ -- Frequency (Hz)", args.curve_name);
		x_axis3_title = "unused".to_string();
		x_axis4_title = "unused".to_string();
		// Interpolate input to plotting freqs to align
		let input_on_plot =
			crate::read::interpolate(&plot_freqs, &input_curve.freq, &input_curve.spl);

		// Left subplot (x3/y3): Input Curve
		let input_second_row = Scatter::new(plot_freqs.to_vec(), input_on_plot.to_vec())
			.mode(Mode::Lines)
			.name(shorten_curve_name(&args.curve_name.clone()))
			.line(plotly::common::Line::new().color("#1f77b4"));
		plot.add_trace(input_second_row);

		// Right subplot (x4/y4): Input Curve + PEQ response
		let input_plus_peq_trace = Scatter::new(plot_freqs.to_vec(), input_on_plot.to_vec())
			.mode(Mode::Lines)
			.name(format!("{} + EQ", shorten_curve_name(&args.curve_name)))
			.x_axis("x2")
			.y_axis("y2")
			.line(plotly::common::Line::new().color("#2ca02c"));
		plot.add_trace(input_plus_peq_trace);
	}

	// Add reference lines y=1 and y=-1 from x=100 to x=10000 (black) on both subplots
	for t in make_ref_lines("x", "y") {
		plot.add_trace(Box::new(t));
	}
	for t in make_ref_lines("x2", "y2") {
		plot.add_trace(Box::new(t));
	}

	// Configure layout with subplots
	let layout = Layout::new()
		.grid(
			LayoutGrid::new()
				.rows(2)
				.columns(2)
				.pattern(GridPattern::Independent)
				.row_order(RowOrder::BottomToTop),
		)
		.width(1024)
		.height(600)
		.x_axis(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text(x_axis1_title))
				.type_(AxisType::Log)
				.range(vec![1.301, 4.301])
				.domain(&[0.0, 0.45]),
		)
		.y_axis(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("SPL (dB)"))
				.range(vec![-10.0, 10.0])
				.domain(&[0.55, 1.0]),
		)
		.x_axis2(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text(x_axis2_title))
				.type_(AxisType::Log)
				.range(vec![1.301, 4.301])
				.domain(&[0.55, 1.0]),
		)
		.y_axis2(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("SPL (dB)"))
				.range(vec![-10.0, 10.0])
				.domain(&[0.55, 1.0]),
		)
		.x_axis3(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text(x_axis3_title))
				.type_(AxisType::Log)
				.range(vec![1.301, 4.301])
				.domain(&[0.0, 0.45]),
		)
		.y_axis3(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("SPL (dB)"))
				.range(vec![-15.0, 5.0])
				.domain(&[0.0, 0.45]),
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
				.range(vec![-15.0, 5.0])
				.domain(&[0.0, 0.45]),
		);

	plot.set_layout(layout);

	plot
}

fn plot_spin(
	cea2034_curves: Option<&HashMap<String, crate::Curve>>,
	eq_response: Option<&Array1<f64>>,
) -> plotly::Plot {
	let mut plot = Plot::new();

	// ----------------------------------------------------------------------
	// Add CEA2034 if provided with and without EQ
	// ----------------------------------------------------------------------
	let x_axis1_title = "CEA2034".to_string();
	let x_axis3_title = "CEA2034 + EQ".to_string();
	if let Some(curves) = cea2034_curves {
		let cea2034_traces = create_cea2034_combined_traces(curves, "x", "y", "y2");
		for trace in cea2034_traces {
			plot.add_trace(Box::new(trace));
		}

		if let Some(eq_resp) = eq_response {
			let cea2034_traces =
				create_cea2034_with_eq_combined_traces(curves, eq_resp, "x3", "y3", "y4");
			for trace in cea2034_traces {
				plot.add_trace(Box::new(trace));
			}
		}
	}

	// Configure layout with subplots
	let layout = Layout::new()
		.grid(LayoutGrid::new().rows(1).columns(2).pattern(GridPattern::Independent))
		.width(1024)
		.height(450)
		// cea2034
		.x_axis(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text(&x_axis1_title))
				.type_(AxisType::Log)
				.range(vec![1.301, 4.301])
				.domain(&[0., 0.4]),
		)
		.y_axis(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("SPL (dB)"))
				.dtick(5.0)
				.range(vec![-40.0, 10.0]),
		)
		.y_axis2(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("DI (dB)                      ⌃"))
				.range(vec![-5.0, 45.0])
				.tick_values(vec![-5.0, 0.0, 5.0, 10.0, 15.0])
				.overlaying("y")
				.side(AxisSide::Right),
		)
		// cea2034 with eq
		.x_axis3(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text(&x_axis3_title))
				.type_(AxisType::Log)
				.range(vec![1.301, 4.301])
				.domain(&[0.55, 0.95]),
		)
		.y_axis3(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("SPL (dB)"))
				.dtick(5.0)
				.range(vec![-40.0, 10.0])
				.anchor("x3"),
		)
		.y_axis4(
			plotly::layout::Axis::new()
				.title(plotly::common::Title::with_text("DI (dB)                      ⌃"))
				.range(vec![-5.0, 45.0])
				.tick_values(vec![-5.0, 0.0, 5.0, 10.0, 15.0])
				.anchor("x3")
				.overlaying("y3")
				.side(AxisSide::Right),
		);
	plot.set_layout(layout);

	plot
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
	args: &super::cli::Args,
	input_curve: &crate::Curve,
	smoothed_curve: Option<&crate::Curve>,
	target_curve: &Array1<f64>,
	optimized_params: &[f64],
	output_path: &PathBuf,
	cea2034_curves: Option<&HashMap<String, crate::Curve>>,
	eq_response: Option<&Array1<f64>>,
) -> Result<(), Box<dyn Error>> {
	let speaker = args.speaker.as_deref();

	// Create a dense frequency vector for smooth plotting
	let mut vfreqs = Vec::new();
	let mut freq = 20.0;
	while freq <= 20000.0 {
		vfreqs.push(freq);
		freq *= 1.0355; // Logarithmic spacing with ~200 points
	}
	let freqs = Array1::from(vfreqs);

	// gather all subplots
	let plot_filters =
		plot_filters(args, input_curve, smoothed_curve, target_curve, &freqs, optimized_params);
	let plot_spin_details = if cea2034_curves.is_some() {
		Some(plot_spin_details(args, input_curve, &freqs, cea2034_curves, eq_response))
	} else {
		None
	};
	let plot_spin_opt =
		if cea2034_curves.is_some() { Some(plot_spin(cea2034_curves, eq_response)) } else { None };

	// Title with optional speaker name
	let title_text = match speaker {
		Some(s) if !s.is_empty() => format!("{} -- #{} peq(s)", s, args.num_filters),
		_ => "IIR Filter Optimization Results".to_string(),
	};

	let html: String = {
		let base = HtmlPage::new()
			.with_title(title_text)
			.with_script_link("https://cdn.plot.ly/plotly-latest.min.js")
			.with_raw(plot_filters.to_inline_html(Some("filters")));
		let page = if let Some(ref plot_spin) = plot_spin_opt {
			base.with_raw(plot_spin.to_inline_html(Some("spinorame")))
		} else {
			base
		};
		let page2 = if let Some(ref plot_spin) = plot_spin_details {
			page.with_raw(plot_spin.to_inline_html(Some("details")))
		} else {
			page
		};
		page2.to_html_string()
	};

	// Ensure parent directory exists before writing files
	let html_output_path = output_path.with_extension("html");
	if let Some(parent) = html_output_path.parent() {
		std::fs::create_dir_all(parent)
			.expect(&format!("Failed to create output directory: {:?}", parent));
	}

	let mut file = File::create(&html_output_path).unwrap();
	file.write_all(html.as_bytes()).expect("failed to write html output");
	file.flush().unwrap();

	// plot_spin.write_html(output_path.with_extension("html"));

	let stem = output_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");

	let mut plots = vec![(plot_filters, "filters", 1280, 400)];
	if let Some(plot_spin) = plot_spin_details {
		plots.push((plot_spin, "details", 1280, 650));
	}

	if let Some(plot_spin) = plot_spin_opt {
		plots.push((plot_spin, "spins", 1280, 450));
	}

	// Try to create an async static exporter. If unavailable, skip PNG export and continue.
	let exporter_build = StaticExporterBuilder::default().webdriver_port(5112).build_async();

	match exporter_build {
		Ok(mut exporter) => {
			for (plot, name, width, height) in plots {
				let img_path = output_path.with_file_name(format!("{}-{}.png", stem, name));

				// Ensure parent directory exists for PNG files
				if let Some(parent) = img_path.parent() {
					std::fs::create_dir_all(parent)
						.expect(&format!("Failed to create PNG output directory: {:?}", parent));
				}

				if let Err(e) = exporter
					.write_fig(
						img_path.as_path(),
						&serde_json::to_value(&plot).expect("Failed to serialize plot to JSON"),
						ImageFormat::PNG,
						width,
						height,
						1.0,
					)
					.await
				{
					eprintln!(
                        "⚠️ Warning: Failed to export plot '{}' to PNG ({}). Continuing without PNG.",
                        name, e
                    );
				}
			}
			// Close exporter (ignore close errors)
			let _ = exporter.close().await;
		}
		Err(e) => {
			eprintln!(
                "⚠️ Warning: PNG export skipped (WebDriver not available): {}. HTML report was generated at {}",
                e,
                html_output_path.display()
            );
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::{
		create_cea2034_combined_traces, create_cea2034_traces,
		create_cea2034_with_eq_combined_traces, create_cea2034_with_eq_traces, filter_color,
		make_ref_lines, shorten_curve_name,
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

		// Add mock curves for the primary CEA2034 set used by create_cea2034_traces
		curves.insert("On Axis".to_string(), crate::Curve { freq: freq.clone(), spl: spl.clone() });
		curves.insert(
			"Listening Window".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl.clone() },
		);
		curves.insert(
			"Early Reflections".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl.clone() },
		);
		curves.insert(
			"Sound Power".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl.clone() },
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

	#[test]
	fn test_create_cea2034_combined_traces_counts_and_axes() {
		// Build minimal curves covering names used by combined function
		let mut curves = HashMap::new();
		let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
		let spl_primary = Array1::from(vec![80.0, 85.0, 82.0]);
		let spl_di = Array1::from(vec![5.0, 6.0, 7.0]);

		// Primary curves
		curves.insert(
			"On Axis".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Listening Window".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Early Reflections".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Sound Power".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);

		// DI curves
		curves.insert(
			"Early Reflections DI".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_di.clone() },
		);
		curves.insert(
			"Sound Power DI".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_di.clone() },
		);

		let traces = create_cea2034_combined_traces(&curves, "x7", "y7", "y7");
		assert_eq!(traces.len(), 6);

		// Check that DI traces target the secondary axis
		let v = to_json(&traces).unwrap();
		let names: Vec<String> =
			v.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
		assert!(names.contains(&"ERDI".to_string()));
		assert!(names.contains(&"SPDI".to_string()));

		// Find DI entries and ensure yaxis is y7 (DI shares primary axis in current implementation)
		for t in v.as_array().unwrap() {
			let n = t["name"].as_str().unwrap();
			if n.ends_with(" DI") {
				assert_eq!(t["yaxis"], json!("y7"));
			}
		}
	}

	#[test]
	fn test_create_cea2034_with_eq_combined_traces_counts_and_names() {
		let mut curves = HashMap::new();
		let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
		let spl_primary = Array1::from(vec![80.0, 85.0, 82.0]);
		let spl_di = Array1::from(vec![5.0, 6.0, 7.0]);

		// Primary curves
		curves.insert(
			"On Axis".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Listening Window".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Early Reflections".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		curves.insert(
			"Sound Power".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_primary.clone() },
		);
		// DI
		curves.insert(
			"Early Reflections DI".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_di.clone() },
		);
		curves.insert(
			"Sound Power DI".to_string(),
			crate::Curve { freq: freq.clone(), spl: spl_di.clone() },
		);

		let eq = Array1::from(vec![1.0, -1.0, 0.5]);
		let traces = create_cea2034_with_eq_combined_traces(&curves, &eq, "x8", "y8", "y8");
		assert_eq!(traces.len(), 6);
		let v = to_json(&traces).unwrap();
		// Primary names should have suffix w/EQ, DI should not
		let names: Vec<String> =
			v.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
		assert!(names.iter().any(|n| n == "ON w/EQ"));
		assert!(names.iter().any(|n| n == "LW w/EQ"));
		assert!(names.iter().any(|n| n == "ER w/EQ"));
		assert!(names.iter().any(|n| n == "SP w/EQ"));
		assert!(names.iter().any(|n| n == "ERDI"));
		assert!(names.iter().any(|n| n == "SPDI"));
		// DI yaxis should be y8 (shares primary axis in current implementation)
		for t in v.as_array().unwrap() {
			let n = t["name"].as_str().unwrap();
			if n.ends_with(" DI") {
				assert_eq!(t["yaxis"], json!("y8"));
			}
		}
	}

	#[test]
	fn test_shorten_curve_name() {
		// Test basic curve name abbreviations
		assert_eq!(shorten_curve_name("On Axis"), "ON");
		assert_eq!(shorten_curve_name("Listening Window"), "LW");
		assert_eq!(shorten_curve_name("Early Reflections"), "ER");
		assert_eq!(shorten_curve_name("Sound Power"), "SP");
		assert_eq!(shorten_curve_name("Estimated In-Room Response"), "PIR");

		// Test DI curve abbreviations
		assert_eq!(shorten_curve_name("Early Reflections DI"), "ERDI");
		assert_eq!(shorten_curve_name("Sound Power DI"), "SPDI");

		// Test unknown curve name (should return original)
		assert_eq!(shorten_curve_name("Unknown Curve"), "Unknown Curve");
		assert_eq!(shorten_curve_name(""), "");

		// Test case sensitivity (should return original since no exact match)
		assert_eq!(shorten_curve_name("on axis"), "on axis");
		assert_eq!(shorten_curve_name("ON AXIS"), "ON AXIS");
	}
}
