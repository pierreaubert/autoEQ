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

use csv::ReaderBuilder;
use ndarray::Array1;
use reqwest;
use serde::Deserialize;
use serde_json::Value;
use tokio::fs;
use urlencoding;

use crate::Curve;
use crate::score;

#[derive(Debug, Deserialize)]
struct CsvRecord {
    frequency: f64,
    spl: f64,
}

const DATA_CACHED: &str = "data_cached";

/// Return the cache directory for a given speaker under `data_cached/` using sanitized name
pub fn data_dir_for(speaker: &str) -> PathBuf {
    let mut p = PathBuf::from(DATA_CACHED);
    p.push(sanitize_dir_name(speaker));
    p
}

// --- Helpers for caching and JSON normalization ---

/// Sanitize a single path component by replacing non-alphanumeric characters
/// (except space, dash and underscore) with underscores. This is used to map
/// user-provided speaker names to safe directory names inside `data/`.
pub fn sanitize_dir_name(name: &str) -> String {
    // Keep alnum, space, dash, underscore; replace others with underscore.
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    // Trim leading/trailing spaces and underscores
    out.trim().trim_matches('_').to_string()
}

/// Return the cache filename for a measurement, neutralizing any path
/// separators. For example, "Estimated In-Room Response" becomes
/// "Estimated In-Room Response.json" and "A/B" becomes "A-B.json".
pub fn measurement_filename(measurement: &str) -> String {
    // Only neutralize path separators; keep the name otherwise to match saved files.
    let safe = measurement.replace(['/', '\\'], "-");
    format!("{}.json", safe)
}

fn normalize_plotly_value(v: &Value) -> Result<Value, Box<dyn Error>> {
    // API format is ["{...plotly json...}"]
    if let Some(arr) = v.as_array() {
        if let Some(first) = arr.first() {
            if let Some(s) = first.as_str() {
                let parsed: Value = serde_json::from_str(s)?;
                return Ok(parsed);
            } else {
                return Err("First element is not a string".into());
            }
        } else {
            return Err("Empty API response".into());
        }
    }
    Err("API response is not an array".into())
}

fn normalize_plotly_json_from_str(content: &str) -> Result<Value, Box<dyn Error>> {
    // Content could be one of:
    // - Already a Plotly JSON object with "data" key
    // - A JSON array with one string (API response)
    // - A bare JSON string containing the Plotly JSON
    let v: Value = serde_json::from_str(content)?;
    if v.is_object() {
        return Ok(v);
    }
    if let Ok(parsed) = normalize_plotly_value(&v) {
        return Ok(parsed);
    }
    if let Some(s) = v.as_str() {
        let inner: Value = serde_json::from_str(s)?;
        return Ok(inner);
    }
    Err("Unrecognized cached JSON format".into())
}

/// Read a frequency response curve from a CSV file
///
/// # Arguments
/// * `path` - Path to the CSV file
///
/// # Returns
/// * Result containing a Curve struct or an error
///
/// # CSV Format
/// The CSV file should have a header row with "frequency" and "spl" columns,
/// followed by rows of frequency (Hz) and SPL (dB) values.
pub fn read_curve_from_csv(path: &PathBuf) -> Result<Curve, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().has_headers(true).from_path(path)?;
    let mut freqs = Vec::new();
    let mut spls = Vec::new();

    for result in rdr.deserialize() {
        let record: CsvRecord = result?;
        freqs.push(record.frequency);
        spls.push(record.spl);
    }

    Ok(super::Curve {
        freq: Array1::from(freqs),
        spl: Array1::from(spls),
    })
}

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
) -> Result<super::Curve, Box<dyn Error>> {
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
    let plot_data = normalize_plotly_value(&api_response)?;

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
) -> Result<super::Curve, Box<dyn Error>> {
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
                if let (Some(x_data), Some(y_data)) = (trace.get("x"), trace.get("y")) {
                    // Decode x values (frequency)
                    if let Some(x_obj) = x_data.as_object() {
                        if let (Some(dtype), Some(bdata)) = (x_obj.get("dtype"), x_obj.get("bdata"))
                        {
                            if let (Some(dtype_str), Some(bdata_str)) =
                                (dtype.as_str(), bdata.as_str())
                            {
                                let decoded_x = decode_typed_array(bdata_str, dtype_str)?;
                                freqs = decoded_x;
                            }
                        }
                    }

                    // Decode y values (SPL)
                    if let Some(y_obj) = y_data.as_object() {
                        if let (Some(dtype), Some(bdata)) = (y_obj.get("dtype"), y_obj.get("bdata"))
                        {
                            if let (Some(dtype_str), Some(bdata_str)) =
                                (dtype.as_str(), bdata.as_str())
                            {
                                let decoded_y = decode_typed_array(bdata_str, dtype_str)?;
                                spls = decoded_y;
                            }
                        }
                    }

                    break;
                }
            }
        }
    }

    if freqs.is_empty() {
        return Err("Failed to extract frequency and SPL data from plot data".into());
    }

    Ok(super::Curve {
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
                let interpolated = interpolate(freq, &curve.freq, &curve.spl);
                curves.insert(
                    name.to_string(),
                    Curve {
                        freq: freq.clone(),
                        spl: interpolated,
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
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "Missing 'Listening Window' curve after extraction",
        )
    })?;
    let er_curve = curves.get("Early Reflections").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "Missing 'Early Reflections' curve after extraction",
        )
    })?;
    let sp_curve = curves.get("Sound Power").ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "Missing 'Sound Power' curve after extraction",
        )
    })?;

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

