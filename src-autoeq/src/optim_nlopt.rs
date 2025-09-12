// NLOPT-specific optimization code

use super::constraints::{
    constraint_ceiling, constraint_min_gain, viol_ceiling_from_spl, viol_min_gain_from_xs,
    viol_spacing_from_xs, x2peq, CeilingConstraintData, MinGainConstraintData,
};
use super::optim::{compute_base_fitness, ObjectiveData};
use nlopt::{Algorithm, Nlopt, Target};

/// Compute objective function value including penalty terms for constraints
///
/// This function adds penalty terms to the base fitness when using algorithms
/// that don't support native constraint handling.
///
/// # Arguments
/// * `x` - Parameter vector
/// * `_gradient` - Gradient vector (unused, for NLOPT compatibility)
/// * `data` - Objective data containing penalty weights and parameters
///
/// # Returns
/// Base fitness value plus weighted penalty terms
pub fn compute_fitness_penalties(
    x: &[f64],
    _gradient: Option<&mut [f64]>,
    data: &mut ObjectiveData,
) -> f64 {
    let fit = compute_base_fitness(x, data);

    // When penalties are enabled (weights > 0), add them to the base fit so that
    // optimizers without nonlinear constraints can still respect our limits.
    let mut penalized = fit;
    let mut penalty_terms = Vec::new();

    if data.penalty_w_ceiling > 0.0 {
        let peq_spl = x2peq(&data.freqs, x, data.srate, data.iir_hp_pk);
        let viol = viol_ceiling_from_spl(&peq_spl, data.max_db, data.iir_hp_pk);
        let penalty = data.penalty_w_ceiling * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "ceiling_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_ceiling, penalty
            ));
        }
    }

    if data.penalty_w_spacing > 0.0 {
        let viol = viol_spacing_from_xs(x, data.min_spacing_oct);
        let penalty = data.penalty_w_spacing * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "spacing_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_spacing, penalty
            ));
        }
    }

    if data.penalty_w_mingain > 0.0 && data.min_db > 0.0 {
        let viol = viol_min_gain_from_xs(x, data.iir_hp_pk, data.min_db);
        let penalty = data.penalty_w_mingain * viol * viol;
        penalized += penalty;
        if viol > 0.0 {
            penalty_terms.push(format!(
                "mingain_viol={:.3e}*{:.1e}={:.3e}",
                viol, data.penalty_w_mingain, penalty
            ));
        }
    }

    // // Log fitness details every 1000 evaluations (approximate)
    // use std::sync::atomic::{AtomicUsize, Ordering};
    // static EVAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
    // let count = EVAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    // if count % 1000 == 0 || (count % 100 == 0 && !penalty_terms.is_empty()) {
    //     let param_summary: Vec<String> = (0..x.len()/3).map(|i| {
    //         let freq = 10f64.powf(x[i*3]);
    //         let q = x[i*3+1];
    //         let gain = x[i*3+2];
    //         format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
    //     }).collect();

    //     eprintln!("TRACE[{}]: fit={:.3e}, penalties=[{}], params=[{}]",
    //               count, fit, penalty_terms.join(", "), param_summary.join(", "));
    // }

    penalized
}

/// Optimize filter parameters using NLOPT algorithms
pub fn optimize_filters_nlopt(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    algo: Algorithm,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Decide whether to use penalties (for algorithms lacking inequality constraints)
    let use_penalties = match algo {
        Algorithm::Crs2Lm
        | Algorithm::Direct
        | Algorithm::DirectL
        | Algorithm::GMlsl
        | Algorithm::GMlslLds
        | Algorithm::Sbplx
        | Algorithm::StoGo
        | Algorithm::StoGoRand
        | Algorithm::Neldermead => true,
        _ => false,
    };

    // Prepare constraint data BEFORE moving objective_data into NLopt
    let ceiling_data = CeilingConstraintData {
        freqs: objective_data.freqs.clone(),
        srate: objective_data.srate,
        max_db: objective_data.max_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };
    let min_gain_data = MinGainConstraintData {
        min_db: objective_data.min_db,
        iir_hp_pk: objective_data.iir_hp_pk,
    };

    // Configure penalty weights when needed
    let mut objective_data = objective_data;
    if use_penalties {
        objective_data.penalty_w_ceiling = 1e4;
        objective_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1e3;
        objective_data.penalty_w_mingain = 1e3;
    } else {
        objective_data.penalty_w_ceiling = 0.0;
        objective_data.penalty_w_spacing = 0.0;
        objective_data.penalty_w_mingain = 0.0;
    }

    // Now create optimizer and move objective_data
    let mut optimizer = Nlopt::new(
        algo,
        num_params,
        compute_fitness_penalties,
        Target::Minimize,
        objective_data,
    );

    let _ = optimizer.set_lower_bounds(lower_bounds).unwrap();
    let _ = optimizer.set_upper_bounds(upper_bounds).unwrap();

    // Register inequality constraints when not using penalties.
    if !use_penalties {
        let _ = optimizer.add_inequality_constraint(constraint_ceiling, ceiling_data, 1e-6);
        // let _ = optimizer.add_inequality_constraint(constraint_spacing, spacing_data, 1e-9);
        let _ = optimizer.add_inequality_constraint(constraint_min_gain, min_gain_data, 1e-6);
    }

    let _ = optimizer.set_population(population);
    let _ = optimizer.set_maxeval(maxeval as u32);
    let _ = optimizer.set_stopval(1e-4).unwrap();
    let _ = optimizer.set_ftol_rel(1e-6).unwrap();
    let _ = optimizer.set_xtol_rel(1e-4).unwrap();

    let result = optimizer.optimize(x);

    match result {
        Ok((status, val)) => Ok((format!("{:?}", status), val)),
        Err((e, val)) => Err((format!("{:?}", e), val)),
    }
}