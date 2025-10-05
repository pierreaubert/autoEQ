use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use ndarray::Array1;
use reqwest;
use serde_json::Value;
use tokio::fs;
use urlencoding;

use crate::Curve;
use crate::cea2034 as score;
use crate::read::directory::{measurement_filename, sanitize_dir_name};
use crate::read::interpolate::interpolate;
use crate::read::plot::{normalize_plotly_json_from_str, normalize_plotly_value_with_suggestions};
use autoeq_env::DATA_CACHED;

/// Fetch a frequency response curve from the spinorama API
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Name of the specific curve to extract
///
/// # Returns
/// * Result containing a Curve struct or an error
pub async fn fetch_curve_from_api(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<crate::Curve, Box<dyn Error>> {
    // Fetch the full measurement once, then extract the requested curve
    let plot_data = fetch_measurement_plot_data(speaker, version, measurement).await?;
    extract_curve_by_name(&plot_data, measurement, curve_name)
}

/// Fetch and parse the full Plotly JSON object for a given measurement (single HTTP GET)
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `measurement` - Measurement type (e.g., "CEA2034")
///
/// # Returns
/// * Result containing the Plotly JSON data or an error
pub async fn fetch_measurement_plot_data(
    speaker: &str,
    version: &str,
    measurement: &str,
) -> Result<Value, Box<dyn Error>> {
    // 1) Try local cache first: data/{sanitized_speaker}/{measurement}.json
    // We keep filename identical to measurement name when possible (with path separators replaced).
    let cache_dir = PathBuf::from(DATA_CACHED).join(sanitize_dir_name(speaker));
    let cache_file = cache_dir.join(measurement_filename(measurement));

    if let Ok(content) = fs::read_to_string(&cache_file).await {
        if let Ok(plot_data) = normalize_plotly_json_from_str(&content) {
            return Ok(plot_data);
        } else {
            eprintln!(
                "⚠️  Cache file exists but could not be parsed as Plotly JSON: {:?}",
                &cache_file
            );
        }
    }

    // URL-encode the parameters
    let encoded_speaker = urlencoding::encode(speaker);
    let encoded_version = urlencoding::encode(version);
    let encoded_measurement = urlencoding::encode(measurement);

    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements/{}?measurement_format=json",
        encoded_speaker, encoded_version, encoded_measurement
    );

    // println!("* Fetching data from {}", url);

    let response = reqwest::get(&url).await?;
    if !response.status().is_success() {
        return Err(format!("API request failed with status: {}", response.status()).into());
    }
    let api_response: Value = response.json().await?;

    // Normalize from API response (array-of-string JSON) to Plotly JSON object
    let plot_data = normalize_plotly_value_with_suggestions(&api_response).await?;

    // 2) Save normalized Plotly JSON to cache for future use
    if let Err(e) = fs::create_dir_all(&cache_dir).await {
        eprintln!("⚠️  Failed to create cache dir {:?}: {}", &cache_dir, e);
    } else {
        match serde_json::to_string(&plot_data) {
            Ok(serialized) => {
                if let Err(e) = fs::write(&cache_file, serialized).await {
                    eprintln!("⚠️  Failed to write cache file {:?}: {}", &cache_file, e);
                }
            }
            Err(e) => eprintln!("⚠️  Failed to serialize plot data for cache: {}", e),
        }
    }

    Ok(plot_data)
}

