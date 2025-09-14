use ndarray::Array1;
use super::super::x2peq::x2peq;

/// Data needed by the nonlinear ceiling constraint callback.
#[derive(Clone)]
pub struct CeilingConstraintData {
    /// Frequency points for evaluation (Hz)
    pub freqs: Array1<f64>,
    /// Sample rate in Hz
    pub srate: f64,
    /// Maximum allowed SPL in dB
    pub max_db: f64,
    /// Whether first filter is highpass (HP+PK mode)
    pub iir_hp_pk: bool,
}

/// Inequality constraint: combined response must not exceed max_db when HP+PK is enabled.
/// Returns fc(x) = max_i (peq_spl[i] - max_db). Feasible when <= 0.
pub fn constraint_ceiling(
    x: &[f64],
    _grad: Option<&mut [f64]>,
    data: &mut CeilingConstraintData,
) -> f64 {
    let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);
    // return max excess (can be negative if no violation)
    let mut max_excess = f64::NEG_INFINITY;
    for &v in peq_spl.iter() {
        let excess = v - data.max_db;
        if excess > max_excess {
            max_excess = excess;
        }
    }
    if max_excess.is_finite() {
        max_excess
    } else {
        0.0
    }
}

/// Compute ceiling constraint violation from frequency response
///
/// Calculates the maximum excess over the allowed SPL ceiling. Only applies
/// in HP+PK mode where we need to limit the combined response.
///
/// # Arguments
/// * `peq_spl` - Frequency response in dB SPL
/// * `max_db` - Maximum allowed SPL ceiling
/// * `iir_hp_pk` - Whether HP+PK mode is enabled (constraint only active then)
///
/// # Returns
/// Maximum violation amount (0.0 if no violation or not HP+PK mode)
pub fn viol_ceiling_from_spl(peq_spl: &Array1<f64>, max_db: f64, iir_hp_pk: bool) -> f64 {
    if !iir_hp_pk {
        return 0.0;
    }
    let mut max_excess = 0.0_f64;
    for &v in peq_spl.iter() {
        let excess = (v - max_db).max(0.0);
        if excess > max_excess {
            max_excess = excess;
        }
    }
    max_excess
}

