use ndarray::Array1;

pub(crate) fn clip_free_inplace(
    x: &mut Array1<f64>,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    is_free: &[bool],
) {
    for i in 0..x.len() {
        if is_free[i] {
            if x[i] < lower[i] {
                x[i] = lower[i];
            }
            if x[i] > upper[i] {
                x[i] = upper[i];
            }
        } else {
            x[i] = lower[i]; // fixed var equals bound
        }
    }
}
