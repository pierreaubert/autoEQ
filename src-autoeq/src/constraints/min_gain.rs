/// Data needed by the nonlinear minimum gain constraint callback.
#[derive(Clone, Copy)]
pub struct MinGainConstraintData {
	/// Minimum required absolute gain in dB
	pub min_db: f64,
	/// Whether first filter is highpass (skip in constraint)
	pub iir_hp_pk: bool,
}

/// Inequality constraint: for Peak filters, require |gain| >= min_db (skip HP in HP+PK mode).
/// Returns fc(x) = max_i (min_db - |g_i|) over applicable filters. Feasible when <= 0.
pub fn constraint_min_gain(
	x: &[f64],
	_grad: Option<&mut [f64]>,
	data: &mut MinGainConstraintData,
) -> f64 {
	if data.min_db <= 0.0 {
		return 0.0;
	}
	let n = x.len() / 3;
	if n == 0 {
		return 0.0;
	}
	let mut worst = f64::NEG_INFINITY;
	for i in 0..n {
		if data.iir_hp_pk && i == 0 {
			continue;
		}
		let g_abs = x[i * 3 + 2].abs();
		let short = data.min_db - g_abs; // can be negative when satisfied
		if short > worst {
			worst = short;
		}
	}
	if worst.is_finite() {
		worst
	} else {
		0.0
	}
}

/// Compute minimum gain constraint violation from parameter vector
///
/// Calculates the worst violation of minimum absolute gain requirement.
/// Only applies to peak filters (skips highpass filter in HP+PK mode).
///
/// # Arguments
/// * `xs` - Parameter vector with [log10(freq), Q, gain] triplets
/// * `iir_hp_pk` - Whether HP+PK mode is enabled (skip first filter)
/// * `min_db` - Minimum required absolute gain in dB
///
/// # Returns
/// Worst gain deficiency (0.0 if no violation or disabled)
pub fn viol_min_gain_from_xs(xs: &[f64], iir_hp_pk: bool, min_db: f64) -> f64 {
	if min_db <= 0.0 {
		return 0.0;
	}
	let n = xs.len() / 3;
	if n == 0 {
		return 0.0;
	}
	let mut worst_short = 0.0_f64;
	for i in 0..n {
		if iir_hp_pk && i == 0 {
			continue;
		}
		let g_abs = xs[i * 3 + 2].abs();
		let short = (min_db - g_abs).max(0.0);
		if short > worst_short {
			worst_short = short;
		}
	}
	worst_short
}
