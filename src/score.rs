use ndarray::concatenate;
use ndarray::s;
use ndarray::{Array1, Array2, Axis};

fn spl2pressure(spl: &Array1<f64>) -> Array1<f64> {
    // 10^((spl-105)/20)
    spl.mapv(|v| 10f64.powf((v - 105.0) / 20.0))
}

fn pressure2spl(p: &Array1<f64>) -> Array1<f64> {
    // 20*log10(p) + 105
    p.mapv(|v| 20.0 * v.log10() + 105.0)
}

fn spl2pressure2(spl: &Array2<f64>) -> Array2<f64> {
    // square(pressure) per row
    let mut out = Array2::<f64>::zeros(spl.raw_dim());
    for (mut row_out, row_in) in out.axis_iter_mut(Axis(0)).zip(spl.axis_iter(Axis(0))) {
        let p = spl2pressure(&row_in.to_owned());
        row_out.assign(&p.mapv(|x| x * x));
    }
    out
}

fn cea2034_array(spl: &Array2<f64>, idx: &[Vec<usize>], weights: &Array1<f64>) -> Array2<f64> {
    let len_spl = spl.shape()[1];
    let p2 = spl2pressure2(spl);
    let idx_sp = idx.len() - 1;
    let idx_lw = 0usize;
    let idx_er = 1usize;
    let idx_pir = idx_sp + 1;

    let mut cea = Array2::<f64>::zeros((idx.len() + 1, len_spl));

    for i in 0..idx_sp {
        let curve = apply_rms(&p2, &idx[i]);
        cea.row_mut(i).assign(&curve);
    }

    // ER: indices 2..=6 per original logic
    let mut er_p2 = Array1::<f64>::zeros(len_spl);
    for i in 2..=6 {
        let pres = spl2pressure(&cea.row(i).to_owned());
        er_p2 = &er_p2 + &pres.mapv(|v| v * v);
    }
    let er = er_p2.mapv(|v| (v / 5.0).sqrt());
    let er_spl = pressure2spl(&er);
    cea.row_mut(idx_er).assign(&er_spl);

    // SP weighted
    let sp_curve = apply_weighted_rms(&p2, &idx[idx_sp], weights);
    cea.row_mut(idx_sp).assign(&sp_curve);

    // PIR
    let lw_p = spl2pressure(&cea.row(idx_lw).to_owned());
    let er_p = spl2pressure(&cea.row(idx_er).to_owned());
    let sp_p = spl2pressure(&cea.row(idx_sp).to_owned());
    let mut pir = Array1::<f64>::zeros(len_spl);
    for ((pir_v, lw), (er, sp)) in pir
        .iter_mut()
        .zip(lw_p.iter())
        .zip(er_p.iter().zip(sp_p.iter()))
    {
        *pir_v = (0.12 * lw * lw + 0.44 * er * er + 0.44 * sp * sp).sqrt();
    }
    let pir_spl = pressure2spl(&pir);
    cea.row_mut(idx_pir).assign(&pir_spl);

    cea
}

fn apply_rms(p2: &Array2<f64>, idx: &[usize]) -> Array1<f64> {
    // sqrt(sum(p2[idx]) / len) then to SPL
    let ncols = p2.shape()[1];
    let mut acc = Array1::<f64>::zeros(ncols);
    for &i in idx {
        acc = &acc + &p2.row(i).to_owned();
    }
    let len_idx = idx.len() as f64;
    let r = acc.mapv(|v| (v / len_idx).sqrt());
    pressure2spl(&r)
}

fn apply_weighted_rms(p2: &Array2<f64>, idx: &[usize], weights: &Array1<f64>) -> Array1<f64> {
    let ncols = p2.shape()[1];
    let mut acc = Array1::<f64>::zeros(ncols);
    let mut sum_w = 0.0;
    for &i in idx {
        let w = weights[i];
        acc = &acc + &(p2.row(i).to_owned() * w);
        sum_w += w;
    }
    let r = acc.mapv(|v| (v / sum_w).sqrt());
    pressure2spl(&r)
}

