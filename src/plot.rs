use std::error::Error;
use std::path::PathBuf;

use eqopt::{Biquad, BiquadFilterType};
use ndarray::Array1;
use plotly::common::{Mode, Title};
use plotly::layout::{AxisType, Layout};
use plotly::{Plot, Scatter};

fn filter_color(index: usize) -> &'static str {
    // Plotly Category10 palette
    const COLORS: [&str; 10] = [
        "#1f77b4", // blue
        "#ff7f0e", // orange
        "#2ca02c", // green
        "#d62728", // red
        "#9467bd", // purple
        "#8c564b", // brown
        "#e377c2", // pink
        "#7f7f7f", // gray
        "#bcbd22", // yellow-green
        "#17becf", // cyan
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

pub async fn plot_results(
    input_curve: &super::Curve,
    optimized_params: &[f64],
    num_filters: usize,
    sample_rate: f64,
    max_db: f64,
    smoothed: Option<&Array1<f64>>,
    output_path: &PathBuf,
    speaker: Option<&str>,
    measurement: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    // Create a dense frequency vector for smooth plotting
    let mut freqs = Vec::new();
    let mut freq = 20.0;
    while freq <= 20000.0 {
        freqs.push(freq);
        freq *= 1.05; // Logarithmic spacing
    }
    let plot_freqs = Array1::from(freqs);

    // Calculate combined IIR response
    let mut combined_response: Array1<f64> = Array1::zeros(plot_freqs.len());

    // Create a single plot with subplots
    let mut plot = Plot::new();

    // Determine lowest-frequency filter index -> Highpass; others Peak
    let mut hp_index = 0usize;
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

    // First subplot: Individual filters (y axis)
    for i in 0..num_filters {
        let f0 = optimized_params[i * 3];
        let q = optimized_params[i * 3 + 1];
        let gain = optimized_params[i * 3 + 2];

        let ftype = if i == hp_index {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, f0, sample_rate, q, gain);
        let filter_response = filter.np_log_result(&plot_freqs);
        combined_response = combined_response + &filter_response;

        let label = if i == hp_index { "Highpass" } else { "Peak" };
        let individual_trace = Scatter::new(plot_freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Markers)
            .name(&format!("Filter {} ({})", i + 1, label))
            .y_axis("y")
            .marker(plotly::common::Marker::new().color(filter_color(i)).size(4));
        plot.add_trace(individual_trace);
    }

    // Add total combined response on the first subplot
    let total_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("Total Filter Response")
        .y_axis("y")
        .line(plotly::common::Line::new().color("#000000").width(2.0));
    plot.add_trace(total_trace);

    // Add vertical markers for center frequencies on first subplot for spacing visualization
    for i in 0..num_filters {
        let f0 = optimized_params[i * 3];
        let (xs, ys) = vline_points(f0, -5.0, 5.0);
        let vline = Scatter::new(xs, ys)
            .mode(Mode::Lines)
            .name(&format!("f0 = {:.0} Hz", f0))
            .y_axis("y")
            .opacity(0.35)
            .line(plotly::common::Line::new().color("#7f7f7f").width(1.0));
        plot.add_trace(vline);
    }

    // Second subplot: Input curve and IIR response (not inverted) (y2 axis)

    let measurement_name = measurement.unwrap_or("Input Curve");

    let iir_trace = Scatter::new(plot_freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("IIR Response")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("#2ca02c"));
    plot.add_trace(iir_trace);

    // If smoothing enabled, add the smoothed inverted target curve trace
    if let Some(sm) = smoothed {
        let smoothed_trace = Scatter::new(input_curve.freq.to_vec(), sm.to_vec())
            .mode(Mode::Lines)
            .name("Smoothed Inverted Target")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#9467bd"));
        plot.add_trace(smoothed_trace);
    } else {
	let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
            .mode(Mode::Lines)
            .name(measurement_name)
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#1f77b4"));
	plot.add_trace(input_trace);
    }

    // Title with optional speaker name
    let title_text = match speaker {
        Some(s) if !s.is_empty() => format!("IIR Filter Optimization Results - {}", s),
        _ => "IIR Filter Optimization Results".to_string(),
    };

    // Configure layout with subplots
    let layout = Layout::new()
        .title(Title::with_text(&title_text))
        .width(800)
        .height(800)
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301]),
        ) // log10(20) to log10(20000)
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Amplitude (dB)"))
                .range(vec![-5.0, 5.0]) // limit filter subplot range per request
                .domain(&[0.5, 1.0]),
        )
        .y_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Amplitude (dB)"))
                .range(vec![-15.0, 15.0])
                .domain(&[0.0, 0.5]),
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
    use super::{apply_db_clamp, filter_color, vline_points};
    use ndarray::Array1;

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
}