#[cfg(test)]
mod tests {
    use super::{
        collect_trace_names, extract_cea2034_curves, extract_curve_by_name, is_target_trace_name,
        normalize_plotly_json_from_str,
    };
    use ndarray::Array1;
    use serde_json::json;

    fn le_f64_bytes(vals: &[f64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(vals.len() * 8);
        for v in vals {
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
        out
    }

    fn base64_encode(bytes: &[u8]) -> String {
        // Same alphabet as decoder
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        let mut i = 0usize;
        while i < bytes.len() {
            let b0 = bytes[i] as u32;
            let b1 = if i + 1 < bytes.len() {
                bytes[i + 1] as u32
            } else {
                0
            };
            let b2 = if i + 2 < bytes.len() {
                bytes[i + 2] as u32
            } else {
                0
            };

            let idx0 = (b0 >> 2) & 0x3F;
            let idx1 = ((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0F);
            let idx2 = ((b1 & 0x0F) << 2) | ((b2 >> 6) & 0x03);
            let idx3 = b2 & 0x3F;

            out.push(alphabet[idx0 as usize] as char);
            out.push(alphabet[idx1 as usize] as char);
            if i + 1 < bytes.len() {
                out.push(alphabet[idx2 as usize] as char);
            } else {
                out.push('=');
            }
            if i + 2 < bytes.len() {
                out.push(alphabet[idx3 as usize] as char);
            } else {
                out.push('=');
            }

            i += 3;
        }
        out
    }

    #[test]
    fn collects_trace_names_from_data_array() {
        let plot_data = json!({
            "data": [
                {"name": "Listening Window", "x": [1,2], "y": [3,4]},
                {"name": "CEA2034"},
                {"x": [0]} // no name
            ]
        });
        let names = collect_trace_names(&plot_data);
        assert_eq!(names, vec!["Listening Window", "CEA2034"]);
    }

    #[test]
    fn handles_missing_or_wrong_shape() {
        let empty = json!({});
        assert!(collect_trace_names(&empty).is_empty());

        let wrong = json!({"data": {"name": "not-an-array"}});
        assert!(collect_trace_names(&wrong).is_empty());
    }

    #[test]
    fn fallback_for_non_cea_measurements() {
        assert!(is_target_trace_name("Other", "ignored", "On Axis"));
        assert!(!is_target_trace_name("Other", "ignored", "SPL something"));
        assert!(!is_target_trace_name("Other", "ignored", "PIR"));
    }

    #[test]
    fn extract_curve_by_name_decodes_typed_arrays() {
        // Prepare typed arrays for two points
        let xf = [100.0_f64, 1000.0_f64];
        let yf = [80.0_f64, 85.0_f64];
        let x_b64 = base64_encode(&le_f64_bytes(&xf));
        let y_b64 = base64_encode(&le_f64_bytes(&yf));
        let plot_data = json!({
            "data": [
                {
                    "name": "Listening Window",
                    "x": {"dtype": "f8", "bdata": x_b64},
                    "y": {"dtype": "f8", "bdata": y_b64}
                },
                {
                    "name": "On Axis",
                    "x": {"dtype": "f8", "bdata": x_b64},
                    "y": {"dtype": "f8", "bdata": y_b64}
                }
            ]
        });

        let curve = extract_curve_by_name(&plot_data, "CEA2034", "Listening Window").unwrap();
        assert_eq!(curve.freq.len(), 2);
        assert_eq!(curve.spl.len(), 2);
        assert!((curve.freq[0] - 100.0).abs() < 1e-12);
        assert!((curve.freq[1] - 1000.0).abs() < 1e-12);
        assert!((curve.spl[0] - 80.0).abs() < 1e-12);
        assert!((curve.spl[1] - 85.0).abs() < 1e-12);
    }

    #[test]
    fn extract_curve_by_name_requires_exact_match_for_cea2034() {
        let xf = [200.0_f64, 400.0_f64];
        let yf = [70.0_f64, 72.0_f64];
        let x_b64 = base64_encode(&le_f64_bytes(&xf));
        let y_b64 = base64_encode(&le_f64_bytes(&yf));
        let plot_data = json!({
            "data": [
                {
                    "name": "Listening Window",
                    "x": {"dtype": "f8", "bdata": x_b64},
                    "y": {"dtype": "f8", "bdata": y_b64}
                }
            ]
        });

        let curve = extract_curve_by_name(&plot_data, "CEA2034", "Listening Window").unwrap();
        assert_eq!(curve.freq.len(), 2);
        assert!((curve.freq[0] - 200.0).abs() < 1e-12);
    }

    #[test]
    fn extract_cea2034_curves_errors_when_curve_missing() {
        // Only provide Listening Window trace; others are missing -> should error on first missing (On Axis)
        let xf = [100.0_f64, 1000.0_f64];
        let yf = [80.0_f64, 85.0_f64];
        let x_b64 = base64_encode(&le_f64_bytes(&xf));
        let y_b64 = base64_encode(&le_f64_bytes(&yf));

        let plot_data = json!({
            "data": [
                {
                    "name": "Listening Window",
                    "x": {"dtype": "f8", "bdata": x_b64},
                    "y": {"dtype": "f8", "bdata": y_b64}
                }
            ]
        });

        let target_freq = Array1::from(vec![100.0, 500.0, 1000.0]);
        let res = extract_cea2034_curves(&plot_data, "CEA2034", &target_freq);
        assert!(res.is_err());
        let err = format!("{}", res.unwrap_err());
        assert!(err.contains("Could not extract curve") || err.contains("Failed to extract"));
    }

    #[test]
    fn extract_cea2034_curves_success_and_contains_pir() {
        // Provide all required curves so extraction succeeds and PIR is computed
        let xf = [100.0_f64, 1000.0_f64, 2000.0_f64];
        let on = [80.0_f64, 82.0_f64, 84.0_f64];
        let lw = [79.0_f64, 81.0_f64, 83.0_f64];
        let er = [78.0_f64, 80.0_f64, 82.0_f64];
        let sp = [77.0_f64, 79.0_f64, 81.0_f64];
        let er_di = [1.0_f64, 1.0_f64, 1.0_f64];
        let sp_di = [2.0_f64, 2.0_f64, 2.0_f64];

        let x_b64 = base64_encode(&le_f64_bytes(&xf));
        let on_b64 = base64_encode(&le_f64_bytes(&on));
        let lw_b64 = base64_encode(&le_f64_bytes(&lw));
        let er_b64 = base64_encode(&le_f64_bytes(&er));
        let sp_b64 = base64_encode(&le_f64_bytes(&sp));
        let er_di_b64 = base64_encode(&le_f64_bytes(&er_di));
        let sp_di_b64 = base64_encode(&le_f64_bytes(&sp_di));

        let plot_data = json!({
            "data": [
                {"name": "On Axis",              "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": on_b64}},
                {"name": "Listening Window",      "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": lw_b64}},
                {"name": "Early Reflections",     "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": er_b64}},
                {"name": "Sound Power",           "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": sp_b64}},
                {"name": "Early Reflections DI",  "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": er_di_b64}},
                {"name": "Sound Power DI",        "x": {"dtype": "f8", "bdata": x_b64}, "y": {"dtype": "f8", "bdata": sp_di_b64}}
            ]
        });

        let target_freq = Array1::from(vec![100.0, 250.0, 1000.0, 1500.0, 2000.0]);
        let curves = extract_cea2034_curves(&plot_data, "CEA2034", &target_freq).unwrap();

        // Required keys + PIR
        for key in [
            "On Axis",
            "Listening Window",
            "Early Reflections",
            "Sound Power",
            "Early Reflections DI",
            "Sound Power DI",
            "Estimated In-Room Response",
        ] {
            assert!(curves.contains_key(key), "missing key: {}", key);
            assert_eq!(curves[key].freq.len(), target_freq.len());
            assert_eq!(curves[key].spl.len(), target_freq.len());
        }
    }

    #[test]
    fn normalize_plotly_handles_object_array_and_string() {
        // Case 1: already a Plotly object
        let obj = json!({"data": [{"name": "On Axis"}]});
        let s1 = serde_json::to_string(&obj).unwrap();
        let p1 = normalize_plotly_json_from_str(&s1).unwrap();
        assert!(p1.get("data").is_some());

        // Case 2: API array-of-string format
        let inner = json!({"data": [{"name": "Listening Window"}]});
        let s_inner = serde_json::to_string(&inner).unwrap();
        let api = json!([s_inner]);
        let s2 = serde_json::to_string(&api).unwrap();
        let p2 = normalize_plotly_json_from_str(&s2).unwrap();
        assert!(p2.get("data").is_some());

        // Case 3: bare JSON string containing the Plotly JSON
        let s3 = serde_json::to_string(&s_inner).unwrap();
        let p3 = normalize_plotly_json_from_str(&s3).unwrap();
        assert!(p3.get("data").is_some());
    }
}

