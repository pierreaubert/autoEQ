use ndarray::Array1;

/// Interpolate frequency response to a standard grid using linear interpolation in log space
///
/// # Arguments
/// * `freq_in` - Input frequency points
/// * `spl_in` - Input SPL values
/// * `freq_out` - Target frequency grid for interpolation
///
/// # Returns
/// * Interpolated SPL values on the target grid
pub fn interpolate_log_space(
    freq_in: &Array1<f64>,
    spl_in: &Array1<f64>,
    freq_out: &Array1<f64>,
) -> Array1<f64> {
    let n_out = freq_out.len();
    let n_in = freq_in.len();
    let mut spl_out = Array1::zeros(n_out);

    // Convert to log space for interpolation
    let log_freq_in: Vec<f64> = freq_in.iter().map(|f| f.ln()).collect();
    let log_freq_out: Vec<f64> = freq_out.iter().map(|f| f.ln()).collect();

    for i in 0..n_out {
        let target_log_freq = log_freq_out[i];

        // Find surrounding points for interpolation
        if target_log_freq <= log_freq_in[0] {
            // Extrapolate from first two points
            if n_in >= 2 {
                let slope = (spl_in[1] - spl_in[0]) / (log_freq_in[1] - log_freq_in[0]);
                spl_out[i] = spl_in[0] + slope * (target_log_freq - log_freq_in[0]);
            } else {
                spl_out[i] = spl_in[0];
            }
        } else if target_log_freq >= log_freq_in[n_in - 1] {
            // Extrapolate from last two points
            if n_in >= 2 {
                let slope = (spl_in[n_in - 1] - spl_in[n_in - 2])
                    / (log_freq_in[n_in - 1] - log_freq_in[n_in - 2]);
                spl_out[i] = spl_in[n_in - 1] + slope * (target_log_freq - log_freq_in[n_in - 1]);
            } else {
                spl_out[i] = spl_in[n_in - 1];
            }
        } else {
            // Linear interpolation between surrounding points
            let mut j = 0;
            while j < n_in - 1 && log_freq_in[j + 1] < target_log_freq {
                j += 1;
            }

            // Interpolate between j and j+1
            let t = (target_log_freq - log_freq_in[j]) / (log_freq_in[j + 1] - log_freq_in[j]);
            spl_out[i] = spl_in[j] * (1.0 - t) + spl_in[j + 1] * t;
        }
    }

    spl_out
}

/// Create a standard logarithmic frequency grid
pub fn create_log_frequency_grid(n_points: usize, f_min: f64, f_max: f64) -> Array1<f64> {
    Array1::logspace(10.0, f_min.log10(), f_max.log10(), n_points)
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
