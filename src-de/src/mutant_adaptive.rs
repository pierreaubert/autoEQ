use std::cmp::Ordering;
use ndarray::{Array1, Array2};
use rand::Rng;
use rand::seq::SliceRandom;

use crate::mutant_rand1::mutant_rand1;

/// Adaptive mutation based on Self-Adaptive Mutation (SAM) from the paper
/// Uses linearly decreasing weight w to select from top individuals
pub(crate) fn mutant_adaptive<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    energies: &Array1<f64>,
    w: f64,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    // Calculate w% of population size for adaptive selection
    let w_size = ((w * pop.nrows() as f64) as usize).max(1).min(pop.nrows() - 1);
    
    // Get sorted indices by fitness (best to worst)
    let mut sorted_indices: Vec<usize> = (0..pop.nrows()).collect();
    sorted_indices.sort_by(|&a, &b| energies[a].partial_cmp(&energies[b]).unwrap_or(Ordering::Equal));
    
    // Select gr_better from top w% individuals randomly
    let top_indices = &sorted_indices[0..w_size];
    let gr_better_idx = top_indices[rng.random_range(0..w_size)];
    // Get two distinct random indices different from i and gr_better_idx
    let mut available: Vec<usize> = (0..pop.nrows()).filter(|&idx| idx != i && idx != gr_better_idx).collect();
    available.shuffle(rng);
    
    if available.len() < 2 {
        // Fallback to standard rand1 if not enough individuals
        return mutant_rand1(i, pop, f, rng);
    }
    
    let r1 = available[0];
    let r2 = available[1];
    
    // Adaptive mutation: x_i + F * (x_gr_better - x_i + x_r1 - x_r2)
    // This is the SAM approach from equation (18) in the paper
    pop.row(i).to_owned()
        + &((pop.row(gr_better_idx).to_owned() - pop.row(i).to_owned() + pop.row(r1).to_owned()
            - pop.row(r2).to_owned())
            * f)
}
