use ndarray::Array1;
use plotly::common::{Anchor, Mode};
use plotly::layout::{Annotation, AxisType, GridPattern, LayoutGrid, RowOrder};
use plotly::{Layout, Plot, Scatter};

use crate::iir::{Biquad, BiquadFilterType};
use crate::plot::filter_color::filter_color;
use crate::plot::ref_lines::make_ref_lines;

pub fn plot_filters(
    args: &crate::cli::Args,
    input_curve: &crate::Curve,
    target_curve: &crate::Curve,
    deviation_curve: &crate::Curve,
    optimized_params: &[f64],
) -> plotly::Plot {
    let freqs = input_curve.freq.clone();
    let mut plot = Plot::new();

    // Compute combined response on the same frequency grid as input_curve for the new subplots
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
    let mut combined_response: Array1<f64> = Array1::zeros(freqs.len());
    for (display_idx, (orig_i, f0, q, gain)) in filters.iter().enumerate() {
        let ftype = if args.iir_hp_pk && *orig_i == 0 {
            BiquadFilterType::Highpass
        } else {
            BiquadFilterType::Peak
        };
        let filter = Biquad::new(ftype, *f0, args.sample_rate, *q, *gain);
        // Compute filter response on plot_freqs for the first subplot
        let filter_response = filter.np_log_result(&freqs);
        combined_response += &filter_response;

        let label = if args.iir_hp_pk && *orig_i == 0 {
            "HPQ"
        } else {
            "PK"
        };
        let individual_trace = Scatter::new(freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Lines)
            .name(format!("{} {} at {:5.0}Hz", label, orig_i + 1, f0))
            .marker(
                plotly::common::Marker::new()
                    .color(filter_color(display_idx))
                    .size(1),
            );
        plot.add_trace(individual_trace);
    }

    // Add total combined response on the first and second subplots
    let iir_trace1 = Scatter::new(freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("autoEQ")
        .line(plotly::common::Line::new().color("#000000").width(2.0));
    plot.add_trace(iir_trace1);

    let iir_trace2 = Scatter::new(freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("autoEQ")
        .show_legend(false)
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("000000").width(2.0));
    plot.add_trace(iir_trace2);

    // Interpolate deviation_curve to plot_freqs for the second subplot
    let target_trace2 = Scatter::new(freqs.to_vec(), deviation_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Deviation")
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("#1f77b4"));
    plot.add_trace(target_trace2);

    let target_trace3 = Scatter::new(freqs.to_vec(), target_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Target")
        .show_legend(false)
        .x_axis("x3")
        .y_axis("y3")
        .line(plotly::common::Line::new().color("#1f77b4"));
    plot.add_trace(target_trace3);

    let error = &deviation_curve.spl - &combined_response;
    let target_trace4 = Scatter::new(freqs.to_vec(), error.to_vec())
        .mode(Mode::Lines)
        .name("Error")
        .x_axis("x3")
        .y_axis("y3")
        .line(plotly::common::Line::new().color(filter_color(6)));
    plot.add_trace(target_trace4);

    // Add input curve and target curve subplot (new subplot)
    let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Input")
        .x_axis("x4")
        .y_axis("y4")
        .line(plotly::common::Line::new().color(filter_color(4)));
    plot.add_trace(input_trace);

    // Add input curve + EQ and target curve subplot (new subplot)
    let input_plus_eq_trace = Scatter::new(
        input_curve.freq.to_vec(),
        (&input_curve.spl + &combined_response).to_vec(),
    )
    .mode(Mode::Lines)
    .name("Input + EQ")
    .x_axis("x4")
    .y_axis("y4")
    .line(plotly::common::Line::new().color(filter_color(5)));
    plot.add_trace(input_plus_eq_trace);

    // Add reference lines
    let ref_lines3 = make_ref_lines("x3", "y3");
    for ref_line in ref_lines3 {
        plot.add_trace(Box::new(ref_line));
    }

    // Configure layout with subplots
    let mut layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(2)
                .columns(2)
                .pattern(GridPattern::Independent)
                .row_order(RowOrder::TopToBottom),
        )
        .width(1024)
        .height(800)
        .x_axis(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        ) // log10(20) to log10(20000)
        .y_axis(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        )
        .x_axis2(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        ) // log10(20) to log10(20000)
        .y_axis2(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        )
        .x_axis3(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-5.0, 5.0]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("IIR filters and Sum of filters")
            .x_ref("x domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y2 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Autoeq v.s. Deviation from target")
            .x_ref("x2 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y3 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Error = Autoeq-Deviation (zoomed)")
            .x_ref("x3 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y4 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Response w/ autoEQ")
            .x_ref("x4 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    plot.set_layout(layout);

    plot
}