/// Extract a single curve from a previously-fetched Plotly JSON object
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Name of the specific curve to extract
///
/// # Returns
/// * Result containing a Curve struct or an error
pub fn extract_curve_by_name(
    plot_data: &Value,
    measurement: &str,
    curve_name: &str,
) -> Result<crate::Curve, Box<dyn Error>> {
    // Extract frequency and SPL data from the Plotly JSON structure
    let mut freqs = Vec::new();
    let mut spls = Vec::new();

    // Look for the trace with the expected name and extract x and y data
    if let Some(data) = plot_data.get("data").and_then(|d| d.as_array()) {
        for trace in data {
            // Check if this is the SPL trace (not DI or other traces)
            let is_spl_trace = trace
                .get("name")
                .and_then(|n| n.as_str())
                .map(|name| is_target_trace_name(measurement, curve_name, name))
                .unwrap_or(false);

            if is_spl_trace {
                // Extract x and y data which are encoded as typed arrays
                if let (Some(x_data), Some(y_data)) = (
                    trace.get("x").and_then(|x| x.as_object()),
                    trace.get("y").and_then(|y| y.as_object()),
                ) {
                    // Decode x values (frequency)
                    if let (Some(dtype), Some(bdata)) = (
                        x_data.get("dtype").and_then(|d| d.as_str()),
                        x_data.get("bdata").and_then(|b| b.as_str()),
                    ) {
                        let decoded_x = decode_typed_array(bdata, dtype)?;
                        freqs = decoded_x;
                    }

                    // Decode y values (SPL)
                    if let (Some(dtype), Some(bdata)) = (
                        y_data.get("dtype").and_then(|d| d.as_str()),
                        y_data.get("bdata").and_then(|b| b.as_str()),
                    ) {
                        let decoded_y = decode_typed_array(bdata, dtype)?;
                        spls = decoded_y;
                    }

                    break;
                }
            }
        }
    }

    if freqs.is_empty() {
        return Err("Failed to extract frequency and SPL data from plot data".into());
    }

    Ok(crate::Curve {
        freq: Array1::from(freqs),
        spl: Array1::from(spls),
    })
}

fn decode_typed_array(bdata: &str, dtype: &str) -> Result<Vec<f64>, Box<dyn Error>> {
    // Create lookup table for base64 decoding
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0u8; 256];
    for (i, c) in chars.chars().enumerate() {
        lookup[c as usize] = i as u8;
    }

    // Calculate buffer length
    let len = bdata.len();
    let mut buffer_length = len * 3 / 4;

    // Adjust for padding
    if len > 0 && bdata.chars().nth(len - 1) == Some('=') {
        buffer_length -= 1;
        if len > 1 && bdata.chars().nth(len - 2) == Some('=') {
            buffer_length -= 1;
        }
    }

    // Decode base64
    let mut bytes = vec![0u8; buffer_length];
    let mut p = 0;
    let bdata_bytes = bdata.as_bytes();

    for i in (0..len).step_by(4) {
        let encoded1 = lookup[bdata_bytes[i] as usize] as u32;
        let encoded2 = if i + 1 < len {
            lookup[bdata_bytes[i + 1] as usize] as u32
        } else {
            0
        };
        let encoded3 = if i + 2 < len {
            lookup[bdata_bytes[i + 2] as usize] as u32
        } else {
            0
        };
        let encoded4 = if i + 3 < len {
            lookup[bdata_bytes[i + 3] as usize] as u32
        } else {
            0
        };

        if p < buffer_length {
            bytes[p] = ((encoded1 << 2) | (encoded2 >> 4)) as u8;
            p += 1;
        }

        if p < buffer_length {
            bytes[p] = (((encoded2 & 15) << 4) | (encoded3 >> 2)) as u8;
            p += 1;
        }

        if p < buffer_length {
            bytes[p] = (((encoded3 & 3) << 6) | (encoded4 & 63)) as u8;
            p += 1;
        }
    }

    // Convert to appropriate typed array based on dtype
    let result = match dtype {
        "f8" => {
            // Float64Array - 8 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let bits = u64::from_le_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
                ]);
                values.push(f64::from_bits(bits));
            }
            values
        }
        "f4" => {
            // Float32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(f32::from_bits(bits) as f64);
            }
            values
        }
        "i4" => {
            // Int32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let val = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(val as f64);
            }
            values
        }
        "i2" => {
            // Int16Array - 2 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let val = i16::from_le_bytes([chunk[0], chunk[1]]);
                values.push(val as f64);
            }
            values
        }
        "i1" => {
            // Int8Array - 1 byte per element
            bytes.into_iter().map(|b| b as i8 as f64).collect()
        }
        "u4" => {
            // Uint32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let val = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(val as f64);
            }
            values
        }
        "u2" => {
            // Uint16Array - 2 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                values.push(val as f64);
            }
            values
        }
        "u1" | "u1c" => {
            // Uint8Array or Uint8ClampedArray - 1 byte per element
            bytes.into_iter().map(|b| b as f64).collect()
        }
        _ => {
            // Default to treating as bytes
            bytes.into_iter().map(|b| b as f64).collect()
        }
    };

    Ok(result)
}

