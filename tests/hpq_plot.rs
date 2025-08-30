use autoeq::{Biquad, BiquadFilterType};
use ndarray::Array1;
use plotly::common::{Mode, Title};
use plotly::{Layout, Plot, Scatter};

fn logspace_20_to_20k_n(n: usize) -> Array1<f64> {
    assert!(n >= 2);
    let f_start = 20.0_f64;
    let f_end = 20_000.0_f64;
    let ratio = (f_end / f_start).powf(1.0 / (n as f64 - 1.0));
    let mut v = Vec::with_capacity(n);
    let mut f = f_start;
    for _ in 0..n {
        v.push(f);
        f *= ratio;
    }
    Array1::from(v)
}

#[test]
fn plot_highpass_variable_q_responses() {
    // Frequency grid: 20 Hz .. 20 kHz, 200 log-spaced points
    let freqs = logspace_20_to_20k_n(200);

    // High-pass cutoff and sample rate
    let fc = 100.0_f64; // Hz
    let srate = 48_000.0_f64;

    // Q values to sweep
    let qs = [0.1_f64, 0.2, 0.3, 0.5, 1.0, 2.0, 5.0, 6.0, 10.0];

    // Build plot with one trace per Q
    let mut plot = Plot::new();

    for &q in &qs {
        let hp = Biquad::new(
            BiquadFilterType::HighpassVariableQ,
            fc,
            srate,
            q,
            0.0, // gain not used for HP
        );
        let resp_db = hp.np_log_result(&freqs);
        // Basic sanity: all finite
        assert!(resp_db.iter().all(|v| v.is_finite()));

        let trace = Scatter::new(freqs.to_vec(), resp_db.to_vec())
            .mode(Mode::Lines)
            .name(&format!("HP @ {:.0} Hz, Q={}", fc, q));
        plot.add_trace(trace);
    }

    // Configure axes (log-x) and title
    let layout = Layout::new()
        .title(Title::with_text("Highpass Variable-Q Responses"))
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(plotly::layout::AxisType::Log)
                .range(vec![1.301, 4.301]), // log10(20)..log10(20000)
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Magnitude (dB)")),
        );
    plot.set_layout(layout);

    // Write HTML next to project root so it's easy to open
    let out = std::path::PathBuf::from("plot_hpq.html");
    plot.write_html(&out);

    // Ensure file was produced
    assert!(out.exists(), "expected plot {:?} to be created", out);
}