fn mad(spl: &Array1<f64>, imin: usize, imax: usize) -> f64 {
    let slice = spl.slice(s![imin..imax]).to_owned();
    let m = slice.mean().unwrap_or(0.0);
    let diffs = slice.mapv(|v| (v - m).abs());
    diffs.mean().unwrap_or(f64::NAN)
}

fn consecutive_groups_first_group(indices: &[(usize, f64)]) -> Vec<(usize, f64)> {
    // Return the first group of consecutive indices
    if indices.is_empty() {
        return Vec::new();
    }
    let mut group: Vec<(usize, f64)> = Vec::new();
    let mut prev = indices[0].0;
    group.push(indices[0]);
    for &(i, f) in indices.iter().skip(1) {
        if i == prev + 1 {
            group.push((i, f));
            prev = i;
        } else {
            break; // only first group
        }
    }
    group
}

fn r_squared(x: &Array1<f64>, y: &Array1<f64>) -> f64 {
    // Pearson correlation squared
    let n = x.len() as f64;
    if n == 0.0 {
        return f64::NAN;
    }
    let mx = x.mean().unwrap_or(0.0);
    let my = y.mean().unwrap_or(0.0);
    let mut num = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for (xi, yi) in x.iter().zip(y.iter()) {
        let dx = *xi - mx;
        let dy = *yi - my;
        num += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 || syy == 0.0 {
        return f64::NAN;
    }
    let r = num / (sxx.sqrt() * syy.sqrt());
    r * r
}

// ---------------- Pure Rust API below ----------------

pub fn cea2034(spl: &Array2<f64>, idx: &[Vec<usize>], weights: &Array1<f64>) -> Array2<f64> {
    cea2034_array(spl, idx, weights)
}

pub fn octave(count: usize) -> Vec<(f64, f64, f64)> {
    assert!(count >= 2, "count (N) must be >= 2");
    let reference = 1290.0_f64;
    let p = 2.0_f64.powf(1.0 / count as f64);
    let p_band = 2.0_f64.powf(1.0 / (2.0 * count as f64));
    let o_iter: i32 = ((count as i32 * 10 + 1) / 2) as i32;
    let mut centers: Vec<f64> = Vec::with_capacity((o_iter as usize) * 2 + 1);
    for i in (1..=o_iter).rev() {
        centers.push(reference / p.powi(i));
    }
    centers.push(reference);
    for i in 1..=o_iter {
        centers.push(reference * p.powi(i));
    }
    centers
        .into_iter()
        .map(|c| (c / p_band, c, c * p_band))
        .collect()
}

pub fn octave_intervals(count: usize, freq: &Array1<f64>) -> Vec<(usize, usize)> {
    let bands = octave(count);
    let mut out = Vec::with_capacity(bands.len());
    for (low, _c, high) in bands.into_iter() {
        let imin = freq.iter().position(|&f| f >= low).unwrap_or(freq.len());
        let imax = freq.iter().position(|&f| f >= high).unwrap_or(freq.len());
        out.push((imin, imax));
    }
    out
}

pub fn nbd(intervals: &[(usize, usize)], spl: &Array1<f64>) -> f64 {
    let mut sum = 0.0;
    let mut cnt = 0.0;
    for &(imin, imax) in intervals.iter() {
        let v = mad(spl, imin, imax);
        if v.is_finite() {
            sum += v;
            cnt += 1.0;
        }
    }
    if cnt == 0.0 { f64::NAN } else { sum / cnt }
}

pub fn lfx(freq: &Array1<f64>, lw: &Array1<f64>, sp: &Array1<f64>) -> f64 {
    let lw_min = freq.iter().position(|&f| f > 300.0).unwrap_or(freq.len());
    let lw_max = freq
        .iter()
        .position(|&f| f >= 10000.0)
        .unwrap_or(freq.len());
    if lw_min >= lw_max {
        return (300.0_f64).log10();
    }
    let lw_ref = lw.slice(s![lw_min..lw_max]).mean().unwrap_or(0.0) - 6.0;
    let mut lfx_range: Vec<(usize, f64)> = Vec::new();
    for (i, (&f, &spv)) in freq
        .iter()
        .take(lw_min)
        .zip(sp.iter().take(lw_min))
        .enumerate()
    {
        if spv <= lw_ref {
            lfx_range.push((i, f));
        }
    }
    if lfx_range.is_empty() {
        return freq[0].log10();
    }
    let group = consecutive_groups_first_group(&lfx_range);
    if group.len() <= 1 {
        return (300.0_f64).log10();
    }
    let mut pos = group.last().unwrap().0;
    if freq.len() < pos - 1 {
        pos += 1;
    }
    freq[pos].log10()
}

pub fn sm(freq: &Array1<f64>, spl: &Array1<f64>) -> f64 {
    let f_min = freq.iter().position(|&f| f > 100.0).unwrap_or(freq.len());
    let f_max = freq
        .iter()
        .position(|&f| f >= 16000.0)
        .unwrap_or(freq.len());
    if f_min >= f_max {
        return f64::NAN;
    }
    let x: Array1<f64> = freq.slice(s![f_min..f_max]).mapv(|v| v.log10());
    let y: Array1<f64> = spl.slice(s![f_min..f_max]).to_owned();
    r_squared(&x, &y)
}

#[derive(Debug, Clone)]
pub struct ScoreMetrics {
    pub nbd_on: f64,
    pub nbd_pir: f64,
    pub lfx: f64,
    pub sm_pir: f64,
    pub pref_score: f64,
}

pub fn score(
    freq: &Array1<f64>,
    intervals: &[(usize, usize)],
    on: &Array1<f64>,
    lw: &Array1<f64>,
    sp: &Array1<f64>,
    pir: &Array1<f64>,
) -> ScoreMetrics {
    let nbd_on = nbd(intervals, on);
    let nbd_pir = nbd(intervals, pir);
    let sm_pir = sm(freq, pir);
    let lfx_val = lfx(freq, lw, sp);
    let pref = 12.69 - 2.49 * nbd_on - 2.99 * nbd_pir - 4.31 * lfx_val + 2.32 * sm_pir;
    ScoreMetrics {
        nbd_on,
        nbd_pir,
        lfx: lfx_val,
        sm_pir,
        pref_score: pref,
    }
}

pub fn score_peq(
    freq: &Array1<f64>,
    idx: &[Vec<usize>],
    intervals: &[(usize, usize)],
    weights: &Array1<f64>,
    spl_h: &Array2<f64>,
    spl_v: &Array2<f64>,
    peq: &Array1<f64>,
) -> (Array2<f64>, ScoreMetrics) {
    assert_eq!(
        peq.len(),
        spl_h.shape()[1],
        "peq length must match SPL columns"
    );
    assert_eq!(
        peq.len(),
        spl_v.shape()[1],
        "peq length must match SPL columns"
    );

    // add PEQ to each row
    let mut spl_h_peq = Array2::<f64>::zeros(spl_h.raw_dim());
    for (mut row_out, row_in) in spl_h_peq
        .axis_iter_mut(Axis(0))
        .zip(spl_h.axis_iter(Axis(0)))
    {
        row_out.assign(&(&row_in.to_owned() + peq));
    }
    let mut spl_v_peq = Array2::<f64>::zeros(spl_v.raw_dim());
    for (mut row_out, row_in) in spl_v_peq
        .axis_iter_mut(Axis(0))
        .zip(spl_v.axis_iter(Axis(0)))
    {
        row_out.assign(&(&row_in.to_owned() + peq));
    }

    let spl_full =
        concatenate(Axis(0), &[spl_h_peq.view(), spl_v_peq.view()]).expect("concatenate failed");
    let spin_nd = cea2034_array(&spl_full, idx, weights);

    // Prepare rows for scoring
    let on = spl_h_peq.row(17).to_owned();
    let lw = spin_nd.row(0).to_owned();
    let sp_row = spin_nd.row(spin_nd.shape()[0] - 2).to_owned();
    let pir = spin_nd.row(spin_nd.shape()[0] - 1).to_owned();

    let metrics = score(freq, intervals, &on, &lw, &sp_row, &pir);
    (spin_nd, metrics)
}

pub fn score_peq_approx(
    freq: &Array1<f64>,
    intervals: &[(usize, usize)],
    lw: &Array1<f64>,
    sp: &Array1<f64>,
    pir: &Array1<f64>,
    on: &Array1<f64>,
    peq: &Array1<f64>,
) -> ScoreMetrics {
    let on2 = on + peq;
    score(freq, intervals, &on2, lw, sp, pir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octave_count_2_includes_reference_center() {
        let bands = octave(2);
        // find the center equal to 1290
        assert!(bands.iter().any(|&(_l, c, _h)| (c - 1290.0).abs() < 1e-9));
    }

    #[test]
    fn nbd_simple_mean_of_mads() {
        let spl = Array1::from(vec![0.0, 1.0, 2.0, 1.0, 0.0]);
        // two intervals: [0..3) and [2..5)
        let intervals = vec![(0, 3), (2, 5)];
        let v = nbd(&intervals, &spl);
        assert!(v.is_finite());
    }

    #[test]
    fn score_peq_approx_matches_score_when_peq_zero() {
        // Simple synthetic data
        let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let intervals = vec![(0, 3)];
        let on = Array1::from(vec![80.0, 85.0, 82.0]);
        let lw = Array1::from(vec![81.0, 84.0, 83.0]);
        let sp = Array1::from(vec![79.0, 83.0, 81.0]);
        let pir = Array1::from(vec![80.5, 84.0, 82.0]);
        let zero = Array1::zeros(freq.len());

        let m1 = score(&freq, &intervals, &on, &lw, &sp, &pir);
        let m2 = score_peq_approx(&freq, &intervals, &lw, &sp, &pir, &on, &zero);

        assert!((m1.nbd_on - m2.nbd_on).abs() < 1e-12);
        assert!((m1.nbd_pir - m2.nbd_pir).abs() < 1e-12);
        assert!((m1.lfx - m2.lfx).abs() < 1e-12);
        assert!((m1.sm_pir - m2.sm_pir).abs() < 1e-12);
        assert!((m1.pref_score - m2.pref_score).abs() < 1e-12);
    }
}

pub fn compute_pir_from_lw_er_sp(
    lw: &Array1<f64>,
    er: &Array1<f64>,
    sp: &Array1<f64>,
) -> Array1<f64> {
    let lw_p = spl2pressure(lw);
    let er_p = spl2pressure(er);
    let sp_p = spl2pressure(sp);
    let lw2 = lw_p.mapv(|v| v * v);
    let er2 = er_p.mapv(|v| v * v);
    let sp2 = sp_p.mapv(|v| v * v);
    let pir_p2 = lw2.mapv(|v| 0.12 * v) + &er2.mapv(|v| 0.44 * v) + &sp2.mapv(|v| 0.44 * v);
    let pir_p = pir_p2.mapv(|v| v.sqrt());
    pressure2spl(&pir_p)
}

#[cfg(test)]
mod pir_helpers_tests {
    use super::{compute_pir_from_lw_er_sp, pressure2spl, spl2pressure};
    use ndarray::Array1;

    #[test]
    fn spl_pressure_roundtrip_is_identity() {
        let spl = Array1::from(vec![60.0, 80.0, 100.0]);
        let p = spl2pressure(&spl);
        let spl2 = pressure2spl(&p);
        for (a, b) in spl.iter().zip(spl2.iter()) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn pir_equals_input_when_all_equal() {
        let lw = Array1::from(vec![80.0, 80.0, 80.0]);
        let er = Array1::from(vec![80.0, 80.0, 80.0]);
        let sp = Array1::from(vec![80.0, 80.0, 80.0]);
        let pir = compute_pir_from_lw_er_sp(&lw, &er, &sp);
        for v in pir.iter() {
            assert!((*v - 80.0).abs() < 1e-12);
        }
    }

    #[test]
    fn pir_reflects_er_sp_weighting() {
        // ER and SP have higher weights than LW (0.44 each vs 0.12)
        let lw = Array1::from(vec![70.0, 70.0, 70.0]);
        let er = Array1::from(vec![80.0, 80.0, 80.0]);
        let sp = Array1::from(vec![80.0, 80.0, 80.0]);
        let pir = compute_pir_from_lw_er_sp(&lw, &er, &sp);
        for v in pir.iter() {
            assert!(*v > 75.0 && *v < 81.0);
        }
    }
}
