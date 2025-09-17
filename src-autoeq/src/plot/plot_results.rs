use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use build_html::*;
use plotly::Plot;
use plotly_static::{ImageFormat, StaticExporterBuilder};

use crate::plot::plot_filters::plot_filters;
use crate::plot::plot_spin::{plot_spin, plot_spin_details};

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
    args: &crate::cli::Args,
    optimized_params: &[f64],
    objective_data: &crate::optim::ObjectiveData,
    input_curve: &crate::Curve,
    target_curve: &crate::Curve,
    deviation_curve: &crate::Curve,
    cea2034_curves: &Option<HashMap<String, crate::Curve>>,
    output_path: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let speaker = args.speaker.as_deref();

    let freqs = input_curve.freq.clone();

    // gather all subplots
    let plot_filters = plot_filters(
        args,
        input_curve,
        target_curve,
        deviation_curve,
        optimized_params,
    );
    let plot_spin_details = if cea2034_curves.is_some() {
        Some(plot_spin_details(
            args,
            input_curve,
            &freqs,
            cea2034_curves.as_ref(),
            Some(&objective_data.target_error),
        ))
    } else {
        None
    };
    let plot_spin_opt = if cea2034_curves.is_some() {
        Some(plot_spin(
            cea2034_curves.as_ref(),
            Some(&objective_data.target_error),
        ))
    } else {
        None
    };

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
            .unwrap_or_else(|_| panic!("Failed to create output directory: {:?}", parent));
    }

    let mut file = File::create(&html_output_path).unwrap();
    file.write_all(html.as_bytes())
        .expect("failed to write html output");
    file.flush().unwrap();

    // plot_spin.write_html(output_path.with_extension("html"));

    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let mut plots: Vec<(Plot, &str, usize, usize)> =
        vec![(plot_filters, "filters", 1280usize, 800usize)];
    if let Some(plot_spin) = plot_spin_details {
        plots.push((plot_spin, "details", 1280, 650));
    }

    if let Some(plot_spin) = plot_spin_opt {
        plots.push((plot_spin, "spins", 1280, 450));
    }

    // Try to create an async static exporter. If unavailable, skip PNG export and continue.
    let exporter_build = StaticExporterBuilder::default()
        .webdriver_port(5112)
        .build_async();

    match exporter_build {
        Ok(mut exporter) => {
            for (plot, name, width, height) in plots {
                let img_path = output_path.with_file_name(format!("{}-{}.png", stem, name));

                // Ensure parent directory exists for PNG files
                if let Some(parent) = img_path.parent() {
                    std::fs::create_dir_all(parent).unwrap_or_else(|_| {
                        panic!("Failed to create PNG output directory: {:?}", parent)
                    });
                }

                if let Err(e) = exporter
                    .write_fig(
                        img_path.as_path(),
                        &serde_json::to_value(&plot).expect("Failed to serialize plot to JSON"),
                        ImageFormat::PNG,
                        width.try_into().unwrap(),
                        height.try_into().unwrap(),
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
