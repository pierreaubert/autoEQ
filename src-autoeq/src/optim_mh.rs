// Metaheuristics-specific optimization code

use super::optim::{ObjectiveData, compute_fitness_penalties};

#[allow(unused_imports)]
use metaheuristics_nature as mh;
#[allow(unused_imports)]
use mh::methods::{De as MhDe, Fa as MhFa, Pso as MhPso, Rga as MhRga, Tlbo as MhTlbo};
#[allow(unused_imports)]
use mh::{Bounded as MhBounded, Fitness as MhFitness, ObjFunc as MhObjFunc, Solver as MhSolver};

// ---------------- Metaheuristics objective and utilities ----------------
#[derive(Clone)]
pub struct MHObjective {
    pub data: ObjectiveData,
    pub bounds: Vec<[f64; 2]>,
}

impl MhBounded for MHObjective {
    fn bound(&self) -> &[[f64; 2]] {
        self.bounds.as_slice()
    }
}

impl MhObjFunc for MHObjective {
    type Ys = f64;
    fn fitness(&self, xs: &[f64]) -> Self::Ys {
        // Create mutable copy of data for compute_fitness_penalties
        let mut data_copy = self.data.clone();
        compute_fitness_penalties(xs, None, &mut data_copy)
    }
}

/// Optimize filter parameters using metaheuristics algorithms
pub fn optimize_filters_mh(
    x: &mut [f64],
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    objective_data: ObjectiveData,
    mh_name: &str,
    population: usize,
    maxeval: usize,
) -> Result<(String, f64), (String, f64)> {
    let num_params = x.len();

    // Build bounds for metaheuristics (as pairs)
    assert_eq!(lower_bounds.len(), num_params);
    assert_eq!(upper_bounds.len(), num_params);
    let mut bounds: Vec<[f64; 2]> = Vec::with_capacity(num_params);
    for i in 0..num_params {
        bounds.push([lower_bounds[i], upper_bounds[i]]);
    }

    // Create objective with penalties (metaheuristics don't support constraints)
    let mut penalty_data = objective_data.clone();
    penalty_data.penalty_w_ceiling = 1e4;
    penalty_data.penalty_w_spacing = objective_data.spacing_weight.max(0.0) * 1e3;
    penalty_data.penalty_w_mingain = 1e3;

    // Simple objective function wrapper for metaheuristics
    let mh_obj = MHObjective {
        data: penalty_data,
        bounds,
    };

    // Choose algorithm configuration
    // Use boxed builder to allow runtime selection with unified type
    let builder = match mh_name {
        "de" => MhSolver::build_boxed(MhDe::default(), mh_obj),
        "pso" => MhSolver::build_boxed(MhPso::default(), mh_obj),
        "rga" => MhSolver::build_boxed(MhRga::default(), mh_obj),
        "tlbo" => MhSolver::build_boxed(MhTlbo::default(), mh_obj),
        "fa" | "firefly" => MhSolver::build_boxed(MhFa::default(), mh_obj),
        _ => MhSolver::build_boxed(MhDe::default(), mh_obj),
    };

    // Estimate generations from maxeval and population
    let pop = population.max(1);
    let gens = ((maxeval.max(pop)) + pop - 1) / pop; // ceil(maxeval/pop)

    // Avoid accessing ctx.gen directly (reserved identifier in Rust 2024).
    // Instead, count down generations via the task FnMut closure.
    let mut left = gens as i64;
    let solver = builder
        .seed(0)
        .pop_num(pop)
        .task(move |_| {
            left -= 1;
            left <= 0
        })
        .solve();

    // Write back the best parameters
    let best_xs = solver.as_best_xs();
    if best_xs.len() == x.len() {
        x.copy_from_slice(best_xs);
    }
    let best_val = *solver.as_best_fit();
    Ok((format!("Metaheuristics({})", mh_name), best_val))
}