/// Linear interpolation function
///
/// # Arguments
/// * `target_freqs` - Target frequencies to interpolate to
/// * `source_freqs` - Source frequency array
/// * `source_spls` - Source SPL values
///
/// # Returns
/// * Interpolated SPL values at target frequencies
pub fn interpolate(
    target_freqs: &Array1<f64>,
    source_freqs: &Array1<f64>,
    source_spls: &Array1<f64>,
) -> Array1<f64> {
    let mut result = Array1::zeros(target_freqs.len());

    for (i, &target_freq) in target_freqs.iter().enumerate() {
        // Find the two nearest points in the source data
        let mut left_idx = 0;
        let mut right_idx = source_freqs.len() - 1;

        // Binary search for the closest points
        if target_freq <= source_freqs[0] {
            // Target frequency is below the range, use the first point
            result[i] = source_spls[0];
        } else if target_freq >= source_freqs[source_freqs.len() - 1] {
            // Target frequency is above the range, use the last point
            result[i] = source_spls[source_freqs.len() - 1];
        } else {
            // Find the two points that bracket the target frequency
            for j in 1..source_freqs.len() {
                if source_freqs[j] >= target_freq {
                    left_idx = j - 1;
                    right_idx = j;
                    break;
                }
            }

            // Linear interpolation
            let freq_left = source_freqs[left_idx];
            let freq_right = source_freqs[right_idx];
            let spl_left = source_spls[left_idx];
            let spl_right = source_spls[right_idx];

            let t = (target_freq - freq_left) / (freq_right - freq_left);
            result[i] = spl_left + t * (spl_right - spl_left);
        }
    }

    result
}

