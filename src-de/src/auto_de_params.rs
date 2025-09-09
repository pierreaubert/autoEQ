/// Parameters for AutoDE (AutoEQ Differential Evolution) algorithm
#[derive(Debug, Clone)]
pub struct AutoDEParams {
    /// Maximum number of iterations/generations
    pub max_iterations: usize,
    /// Population size (None = auto-sized based on problem dimension)
    pub population_size: Option<usize>,
    /// Mutation factor F ∈ [0, 2] (typical: 0.5-0.8)
    pub f: f64,
    /// Crossover probability CR ∈ [0, 1] (typical: 0.7-0.9)
    pub cr: f64,
    /// Convergence tolerance for objective function
    pub tolerance: f64,
    /// Random seed for reproducibility (None = random)
    pub seed: Option<u64>,
}

impl Default for AutoDEParams {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            population_size: None, // Auto-sized
            f: 0.8,
            cr: 0.9,
            tolerance: 1e-6,
            seed: None,
        }
    }
}