fn is_target_trace_name(measurement: &str, curve_name: &str, candidate: &str) -> bool {
    if measurement.eq_ignore_ascii_case("CEA2034") || measurement.eq("Estimated In-Room Response") {
        // For CEA2034 data, select the specific curve provided by the user
        // Prefer exact match; allow substring match as a fallback
        candidate == curve_name
    } else {
        // Fallback heuristic for other measurement types
        eprintln!(
            "⚠️  Warning: unable to determine if trace name {} is a target for curve {}, using heuristic",
            candidate, curve_name
        );
        candidate.contains("Listening Window")
            || candidate.contains("On Axis")
            || candidate.contains("Sound Power")
            || candidate.contains("Early Reflections")
            || candidate.contains("Estimated In-Room Response")
    }
}

fn collect_trace_names(plot_data: &Value) -> Vec<String> {
    plot_data
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|trace| {
                    trace
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

/// Extract all CEA2034 curves from plot data and interpolate to target frequency grid
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data containing CEA2034 measurements
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `freq` - Target frequency grid for interpolation
///
/// # Returns
/// * HashMap of curve names to interpolated Curve structs
///
/// # Details
/// Extracts standard CEA2034 curves (On Axis, Listening Window, Early Reflections,
/// Sound Power, etc.) and interpolates them to the specified frequency grid.
pub fn extract_cea2034_curves(
    plot_data: &Value,
    measurement: &str,
    freq: &Array1<f64>,
) -> Result<HashMap<String, Curve>, Box<dyn Error>> {
    let mut curves = HashMap::new();

    // List of CEA2034 curves to extract
    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
        "Early Reflections DI",
        "Sound Power DI",
    ];

    // Extract each curve
    for name in &curve_names {
        match extract_curve_by_name(plot_data, measurement, name) {
            Ok(curve) => {
                // Interpolate to the target frequency grid
                let interpolated = interpolate(freq, &curve);
                curves.insert(
                    name.to_string(),
                    Curve {
                        freq: freq.clone(),
                        spl: interpolated.spl,
                    },
                );
            }
            Err(e) => {
                let available = collect_trace_names(plot_data);
                return Err(format!(
                    "Could not extract curve '{}' for measurement '{}': {}. Available traces: {:?}",
                    name, measurement, e, available
                )
                .into());
            }
        }
    }

    // Ensure required curves exist for PIR computation
    let lw_curve = curves.get("Listening Window").ok_or_else(|| {
        std::io::Error::other("Missing 'Listening Window' curve after extraction")
    })?;
    let er_curve = curves.get("Early Reflections").ok_or_else(|| {
        std::io::Error::other("Missing 'Early Reflections' curve after extraction")
    })?;
    let sp_curve = curves
        .get("Sound Power")
        .ok_or_else(|| std::io::Error::other("Missing 'Sound Power' curve after extraction"))?;

    let lw = &lw_curve.spl;
    let er = &er_curve.spl;
    let sp = &sp_curve.spl;
    let pir = score::compute_pir_from_lw_er_sp(lw, er, sp);
    curves.insert(
        "Estimated In-Room Response".to_string(),
        Curve {
            freq: freq.clone(),
            spl: pir,
        },
    );

    Ok(curves)
}