/// Clamp only positive dB values to +max_db, leave negatives unchanged
///
/// # Arguments
/// * `arr` - Array of SPL values
/// * `max_db` - Maximum positive dB value
///
/// # Returns
/// * Array with positive values clamped to max_db
pub fn clamp_positive_only(arr: &Array1<f64>, max_db: f64) -> Array1<f64> {
    arr.mapv(|v| if v > 0.0 { v.min(max_db) } else { v })
}

/// Simple 1/N-octave smoothing: for each frequency f_i, average values whose
/// frequency lies within [f_i * 2^(-1/(2N)), f_i * 2^(1/(2N))]
///
/// # Arguments
/// * `freqs` - Frequency array
/// * `values` - SPL values to smooth
/// * `n` - Number of bands per octave
///
/// # Returns
/// * Smoothed SPL values
pub fn smooth_one_over_n_octave(
    freqs: &Array1<f64>,
    values: &Array1<f64>,
    n: usize,
) -> Array1<f64> {
    let n = n.max(1);
    let half_win = (2.0_f64).powf(1.0 / (2.0 * n as f64));
    let mut out = Array1::zeros(values.len());
    for i in 0..freqs.len() {
        let f = freqs[i].max(1e-12);
        let lo = f / half_win;
        let hi = f * half_win;
        let mut sum = 0.0;
        let mut cnt = 0usize;
        for j in 0..freqs.len() {
            let fj = freqs[j];
            if fj >= lo && fj <= hi {
                sum += values[j];
                cnt += 1;
            }
        }
        out[i] = if cnt > 0 { sum / cnt as f64 } else { values[i] };
    }
    out
}

#[cfg(test)]
mod clamp_and_smooth_tests {
    use super::{clamp_positive_only, smooth_one_over_n_octave};
    use ndarray::Array1;

    #[test]
    fn clamp_positive_only_clamps_only_positive_side() {
        let arr = Array1::from(vec![-15.0, -1.0, 0.0, 1.0, 10.0, 25.0]);
        let out = clamp_positive_only(&arr, 12.0);
        assert_eq!(out.to_vec(), vec![-15.0, -1.0, 0.0, 1.0, 10.0, 12.0]);
    }

    #[test]
    fn smooth_one_over_n_octave_basic_monotonic() {
        // Simple check: with N large, window small -> output close to input
        let freqs = Array1::from(vec![100.0, 200.0, 400.0, 800.0]);
        let vals = Array1::from(vec![0.0, 1.0, 0.0, -1.0]);
        let out = smooth_one_over_n_octave(&freqs, &vals, 24);
        // Expect no drastic change
        for (o, v) in out.iter().zip(vals.iter()) {
            assert!((o - v).abs() <= 0.5);
        }
    }
}
