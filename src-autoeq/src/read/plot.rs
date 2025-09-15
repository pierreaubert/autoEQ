use crate::read::plot;
use serde_json::Value;
use std::error::Error;

pub fn normalize_plotly_value(v: &Value) -> Result<Value, Box<dyn Error>> {
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

pub fn normalize_plotly_json_from_str(content: &str) -> Result<Value, Box<dyn Error>> {
	// Content could be one of:
	// - Already a Plotly JSON object with "data" key
	// - A JSON array with one string (API response)
	// - A bare JSON string containing the Plotly JSON
	let v: Value = serde_json::from_str(content)?;
	if v.is_object() {
		return Ok(v);
	}
	if let Ok(parsed) = plot::normalize_plotly_value(&v) {
		return Ok(parsed);
	}
	if let Some(s) = v.as_str() {
		let inner: Value = serde_json::from_str(s)?;
		return Ok(inner);
	}
	Err("Unrecognized cached JSON format".into())
}

#[cfg(test)]
mod tests {
	use super::normalize_plotly_json_from_str;
	use serde_json::json;
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
