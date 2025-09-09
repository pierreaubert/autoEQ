//! Zakharov2 test function

use ndarray::Array1;
use crate::functions::zakharov::zakharov;

/// Zakharov function variant (2D specific)
pub fn zakharov2(x: &Array1<f64>) -> f64 {
    zakharov(x)
}
