use ndarray::Array1;

use super::interpolate::*;

const NORMALIZE_LOW_FREQ: f64 = 1000.0;
const NORMALIZE_HIGH_FREQ: f64 = 2000.0;

/// Normalize frequency response by subtracting mean in 100Hz-12kHz range
pub fn normalize_response(
	freq: &Array1<f64>,
	spl: &Array1<f64>,
	f_min: f64,
	f_max: f64,
) -> Array1<f64> {
	let mut sum = 0.0;
	let mut count = 0;

	// Calculate mean in the specified frequency range
	for i in 0..freq.len() {
		if freq[i] >= f_min && freq[i] <= f_max {
			sum += spl[i];
			count += 1;
		}
	}

	if count > 0 {
		let mean = sum / count as f64;
		spl - mean // Subtract mean from all values
	} else {
		spl.clone() // Return unchanged if no points in range
	}
}

/// Normalize both input and target, potentially normalize to a common frequency grid
///
/// # Arguments
/// * `freq` - Frequency points
/// * `spl` - SPL values
/// * `target_data` - Optional target frequency response data
///
/// # Returns
/// * Tuple of (frequency data, deviation data) for computing headphone loss
pub fn normalize_both_curves(
	freq: &ndarray::Array1<f64>,
	spl: &ndarray::Array1<f64>,
	target_data: Option<(&ndarray::Array1<f64>, &ndarray::Array1<f64>)>,
) -> (ndarray::Array1<f64>, ndarray::Array1<f64>) {
	if let Some((target_freq, target_spl)) = target_data {
		// Check if frequencies match
		let frequencies_match = freq.len() == target_freq.len()
			&& freq
				.iter()
				.zip(target_freq.iter())
				.all(|(f1, f2)| (f1 - f2).abs() / f1.max(*f2) < 0.01); // 1% tolerance

		if frequencies_match {
			// Same frequency grid - normalize and use directly
			let spl_norm = normalize_response(&freq, &spl, NORMALIZE_LOW_FREQ, 12000.0);
			let target_norm = normalize_response(
				&target_freq,
				&target_spl,
				NORMALIZE_LOW_FREQ,
				NORMALIZE_HIGH_FREQ,
			);

			// Compute deviation from normalized target
			let deviation = &target_norm - &spl_norm;
			(freq.clone(), deviation)
		} else {
			// Different grids - resample both to common grid

			// Create standard grid: 200 points from 20 Hz to 20 kHz
			let standard_freq = create_log_frequency_grid(200, 20.0, 20000.0);

			// Interpolate both curves to standard grid
			let spl_interp = interpolate_log_space(&freq, &spl, &standard_freq);
			let target_interp = interpolate_log_space(&target_freq, &target_spl, &standard_freq);

			// Normalize after interpolation
			let spl_norm = normalize_response(
				&standard_freq,
				&spl_interp,
				NORMALIZE_LOW_FREQ,
				NORMALIZE_HIGH_FREQ,
			);
			let target_norm = normalize_response(
				&standard_freq,
				&target_interp,
				NORMALIZE_LOW_FREQ,
				NORMALIZE_HIGH_FREQ,
			);

			// Compute deviation from normalized target
			let deviation = &target_norm - &spl_norm;
			(standard_freq, deviation)
		}
	} else {
		// Compute absolute headphone loss
		// If frequency grid is sparse, resample to standard grid
		if freq.len() < 50 {
			println!(
				"  Sparse frequency grid detected ({} points) - resampling to 200-point log grid",
				freq.len()
			);
			let standard_freq = create_log_frequency_grid(200, 20.0, 20000.0);
			let spl_interp = interpolate_log_space(&freq, &spl, &standard_freq);
			println!("  Computing absolute headphone loss on resampled data");
			(standard_freq, spl_interp)
		} else {
			println!("  Computing absolute headphone loss");
			(freq.clone(), spl.clone())
		}
	}
}
