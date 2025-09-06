//! AutoEQ Benchmark CLI: runs optimization scenarios across cached speakers and writes CSV results
//!
//! Scenarios per speaker:
//! 1) --loss flat --measurement CEA2034 --curve-name "Listening Window"
//! 2) --loss flat --measurement "Estimated In-Room Response" --curve-name "Estimated In-Room Response"
//! 3) --loss score --measurement CEA2034
//!
//! Input data is expected under data/{speaker}/{measurement}.json (Plotly JSON),
//! optionally data/{speaker}/metadata.json for metadata preference score.

use autoeq::iir;
use autoeq::optim::ObjectiveData;
use autoeq::score;
use clap::Parser;
use ndarray::Array1;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinSet;

extern crate blas_src;

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    about = "Benchmark AutoEQ optimizations across cached speakers"
)]
pub struct BenchArgs {
    #[command(flatten)]
    pub base: autoeq::cli::Args,

    /// Limit to first 5 speakers for quick smoke run
    #[arg(long, default_value_t = false)]
    pub smoke_test: bool,

    /// Number of parallel jobs (0 = use all logical cores)
    #[arg(long, default_value_t = 0)]
    pub jobs: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = BenchArgs::parse();
    
    // Check if user wants to see algorithm list
    if args.base.algo_list {
        autoeq::cli::display_algorithm_list();
    }
    
    // Validate CLI arguments
    autoeq::cli::validate_args_or_exit(&args.base);

    // Enumerate speakers as subdirectories of ./data
    let speakers = list_speakers("data")?;
    let speakers: Vec<String> = if args.smoke_test {
        speakers.into_iter().take(5).collect()
    } else {
        speakers
    };
    if speakers.is_empty() {
        eprintln!("No speakers found under ./data. Exiting.");
        return Ok(());
    }

    // Determine parallelism
    let jobs = if args.jobs > 0 {
        args.jobs
    } else {
        num_cpus::get()
    };
    eprintln!("Running benchmark with {} parallel jobs", jobs);

    // Channel for rows; writer runs on main task
    let (tx, mut rx) =
        mpsc::channel::<(String, Option<f64>, Option<f64>, Option<f64>, Option<f64>)>(jobs * 2);
    let sem = std::sync::Arc::new(Semaphore::new(jobs));
    let mut set = JoinSet::new();

    for speaker in speakers.clone() {
        let tx = tx.clone();
        let sem = sem.clone();
        let base_args = args.base.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");

            // For local cache usage, version value is irrelevant provided cache exists.
            let version = "latest".to_string();

            // Scenario 1
            let mut a1 = base_args.clone();
            a1.speaker = Some(speaker.clone());
            a1.version = Some(version.clone());
            a1.measurement = Some("CEA2034".to_string());
            a1.curve_name = "Listening Window".to_string();
            a1.loss = autoeq::LossType::Flat;
            let s1 = run_one(&a1).await.ok().map(|m| m.pref_score);

            // Scenario 2
            let mut a2 = base_args.clone();
            a2.speaker = Some(speaker.clone());
            a2.version = Some(version.clone());
            a2.measurement = Some("Estimated In-Room Response".to_string());
            a2.curve_name = "Estimated In-Room Response".to_string();
            a2.loss = autoeq::LossType::Flat;
            let s2 = run_one(&a2).await.ok().map(|m| m.pref_score);

            // Scenario 3
            let mut a3 = base_args.clone();
            a3.speaker = Some(speaker.clone());
            a3.version = Some(version.clone());
            a3.measurement = Some("CEA2034".to_string());
            a3.loss = autoeq::LossType::Score;
            let s3 = run_one(&a3).await.ok().map(|m| m.pref_score);

            // Metadata preference
            let meta_pref = read_metadata_pref_score(&speaker).ok().flatten();

            let _ = tx.send((speaker, s1, s2, s3, meta_pref)).await;
        });
    }
    drop(tx); // close sender when tasks finish

    // CSV writer: header then rows as they arrive (unordered)
    let mut wtr = csv::Writer::from_path("data_generated/benchmark.csv")?;
    wtr.write_record([
        "speaker",
        "flat_cea2034_lw",
        "flat_eir",
        "score_cea2034",
        "metadata_pref",
    ])?;

    // Collect deltas (scenario - metadata) for end-of-run statistics
    let mut deltas_s1: Vec<f64> = Vec::new();
    let mut deltas_s2: Vec<f64> = Vec::new();
    let mut deltas_s3: Vec<f64> = Vec::new();

    while let Some((speaker, s1, s2, s3, meta_pref)) = rx.recv().await {
        wtr.write_record([
            speaker.as_str(),
            fmt_opt_f64(s1).as_str(),
            fmt_opt_f64(s2).as_str(),
            fmt_opt_f64(s3).as_str(),
            fmt_opt_f64(meta_pref).as_str(),
        ])?;

        // Accumulate deltas vs metadata when both values are present and finite
        if let (Some(v), Some(m)) = (s1, meta_pref) {
            if v.is_finite() && m.is_finite() {
                deltas_s1.push(v - m);
            }
        }
        if let (Some(v), Some(m)) = (s2, meta_pref) {
            if v.is_finite() && m.is_finite() {
                deltas_s2.push(v - m);
            }
        }
        if let (Some(v), Some(m)) = (s3, meta_pref) {
            if v.is_finite() && m.is_finite() {
                deltas_s3.push(v - m);
            }
        }
    }
    wtr.flush()?;

    // Ensure all tasks are done
    while let Some(_res) = set.join_next().await {
        // ignore task result; errors are reflected as empty row fields
    }

    // Print end-of-run statistics comparing scenarios to metadata
    eprintln!("\n=== Benchmark statistics (scenario - metadata) ===");
    print_stats("flat_cea2034_lw", &deltas_s1);
    print_stats("flat_eir", &deltas_s2);
    print_stats("score_cea2034", &deltas_s3);

    Ok(())
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{:.6}", x),
        _ => String::from(""),
    }
}

