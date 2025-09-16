use ndarray::Array1;
use plotly::common::{AxisSide, Mode};
use plotly::layout::{AxisType, GridPattern, LayoutGrid, RowOrder};
use plotly::{Layout, Plot, Scatter};
use plotly_static::ImageFormat;
use std::collections::HashMap;

use crate::iir::{Biquad, BiquadFilterType};
use crate::plot::filter_color::filter_color;

pub fn plot_filters(
    args: &crate::cli::Args,
    input_curve: &crate::Curve,
    smoothed_curve: Option<&crate::Curve>,
    target_curve: &Array1<f64>,
    plot_freqs: &Array1<f64>,
    optimized_params: &[f64],
) -> plotly::Plot {
    let mut plot = Plot::new();

    // Compute combined response on the same frequency grid as input_curve for the new subplots
    let mut combined_response: Array1<f64> = Array1::zeros(input_curve.freq.len());
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

    // For the first subplot (individual filters), compute responses on plot_freqs
    let mut combined_response_plot: Array1<f64> = Array1::zeros(plot_freqs.len());
    for (display_idx, (orig_i, f0, q, gain)) in filters.iter().enumerate() {
        let ftype = if args.iir_hp_pk && *orig_i == 0 {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, *f0, args.sample_rate, *q, *gain);
        // Compute filter response on plot_freqs for the first subplot
        let filter_response = filter.np_log_result(plot_freqs);
        combined_response_plot += &filter_response;

        let label = if args.iir_hp_pk && *orig_i == 0 {
            "HPQ"
        } else {
            "PK"
        };
        let individual_trace = Scatter::new(plot_freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Lines)
            .name(format!("{} {} at {:5.0}Hz", label, orig_i + 1, f0))
            .marker(
                plotly::common::Marker::new()
                    .color(filter_color(display_idx))
                    .size(1),
            );
        plot.add_trace(individual_trace);
    }

    // Add total combined response on the first subplot (using plot_freqs)
    let total_trace = Scatter::new(plot_freqs.to_vec(), combined_response_plot.to_vec())
        .mode(Mode::Lines)
        .name("Sum")
        .line(plotly::common::Line::new().color("#000000").width(2.0));
    plot.add_trace(total_trace);

    let iir_trace = Scatter::new(plot_freqs.to_vec(), combined_response_plot.to_vec())
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
        // Interpolate target_curve to plot_freqs for the second subplot
        let target_interp = crate::read::interpolate(plot_freqs, &input_curve.freq, target_curve);
        let target_trace = Scatter::new(plot_freqs.to_vec(), target_interp.to_vec())
            .mode(Mode::Lines)
            .name("Target")
            .x_axis("x2")
            .y_axis("y2")
            .line(plotly::common::Line::new().color("#1f77b4"));
        plot.add_trace(target_trace);
    }

    // Recompute combined response on input_curve frequency grid for the new subplots
    for (orig_i, f0, q, gain) in filters {
        let ftype = if args.iir_hp_pk && orig_i == 0 {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, f0, args.sample_rate, q, gain);
        // Compute filter response on input_curve frequency grid
        let filter_response = filter.np_log_result(&input_curve.freq);
        combined_response += &filter_response;
    }

    // Add input curve and target curve subplot (new subplot)
    let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Input")
        .x_axis("x3")
        .y_axis("y3")
        .line(plotly::common::Line::new().color("#1f77b4"));
    plot.add_trace(input_trace);

    let target_trace_subplot = Scatter::new(input_curve.freq.to_vec(), target_curve.to_vec())
        .mode(Mode::Lines)
        .name("Target")
        .x_axis("x3")
        .y_axis("y3")
        .line(plotly::common::Line::new().color("#9467bd"));
    plot.add_trace(target_trace_subplot);

    // Add input curve + EQ and target curve subplot (new subplot)
    let input_plus_eq_trace = Scatter::new(
        input_curve.freq.to_vec(),
        (&input_curve.spl + &combined_response).to_vec(),
    )
    .mode(Mode::Lines)
    .name("Input + EQ")
    .x_axis("x4")
    .y_axis("y4")
    .line(plotly::common::Line::new().color("#2ca02c"));
    plot.add_trace(input_plus_eq_trace);

    let target_trace_subplot2 = Scatter::new(input_curve.freq.to_vec(), target_curve.to_vec())
        .mode(Mode::Lines)
        .name("Target")
        .x_axis("x4")
        .y_axis("y4")
        .line(plotly::common::Line::new().color("#9467bd"));
    plot.add_trace(target_trace_subplot2);

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
        .height(800)
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
        )
        .x_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        );
    plot.set_layout(layout);

    plot
}
