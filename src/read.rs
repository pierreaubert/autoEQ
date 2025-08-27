use std::error::Error;
use std::path::PathBuf;

use csv::ReaderBuilder;
use ndarray::Array1;
use reqwest;
use serde::Deserialize;
use serde_json::Value;
use urlencoding;

#[derive(Debug, Deserialize)]
struct CsvRecord {
    frequency: f64,
    spl: f64,
}

pub fn read_curve_from_csv(path: &PathBuf) -> Result<super::Curve, Box<dyn Error>> {
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

pub async fn fetch_curve_from_api(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<super::Curve, Box<dyn Error>> {
    // URL-encode the parameters
    let encoded_speaker = urlencoding::encode(speaker);
    let encoded_version = urlencoding::encode(version);
    let encoded_measurement = urlencoding::encode(measurement);

    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements/{}?measurement_format=json",
        encoded_speaker, encoded_version, encoded_measurement
    );

    println!("🔄 Fetching data from {}", url);

    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Err(format!("API request failed with status: {}", response.status()).into());
    }

    let api_response: Value = response.json().await?;

    // Extract frequency and SPL data from the Plotly JSON structure
    let mut freqs = Vec::new();
    let mut spls = Vec::new();

    // The API response is a list with a single element that is a JSON string
    let data_string = if let Some(array) = api_response.as_array() {
        println!("API response array length: {}", array.len());
        if let Some(first_element) = array.get(0) {
            first_element
                .as_str()
                .ok_or("First element is not a string")?
        } else {
            return Err("Empty API response".into());
        }
    } else {
        return Err("API response is not an array".into());
    };

    let plot_data: Value = serde_json::from_str(&data_string)?;

    // Also print the list of trace names found for debugging/inspection
    let trace_names = collect_trace_names(&plot_data);
    println!("Trace names: {:?}", trace_names);

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
                println!("Found SPL trace");

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
                                println!("Decoded {} frequency values", freqs.len());
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
                                println!("Decoded {} SPL values", spls.len());
                            }
                        }
                    }

                    break;
                }
            }
        }
    }

    if freqs.is_empty() {
        return Err("Failed to extract frequency and SPL data from API response".into());
    }

    println!("Extracted {} frequency points", freqs.len());

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
    if measurement.eq_ignore_ascii_case("CEA2034") {
        // For CEA2034 data, select the specific curve provided by the user
        // Prefer exact match; allow substring match as a fallback
        candidate == curve_name || candidate.contains(curve_name)
    } else {
        // Fallback heuristic for other measurement types
        candidate.contains("Listening Window")
            || candidate.contains("CEA2034")
            || candidate.contains("On Axis")
            || candidate.contains("SPL")
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

#[cfg(test)]
mod tests {
    use super::{collect_trace_names, is_target_trace_name};
    use serde_json::json;

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
    fn matches_target_name_for_cea2034() {
        assert!(is_target_trace_name("CEA2034", "Listening Window", "Listening Window"));
        assert!(is_target_trace_name("CEA2034", "On Axis", "On Axis"));
        assert!(!is_target_trace_name("CEA2034", "On Axis", "Early Reflections"));
        // Substring fallback
        assert!(is_target_trace_name("CEA2034", "Listening", "Listening Window"));
    }

    #[test]
    fn fallback_for_non_cea_measurements() {
        assert!(is_target_trace_name("Other", "ignored", "On Axis"));
        assert!(is_target_trace_name("Other", "ignored", "SPL something"));
        assert!(!is_target_trace_name("Other", "ignored", "Early Reflections DI"));
    }
}