/// Compute mean and sample standard deviation of a slice.
/// Returns (mean, std). For n == 0 returns None. For n == 1, std = 0.0.
fn mean_std(data: &[f64]) -> Option<(f64, f64)> {
    let n = data.len();
    if n == 0 {
        return None;
    }
    let mean = data.iter().sum::<f64>() / (n as f64);
    if n == 1 {
        return Some((mean, 0.0));
    }
    let var_num: f64 = data
        .iter()
        .map(|&x| {
            let dx = x - mean;
            dx * dx
        })
        .sum();
    let std = (var_num / ((n - 1) as f64)).sqrt();
    Some((mean, std))
}

fn print_stats(name: &str, data: &[f64]) {
    match mean_std(data) {
        Some((m, s)) => {
            eprintln!(
                "{:>20}: N={:>4}, mean={:+.4}, std={:.4}",
                name,
                data.len(),
                m,
                s
            );
        }
        None => {
            eprintln!("{:>20}: N=   0", name);
        }
    }
}

fn list_speakers<P: AsRef<Path>>(data_dir: P) -> Result<Vec<String>, Box<dyn Error>> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(data_dir) {
        Ok(e) => e,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(out);
            } else {
                return Err(e.into());
            }
        }
    };
    for ent in entries {
        let ent = ent?;
        let p = ent.path();
        if p.is_dir() {
            if let Some(name) = p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
            {
                out.push(name);
            }
        }
    }
    out.sort();
    Ok(out)
}

async fn run_one(args: &autoeq::cli::Args) -> Result<score::ScoreMetrics, Box<dyn Error>> {
    let (input_curve, spin_data) = load_input_curve(args).await?;
    let (inverted_curve, smoothed_curve) = build_target_curve(args, &input_curve);
    let target_curve = smoothed_curve.as_ref().unwrap_or(&inverted_curve);
    let (objective_data, use_cea) =
        setup_objective_data(args, &input_curve, target_curve, &spin_data);

    let x = perform_optimization(args, &objective_data)?;

    if use_cea {
        let freq = &input_curve.freq;
        let peq_after = iir::compute_peq_response(freq, &x, args.sample_rate, args.iir_hp_pk);
        let metrics =
            score::compute_cea2034_metrics(freq, spin_data.as_ref().unwrap(), Some(&peq_after))
                .await?;
        Ok(metrics)
    } else {
        Err("CEA2034 data required to compute preference score".into())
    }
}

async fn load_input_curve(
    args: &autoeq::cli::Args,
) -> Result<(autoeq::Curve, Option<HashMap<String, autoeq::Curve>>), Box<dyn Error>> {
    autoeq::workflow::load_input_curve(args).await
}

fn build_target_curve(
    args: &autoeq::cli::Args,
    input_curve: &autoeq::Curve,
) -> (Array1<f64>, Option<Array1<f64>>) {
    autoeq::workflow::build_target_curve(args, input_curve)
}

fn setup_objective_data(
    args: &autoeq::cli::Args,
    input_curve: &autoeq::Curve,
    target_curve: &Array1<f64>,
    spin_data: &Option<HashMap<String, autoeq::Curve>>,
) -> (ObjectiveData, bool) {
    autoeq::workflow::setup_objective_data(args, input_curve, target_curve, spin_data)
}

fn perform_optimization(
    args: &autoeq::cli::Args,
    objective_data: &ObjectiveData,
) -> Result<Vec<f64>, Box<dyn Error>> {
    autoeq::workflow::perform_optimization(args, objective_data)
}

fn read_metadata_pref_score(speaker: &str) -> Result<Option<f64>, Box<dyn Error>> {
    let p = PathBuf::from("data").join(speaker).join("metadata.json");
    let content = match fs::read_to_string(&p) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            } else {
                return Err(e.into());
            }
        }
    };
    let v: Value = serde_json::from_str(&content)?;
    Ok(extract_pref_from_metadata_value(&v))
}

fn extract_pref_from_metadata_value(v: &Value) -> Option<f64> {
    // Path: measurements[default_measurement][pref_rating_eq].pref_score
    let default_measurement = v.get("default_measurement").and_then(|x| x.as_str())?;
    let measurements = v.get("measurements")?;
    let m = measurements.get(default_measurement)?;
    let pref = m.get("pref_rating_eq")?;
    pref.get("pref_score").and_then(|x| x.as_f64())
}

#[cfg(test)]
mod tests {
    use super::extract_pref_from_metadata_value;
    use serde_json::json;

    #[test]
    fn metadata_pref_path_extracts() {
        let v = json!({
            "default_measurement": "CEA2034",
            "measurements": {
                "CEA2034": {
                    "pref_rating_eq": {"pref_score": 6.789},
                    "pref_rating": {"pref_score": 5.0}
                }
            }
        });
        let got = extract_pref_from_metadata_value(&v);
        assert!(got.is_some());
        assert!((got.unwrap() - 6.789).abs() < 1e-12);
    }

    #[test]
    fn mean_std_basic() {
        let d = vec![1.0, 2.0, 3.0, 4.0];
        let (m, s) = super::mean_std(&d).unwrap();
        assert!((m - 2.5).abs() < 1e-12, "mean got {}", m);
        let expected_std = (5.0_f64 / 3.0).sqrt(); // sample std
        assert!((s - expected_std).abs() < 1e-12, "std got {}", s);
    }
}
