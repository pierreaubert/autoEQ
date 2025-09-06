//! Differential Evolution (DE) global optimizer in pure Rust using ndarray
//!
//! This is a pragmatic, dependency-light implementation inspired by
//! SciPy's `scipy.optimize.differential_evolution`.
//!
//! Supported features:
//! - Box constraints (lower/upper bounds)
//! - Common strategies: best1bin, rand1bin, rand2bin, currenttobest1bin, best2bin
//! - Binomial crossover
//! - Mutation as a fixed factor or dithering in a range [min,max)
//! - Initialization: Latin Hypercube Sampling (LHS) or random uniform
//! - Optional initial guess `x0` overriding the best member after init
//! - Convergence by std(pop_f) <= atol + tol * |mean(pop_f)|
//! - Optional integrality mask to round decision variables to nearest integer

#![allow(missing_docs)]

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use ndarray::{Array1, Array2};
use nlopt::{Algorithm as NlAlgorithm, Nlopt, Target as NlTarget};
use rand::prelude::*;
use rand::seq::SliceRandom;

extern crate blas_src;

/// Differential Evolution strategy
#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    Best1Bin,
    Best1Exp,
    Rand1Bin,
    Rand1Exp,
    Rand2Bin,
    Rand2Exp,
    CurrentToBest1Bin,
    CurrentToBest1Exp,
    Best2Bin,
    Best2Exp,
    RandToBest1Bin,
    RandToBest1Exp,
}

impl FromStr for Strategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.to_lowercase();
        match t.as_str() {
            "best1bin" | "best1" => Ok(Strategy::Best1Bin),
            "best1exp" => Ok(Strategy::Best1Exp),
            "rand1bin" | "rand1" => Ok(Strategy::Rand1Bin),
            "rand1exp" => Ok(Strategy::Rand1Exp),
            "rand2bin" | "rand2" => Ok(Strategy::Rand2Bin),
            "rand2exp" => Ok(Strategy::Rand2Exp),
            "currenttobest1bin" | "current-to-best1bin" | "current_to_best1bin" => {
                Ok(Strategy::CurrentToBest1Bin)
            }
            "currenttobest1exp" | "current-to-best1exp" | "current_to_best1exp" => {
                Ok(Strategy::CurrentToBest1Exp)
            }
            "best2bin" | "best2" => Ok(Strategy::Best2Bin),
            "best2exp" => Ok(Strategy::Best2Exp),
            "randtobest1bin" | "rand-to-best1bin" | "rand_to_best1bin" => {
                Ok(Strategy::RandToBest1Bin)
            }
            "randtobest1exp" | "rand-to-best1exp" | "rand_to_best1exp" => {
                Ok(Strategy::RandToBest1Exp)
            }
            _ => Err(format!("unknown strategy: {}", s)),
        }
    }
}

/// Crossover type
#[derive(Debug, Clone, Copy)]
pub enum Crossover {
    /// Binomial (uniform) crossover
    Binomial,
    /// Exponential crossover
    Exponential,
}

impl Default for Crossover {
    fn default() -> Self {
        Crossover::Binomial
    }
}

/// Mutation setting: either a fixed factor or a uniform range (dithering)
#[derive(Debug, Clone, Copy)]
pub enum Mutation {
    /// Fixed mutation factor F in [0, 2)
    Factor(f64),
    /// Dithering range [min, max) with 0 <= min < max <= 2
    Range { min: f64, max: f64 },
}

impl Default for Mutation {
    fn default() -> Self {
        Mutation::Factor(0.8)
    }
}

impl Mutation {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        match *self {
            Mutation::Factor(f) => f,
            Mutation::Range { min, max } => rng.random_range(min..max),
        }
    }
}

/// Initialization scheme for the population
#[derive(Debug, Clone, Copy)]
pub enum Init {
    LatinHypercube,
    Random,
}
impl Default for Init {
    fn default() -> Self {
        Init::LatinHypercube
    }
}

/// Whether best updates during a generation (we use Deferred only)
#[derive(Debug, Clone, Copy)]
pub enum Updating {
    Deferred,
}
impl Default for Updating {
    fn default() -> Self {
        Updating::Deferred
    }
}

/// Linear penalty specification: lb <= A x <= ub (component-wise)
#[derive(Debug, Clone)]
pub struct LinearPenalty {
    pub a: Array2<f64>,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
    pub weight: f64,
}

/// SciPy-like linear constraint helper: lb <= A x <= ub
#[derive(Debug, Clone)]
pub struct LinearConstraintHelper {
    pub a: Array2<f64>,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
}

impl LinearConstraintHelper {
    /// Apply helper by merging into DEConfig.linear_penalty (stacking rows if already present)
    pub fn apply_to(&self, cfg: &mut DEConfig, weight: f64) {
        let new_lp = LinearPenalty {
            a: self.a.clone(),
            lb: self.lb.clone(),
            ub: self.ub.clone(),
            weight,
        };
        match &mut cfg.linear_penalty {
            Some(existing) => stack_linear_penalty(existing, &new_lp),
            None => cfg.linear_penalty = Some(new_lp),
        }
    }
}

/// SciPy-like nonlinear constraint helper: vector-valued fun(x) with lb <= fun(x) <= ub
#[derive(Clone)]
pub struct NonlinearConstraintHelper {
    pub fun: Arc<dyn Fn(&Array1<f64>) -> Array1<f64> + Send + Sync>,
    pub lb: Array1<f64>,
    pub ub: Array1<f64>,
}

impl NonlinearConstraintHelper {
    /// Apply helper by emitting penalty closures per component.
    /// lb <= f_i(x) <= ub becomes two inequalities: f_i(x)-ub <= 0 and lb - f_i(x) <= 0.
    /// If lb==ub, emit an equality penalty for f_i(x)-lb.
    pub fn apply_to(&self, cfg: &mut DEConfig, weight_ineq: f64, weight_eq: f64) {
        let f = self.fun.clone();
        let lb = self.lb.clone();
        let ub = self.ub.clone();
        let m = lb.len().min(ub.len());
        for i in 0..m {
            let l = lb[i];
            let u = ub[i];
            if (u - l).abs() < 1e-18 {
                let fi = f.clone();
                cfg.penalty_eq.push((
                    Box::new(move |x: &Array1<f64>| {
                        let y = (fi)(x);
                        y[i] - l
                    }),
                    weight_eq,
                ));
            } else {
                let fi_u = f.clone();
                cfg.penalty_ineq.push((
                    Box::new(move |x: &Array1<f64>| {
                        let y = (fi_u)(x);
                        y[i] - u
                    }),
                    weight_ineq,
                ));
                let fi_l = f.clone();
                cfg.penalty_ineq.push((
                    Box::new(move |x: &Array1<f64>| {
                        let y = (fi_l)(x);
                        l - y[i]
                    }),
                    weight_ineq,
                ));
            }
        }
    }
}

/// Configuration for the Differential Evolution optimizer
pub struct DEConfig {
    pub maxiter: usize,
    pub popsize: usize, // total NP = popsize * n_params_free
    pub tol: f64,
    pub atol: f64,
    pub mutation: Mutation,
    pub recombination: f64, // CR in [0,1]
    pub strategy: Strategy,
    pub crossover: Crossover,
    pub init: Init,
    pub updating: Updating,
    pub seed: Option<u64>,
    /// Optional integrality mask; true => variable is integer-constrained
    pub integrality: Option<Vec<bool>>,
    /// Optional initial guess used to replace the best member after init
    pub x0: Option<Array1<f64>>,
    /// Print objective best at each iteration
    pub disp: bool,
    /// Optional per-iteration callback (may stop early)
    pub callback: Option<Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send>>,
    /// Penalty-based inequality constraints: fc(x) <= 0
    pub penalty_ineq: Vec<(Box<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>, f64)>,
    /// Penalty-based equality constraints: h(x) = 0
    pub penalty_eq: Vec<(Box<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>, f64)>,
    /// Optional linear constraints treated by penalty: lb <= A x <= ub (component-wise)
    pub linear_penalty: Option<LinearPenalty>,
    /// Polishing configuration (optional)
    pub polish: Option<PolishConfig>,
}

impl Default for DEConfig {
    fn default() -> Self {
        Self {
            maxiter: 1000,
            popsize: 15,
            tol: 1e-2,
            atol: 0.0,
            mutation: Mutation::default(),
            recombination: 0.7,
            strategy: Strategy::Best1Bin,
            crossover: Crossover::default(),
            init: Init::default(),
            updating: Updating::default(),
            seed: None,
            integrality: None,
            x0: None,
            disp: false,
            callback: None,
            penalty_ineq: Vec::new(),
            penalty_eq: Vec::new(),
            linear_penalty: None,
            polish: None,
        }
    }
}

/// Fluent builder for `DEConfig` for ergonomic configuration.
pub struct DEConfigBuilder {
    cfg: DEConfig,
}
impl DEConfigBuilder {
    pub fn new() -> Self {
        Self {
            cfg: DEConfig::default(),
        }
    }
    pub fn maxiter(mut self, v: usize) -> Self {
        self.cfg.maxiter = v;
        self
    }
    pub fn popsize(mut self, v: usize) -> Self {
        self.cfg.popsize = v;
        self
    }
    pub fn tol(mut self, v: f64) -> Self {
        self.cfg.tol = v;
        self
    }
    pub fn atol(mut self, v: f64) -> Self {
        self.cfg.atol = v;
        self
    }
    pub fn mutation(mut self, v: Mutation) -> Self {
        self.cfg.mutation = v;
        self
    }
    pub fn recombination(mut self, v: f64) -> Self {
        self.cfg.recombination = v;
        self
    }
    pub fn strategy(mut self, v: Strategy) -> Self {
        self.cfg.strategy = v;
        self
    }
    pub fn crossover(mut self, v: Crossover) -> Self {
        self.cfg.crossover = v;
        self
    }
    pub fn init(mut self, v: Init) -> Self {
        self.cfg.init = v;
        self
    }
    pub fn seed(mut self, v: u64) -> Self {
        self.cfg.seed = Some(v);
        self
    }
    pub fn integrality(mut self, v: Vec<bool>) -> Self {
        self.cfg.integrality = Some(v);
        self
    }
    pub fn x0(mut self, v: Array1<f64>) -> Self {
        self.cfg.x0 = Some(v);
        self
    }
    pub fn disp(mut self, v: bool) -> Self {
        self.cfg.disp = v;
        self
    }
    pub fn callback(
        mut self,
        cb: Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send>,
    ) -> Self {
        self.cfg.callback = Some(cb);
        self
    }
    pub fn add_penalty_ineq(
        mut self,
        f: Box<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>,
        w: f64,
    ) -> Self {
        self.cfg.penalty_ineq.push((f, w));
        self
    }
    pub fn add_penalty_eq(
        mut self,
        f: Box<dyn Fn(&Array1<f64>) -> f64 + Send + Sync>,
        w: f64,
    ) -> Self {
        self.cfg.penalty_eq.push((f, w));
        self
    }
    pub fn linear_penalty(mut self, lp: LinearPenalty) -> Self {
        self.cfg.linear_penalty = Some(lp);
        self
    }
    pub fn polish(mut self, pol: PolishConfig) -> Self {
        self.cfg.polish = Some(pol);
        self
    }
    pub fn build(self) -> DEConfig {
        self.cfg
    }
}

fn mutant_rand_to_best1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    // x_r0 + F * (x_best - x_r0 + x_r1 - x_r2)
    let idxs = distinct_indices(i, 3, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    let x_r0 = pop.row(r0).to_owned();
    &x_r0
        + &((pop.row(best_idx).to_owned() - x_r0.clone() + pop.row(r1).to_owned()
            - pop.row(r2).to_owned())
            * f)
}

fn stack_linear_penalty(dst: &mut LinearPenalty, src: &LinearPenalty) {
    // Vertically stack A, lb, ub; pick max weight to enforce strongest among merged
    let a_dst = dst.a.clone();
    let a_src = src.a.clone();
    let rows = a_dst.nrows() + a_src.nrows();
    let cols = a_dst.ncols();
    assert_eq!(
        cols,
        a_src.ncols(),
        "LinearPenalty A width mismatch while stacking"
    );
    let mut a_new = Array2::<f64>::zeros((rows, cols));
    // copy
    for i in 0..a_dst.nrows() {
        for j in 0..cols {
            a_new[(i, j)] = a_dst[(i, j)];
        }
    }
    for i in 0..a_src.nrows() {
        for j in 0..cols {
            a_new[(a_dst.nrows() + i, j)] = a_src[(i, j)];
        }
    }
    let mut lb_new = Array1::<f64>::zeros(rows);
    let mut ub_new = Array1::<f64>::zeros(rows);
    for i in 0..a_dst.nrows() {
        lb_new[i] = dst.lb[i];
        ub_new[i] = dst.ub[i];
    }
    for i in 0..a_src.nrows() {
        lb_new[a_dst.nrows() + i] = src.lb[i];
        ub_new[a_dst.nrows() + i] = src.ub[i];
    }
    dst.a = a_new;
    dst.lb = lb_new;
    dst.ub = ub_new;
    dst.weight = dst.weight.max(src.weight);
}

/// Polishing configuration using NLopt local optimizer within bounds
#[derive(Debug, Clone)]
pub struct PolishConfig {
    pub enabled: bool,
    pub algo: String,   // e.g., "neldermead", "sbplx", "cobyla"
    pub maxeval: usize, // e.g., 200*n
}

/// Result/Report of a DE optimization run
#[derive(Clone)]
pub struct DEReport {
    pub x: Array1<f64>,
    pub fun: f64,
    pub success: bool,
    pub message: String,
    pub nit: usize,
    pub nfev: usize,
    pub population: Array2<f64>,
    pub population_energies: Array1<f64>,
}

impl fmt::Debug for DEReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DEReport")
            .field("x", &format!("len={}", self.x.len()))
            .field("fun", &self.fun)
            .field("success", &self.success)
            .field("message", &self.message)
            .field("nit", &self.nit)
            .field("nfev", &self.nfev)
            .field(
                "population",
                &format!("{}x{}", self.population.nrows(), self.population.ncols()),
            )
            .field(
                "population_energies",
                &format!("len={}", self.population_energies.len()),
            )
            .finish()
    }
}

/// Information passed to callback after each generation
pub struct DEIntermediate {
    pub x: Array1<f64>,
    pub fun: f64,
    pub convergence: f64, // measured as std(pop_f)
    pub iter: usize,
}

/// Action returned by callback
pub enum CallbackAction {
    Continue,
    Stop,
}

/// Differential Evolution optimizer
pub struct DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    func: &'a F,
    lower: Array1<f64>,
    upper: Array1<f64>,
    config: DEConfig,
}

impl<'a, F> DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    /// Create a new DE optimizer with objective `func` and bounds [lower, upper]
    pub fn new(func: &'a F, lower: Array1<f64>, upper: Array1<f64>) -> Self {
        assert_eq!(lower.len(), upper.len(), "lower/upper size mismatch");
        Self {
            func,
            lower,
            upper,
            config: DEConfig::default(),
        }
    }

    /// Mutable access to configuration
    pub fn config_mut(&mut self) -> &mut DEConfig {
        &mut self.config
    }

    /// Run the optimization and return a report
    pub fn solve(&mut self) -> DEReport {
        let n = self.lower.len();

        // Identify fixed (equal-bounds) and free variables
        let mut is_free: Vec<bool> = Vec::with_capacity(n);
        for i in 0..n {
            is_free.push((self.upper[i] - self.lower[i]).abs() > 0.0);
        }
        let n_free = is_free.iter().filter(|&&b| b).count();
        let _n_equal = n - n_free;
        if n_free == 0 {
            // All fixed; just evaluate x = lower
            let x_fixed = self.lower.clone();
            let mut x_eval = x_fixed.clone();
            if let Some(mask) = &self.config.integrality {
                apply_integrality(&mut x_eval, mask, &self.lower, &self.upper);
            }
            let f = (self.func)(&x_eval);
            return DEReport {
                x: x_eval,
                fun: f,
                success: true,
                message: "All variables fixed by bounds".into(),
                nit: 0,
                nfev: 1,
                population: Array2::zeros((1, n)),
                population_energies: Array1::from(vec![f]),
            };
        }

        let npop = self.config.popsize * n_free;
        let _bounds_span = &self.upper - &self.lower;
        
        eprintln!("DE Init: {} dimensions ({} free), population={}, maxiter={}", 
                  n, n_free, npop, self.config.maxiter);
        eprintln!("  Strategy: {:?}, Mutation: {:?}, Crossover: CR={:.3}",
                  self.config.strategy, self.config.mutation, self.config.recombination);
        eprintln!("  Tolerances: tol={:.2e}, atol={:.2e}", self.config.tol, self.config.atol);

        // RNG
        let mut rng: StdRng = match self.config.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_rng(&mut rand::rng()),
        };

        // Initialize population in [lower, upper]
        let mut pop = match self.config.init {
            Init::LatinHypercube => {
                eprintln!("  Using Latin Hypercube initialization");
                init_latin_hypercube(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
            Init::Random => {
                eprintln!("  Using Random initialization");
                init_random(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
        };

        // Evaluate energies (objective + penalties)
        let mut nfev: usize = 0;
        let mut energies = Array1::zeros(npop);
        eprintln!("  Evaluating initial population of {} individuals...", npop);
        for i in 0..npop {
            let xi = pop.row(i).to_owned();
            let mut x_eval = xi.clone();
            if let Some(mask) = &self.config.integrality {
                apply_integrality(&mut x_eval, mask, &self.lower, &self.upper);
            }
            let fi = self.energy(&x_eval);
            energies[i] = fi;
            nfev += 1;
        }
        
        // Report initial population statistics
        let (pop_mean, pop_std) = mean_std(&energies);
        eprintln!("  Initial population: mean={:.6e}, std={:.6e}", pop_mean, pop_std);

        // If x0 provided, override the best member
        if let Some(x0) = &self.config.x0 {
            let mut x0c = x0.clone();
            clip_inplace(&mut x0c, &self.lower, &self.upper);
            if let Some(mask) = &self.config.integrality {
                apply_integrality(&mut x0c, mask, &self.lower, &self.upper);
            }
            let f0 = self.energy(&x0c);
            nfev += 1;
            // find current best
            let (best_idx, _best_f) = argmin(&energies);
            pop.row_mut(best_idx).assign(&x0c.view());
            energies[best_idx] = f0;
        }

        let (mut best_idx, mut best_f) = argmin(&energies);
        let mut best_x = pop.row(best_idx).to_owned();
        
        eprintln!("  Initial best: fitness={:.6e} at index {}", best_f, best_idx);
        let param_summary: Vec<String> = (0..best_x.len()/3).map(|i| {
            let freq = 10f64.powf(best_x[i*3]);
            let q = best_x[i*3+1];
            let gain = best_x[i*3+2];
            format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
        }).collect();
        eprintln!("  Initial best params: [{}]", param_summary.join(", "));

        if self.config.disp {
            eprintln!("DE iter {:4}  best_f={:.6e}", 0, best_f);
        }

        // Main loop
        let mut success = false;
        let mut message = String::new();
        let mut nit = 0;
        let mut accepted_trials = 0;
        let mut improvement_count = 0;
        
        for iter in 1..=self.config.maxiter {
            nit = iter;
            accepted_trials = 0;
            improvement_count = 0;

            // For deferred updating we build a new population in place
            for i in 0..npop {
                // Mutation factor for this candidate (dithering if needed)
                let f = self.config.mutation.sample(&mut rng);

                // Build mutant and pick crossover type from strategy when explicit, else from config
                let (mutant, cross) = match self.config.strategy {
                    Strategy::Best1Bin => (
                        mutant_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Binomial,
                    ),
                    Strategy::Best1Exp => (
                        mutant_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Exponential,
                    ),
                    Strategy::Rand1Bin => (mutant_rand1(i, &pop, f, &mut rng), Crossover::Binomial),
                    Strategy::Rand1Exp => {
                        (mutant_rand1(i, &pop, f, &mut rng), Crossover::Exponential)
                    }
                    Strategy::Rand2Bin => (mutant_rand2(i, &pop, f, &mut rng), Crossover::Binomial),
                    Strategy::Rand2Exp => {
                        (mutant_rand2(i, &pop, f, &mut rng), Crossover::Exponential)
                    }
                    Strategy::CurrentToBest1Bin => (
                        mutant_current_to_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Binomial,
                    ),
                    Strategy::CurrentToBest1Exp => (
                        mutant_current_to_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Exponential,
                    ),
                    Strategy::Best2Bin => (
                        mutant_best2(i, &pop, best_idx, f, &mut rng),
                        Crossover::Binomial,
                    ),
                    Strategy::Best2Exp => (
                        mutant_best2(i, &pop, best_idx, f, &mut rng),
                        Crossover::Exponential,
                    ),
                    Strategy::RandToBest1Bin => (
                        mutant_rand_to_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Binomial,
                    ),
                    Strategy::RandToBest1Exp => (
                        mutant_rand_to_best1(i, &pop, best_idx, f, &mut rng),
                        Crossover::Exponential,
                    ),
                };

                // If strategy didn't dictate crossover, fallback to config
                let crossover = cross;
                let trial = match crossover {
                    Crossover::Binomial => binomial_crossover(
                        &pop.row(i).to_owned(),
                        &mutant,
                        self.config.recombination,
                        &mut rng,
                    ),
                    Crossover::Exponential => exponential_crossover(
                        &pop.row(i).to_owned(),
                        &mutant,
                        self.config.recombination,
                        &mut rng,
                    ),
                };

                // Respect bounds and integrality
                let mut trial_clipped = trial;
                clip_free_inplace(&mut trial_clipped, &self.lower, &self.upper, &is_free);
                if let Some(mask) = &self.config.integrality {
                    apply_integrality(&mut trial_clipped, mask, &self.lower, &self.upper);
                }

                let ft = self.energy(&trial_clipped);
                nfev += 1;

                // Selection
                if ft <= energies[i] {
                    accepted_trials += 1;
                    let old_energy = energies[i];
                    pop.row_mut(i).assign(&trial_clipped.view());
                    energies[i] = ft;
                    
                    if ft < old_energy {
                        improvement_count += 1;
                    }

                    // Track best for next iterations
                    match ft.partial_cmp(&best_f).unwrap_or(Ordering::Greater) {
                        Ordering::Less => {
                            let old_best = best_f;
                            best_f = ft;
                            best_idx = i;
                            best_x = pop.row(i).to_owned();
                            
                            // Log significant improvements
                            if iter % 10 == 0 || old_best - ft > 0.01 {
                                eprintln!("  --> NEW BEST at iter {}: {:.6e} (improvement: {:.3e})", 
                                         iter, ft, old_best - ft);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Convergence check using std/mean of energies
            let (e_mean, e_std) = mean_std(&energies);
            let conv_threshold = self.config.atol + self.config.tol * e_mean.abs();
            let conv = e_std <= conv_threshold;
            
            // More detailed convergence logging every 10 iterations
            if iter % 10 == 0 || conv {
                eprintln!("DE iter {:4}  best_f={:.6e}  mean={:.6e}  std={:.6e}  conv_thresh={:.6e} conv={}",
                         iter, best_f, e_mean, e_std, conv_threshold, conv);
                eprintln!("  --> Trials: {}/{} accepted ({:.1}%), {} improvements",
                         accepted_trials, npop, (accepted_trials as f64 / npop as f64) * 100.0, improvement_count);
            }
            
            if self.config.disp {
                eprintln!(
                    "DE iter {:4}  best_f={:.6e}  mean={:.6e}  std={:.6e}",
                    iter, best_f, e_mean, e_std
                );
            }
            // Optional callback
            if let Some(cb) = self.config.callback.as_mut() {
                let intermediate = DEIntermediate {
                    x: best_x.clone(),
                    fun: best_f,
                    convergence: e_std,
                    iter,
                };
                match (cb)(&intermediate) {
                    CallbackAction::Continue => {}
                    CallbackAction::Stop => {
                        return self.finish_report(
                            pop,
                            energies,
                            best_x,
                            best_f,
                            true,
                            "Stopped by callback".into(),
                            iter,
                            nfev,
                        );
                    }
                }
            }
            if conv {
                success = true;
                message = "Converged (stdev <= atol + tol*|mean|)".into();
                break;
            }
        }

        if !success {
            message = "Maximum iterations reached".into();
        }

        // Optional polishing step using NLopt within bounds on augmented objective
        let (best_x, best_f, nfev, message, success) = if let Some(pol) = &self.config.polish {
            if pol.enabled {
                let (xpol, fpol, evals) = self.polish(&best_x);
                let msg = if fpol < best_f {
                    String::from("Polished to lower objective")
                } else {
                    message
                };
                (xpol, fpol.min(best_f), nfev + evals, msg, success)
            } else {
                (best_x, best_f, nfev, message, success)
            }
        } else {
            (best_x, best_f, nfev, message, success)
        };

        self.finish_report(pop, energies, best_x, best_f, success, message, nit, nfev)
    }
}

// ------------------------------ Utilities ------------------------------

fn clip_inplace(x: &mut Array1<f64>, lower: &Array1<f64>, upper: &Array1<f64>) {
    for i in 0..x.len() {
        if x[i] < lower[i] {
            x[i] = lower[i];
        }
        if x[i] > upper[i] {
            x[i] = upper[i];
        }
    }
}

fn clip_free_inplace(
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

fn apply_integrality(x: &mut Array1<f64>, mask: &[bool], lower: &Array1<f64>, upper: &Array1<f64>) {
    for i in 0..x.len() {
        if i < mask.len() && mask[i] {
            x[i] = x[i].round();
            if x[i] < lower[i] {
                x[i] = lower[i].ceil();
            }
            if x[i] > upper[i] {
                x[i] = upper[i].floor();
            }
        }
    }
}

fn mean_std(v: &Array1<f64>) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.sum() / n;
    let mut var = 0.0;
    for &x in v.iter() {
        var += (x - mean) * (x - mean);
    }
    var /= n.max(1.0);
    (mean, var.sqrt())
}

fn argmin(v: &Array1<f64>) -> (usize, f64) {
    let mut best_i = 0usize;
    let mut best_v = v[0];
    for (i, &val) in v.iter().enumerate() {
        if val < best_v {
            best_v = val;
            best_i = i;
        }
    }
    (best_i, best_v)
}

fn init_random<R: Rng + ?Sized>(
    n: usize,
    npop: usize,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    is_free: &[bool],
    rng: &mut R,
) -> Array2<f64> {
    let mut pop = Array2::<f64>::zeros((npop, n));
    for i in 0..npop {
        for j in 0..n {
            if is_free[j] {
                let u: f64 = rng.r#random();
                pop[(i, j)] = lower[j] + u * (upper[j] - lower[j]);
            } else {
                pop[(i, j)] = lower[j];
            }
        }
    }
    pop
}

fn init_latin_hypercube<R: Rng + ?Sized>(
    n: usize,
    npop: usize,
    lower: &Array1<f64>,
    upper: &Array1<f64>,
    is_free: &[bool],
    rng: &mut R,
) -> Array2<f64> {
    let mut samples = Array2::<f64>::zeros((npop, n));
    // For each dimension, create stratified samples and permute
    for j in 0..n {
        if !is_free[j] {
            // fixed variable
            for i in 0..npop {
                samples[(i, j)] = 0.0;
            }
            continue;
        }
        let mut vals = Vec::with_capacity(npop);
        for k in 0..npop {
            let u: f64 = rng.r#random();
            vals.push(((k as f64) + u) / (npop as f64));
        }
        vals.shuffle(rng);
        for i in 0..npop {
            samples[(i, j)] = vals[i];
        }
    }
    // Scale to [lower, upper]
    for i in 0..npop {
        for j in 0..n {
            if is_free[j] {
                samples[(i, j)] = lower[j] + samples[(i, j)] * (upper[j] - lower[j]);
            } else {
                samples[(i, j)] = lower[j];
            }
        }
    }
    samples
}

fn distinct_indices<R: Rng + ?Sized>(
    exclude: usize,
    count: usize,
    pool_size: usize,
    rng: &mut R,
) -> Vec<usize> {
    debug_assert!(count <= pool_size.saturating_sub(1));
    // Generate a shuffled pool and take first `count` not equal to exclude
    let mut idxs: Vec<usize> = (0..pool_size).collect();
    idxs.shuffle(rng);
    let mut out = Vec::with_capacity(count);
    for idx in idxs.into_iter() {
        if idx == exclude {
            continue;
        }
        out.push(idx);
        if out.len() == count {
            break;
        }
    }
    out
}

fn mutant_best1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let _n = pop.ncols();
    let idxs = distinct_indices(i, 2, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    &pop.row(best_idx).to_owned() + &(pop.row(r0).to_owned() - pop.row(r1).to_owned()) * f
}

fn mutant_rand1<R: Rng + ?Sized>(i: usize, pop: &Array2<f64>, f: f64, rng: &mut R) -> Array1<f64> {
    let idxs = distinct_indices(i, 3, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    &pop.row(r0).to_owned() + &(pop.row(r1).to_owned() - pop.row(r2).to_owned()) * f
}

fn mutant_rand2<R: Rng + ?Sized>(i: usize, pop: &Array2<f64>, f: f64, rng: &mut R) -> Array1<f64> {
    let idxs = distinct_indices(i, 5, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    let r3 = idxs[3];
    let r4 = idxs[4];
    &pop.row(r0).to_owned()
        + &((pop.row(r1).to_owned() + pop.row(r2).to_owned()
            - pop.row(r3).to_owned()
            - pop.row(r4).to_owned())
            * f)
}

fn mutant_current_to_best1<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let idxs = distinct_indices(i, 2, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    pop.row(i).to_owned()
        + &((&pop.row(best_idx).to_owned() - pop.row(i).to_owned())
            + (pop.row(r0).to_owned() - pop.row(r1).to_owned()))
            * f
}

fn mutant_best2<R: Rng + ?Sized>(
    i: usize,
    pop: &Array2<f64>,
    best_idx: usize,
    f: f64,
    rng: &mut R,
) -> Array1<f64> {
    let idxs = distinct_indices(i, 4, pop.nrows(), rng);
    let r0 = idxs[0];
    let r1 = idxs[1];
    let r2 = idxs[2];
    let r3 = idxs[3];
    &pop.row(best_idx).to_owned()
        + &((pop.row(r0).to_owned() + pop.row(r1).to_owned()
            - pop.row(r2).to_owned()
            - pop.row(r3).to_owned())
            * f)
}

fn binomial_crossover<R: Rng + ?Sized>(
    target: &Array1<f64>,
    mutant: &Array1<f64>,
    cr: f64,
    rng: &mut R,
) -> Array1<f64> {
    let n = target.len();
    let jrand = rng.random_range(0..n);
    let mut trial = target.clone();
    for j in 0..n {
        if j == jrand || rng.r#random::<f64>() < cr {
            trial[j] = mutant[j];
        }
    }
    trial
}

fn exponential_crossover<R: Rng + ?Sized>(
    target: &Array1<f64>,
    mutant: &Array1<f64>,
    cr: f64,
    rng: &mut R,
) -> Array1<f64> {
    let n = target.len();
    let mut trial = target.clone();
    let mut j = rng.random_range(0..n);
    let mut l = 0usize;
    // ensure at least one parameter from mutant
    loop {
        trial[j] = mutant[j];
        l += 1;
        j = (j + 1) % n;
        if rng.r#random::<f64>() >= cr || l >= n {
            break;
        }
    }
    trial
}

/// Convenience function mirroring SciPy's API shape (simplified):
/// - `func`: objective function mapping x -> f(x)
/// - `bounds`: vector of (lower, upper) pairs
/// - `config`: DE configuration
pub fn differential_evolution<F>(func: &F, bounds: &[(f64, f64)], config: DEConfig) -> DEReport
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let n = bounds.len();
    let mut lower = Array1::<f64>::zeros(n);
    let mut upper = Array1::<f64>::zeros(n);
    for (i, (lo, hi)) in bounds.iter().enumerate() {
        lower[i] = *lo;
        upper[i] = *hi;
        assert!(hi >= lo, "bound[{}] has upper < lower", i);
    }
    let mut de = DifferentialEvolution::new(func, lower, upper);
    *de.config_mut() = config;
    de.solve()
}

// ------------------------------ Internal helpers ------------------------------

impl<'a, F> DifferentialEvolution<'a, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    fn energy(&self, x: &Array1<f64>) -> f64 {
        let base = (self.func)(x);
        base + self.penalty(x)
    }

    fn penalty(&self, x: &Array1<f64>) -> f64 {
        let mut p = 0.0;
        // Nonlinear ineq: fc(x) <= 0 feasible
        for (f, w) in &self.config.penalty_ineq {
            let v = f(x);
            let viol = v.max(0.0);
            p += w * viol * viol;
        }
        // Nonlinear eq: h(x) = 0
        for (h, w) in &self.config.penalty_eq {
            let v = h(x);
            p += w * v * v;
        }
        // Linear penalties: lb <= A x <= ub
        if let Some(lp) = &self.config.linear_penalty {
            let ax = lp.a.dot(&x.view());
            for i in 0..ax.len() {
                let v = ax[i];
                let lo = lp.lb[i];
                let hi = lp.ub[i];
                if v < lo {
                    let d = lo - v;
                    p += lp.weight * d * d;
                }
                if v > hi {
                    let d = v - hi;
                    p += lp.weight * d * d;
                }
            }
        }
        p
    }

    fn finish_report(
        &self,
        pop: Array2<f64>,
        energies: Array1<f64>,
        x: Array1<f64>,
        fun: f64,
        success: bool,
        message: String,
        nit: usize,
        nfev: usize,
    ) -> DEReport {
        DEReport {
            x,
            fun,
            success,
            message,
            nit,
            nfev,
            population: pop,
            population_energies: energies,
        }
    }

    fn polish(&self, x0: &Array1<f64>) -> (Array1<f64>, f64, usize) {
        // Prepare NLopt with local algorithm within bounds minimizing augmented objective
        let mut pen_ineq: Vec<(&(dyn Fn(&Array1<f64>) -> f64 + Send + Sync), f64)> = Vec::new();
        for (f, w) in &self.config.penalty_ineq {
            pen_ineq.push((f.as_ref(), *w));
        }
        let mut pen_eq: Vec<(&(dyn Fn(&Array1<f64>) -> f64 + Send + Sync), f64)> = Vec::new();
        for (h, w) in &self.config.penalty_eq {
            pen_eq.push((h.as_ref(), *w));
        }

        let pol_data = PolData {
            func: self.func,
            lower: self.lower.clone(),
            upper: self.upper.clone(),
            integrality: self.config.integrality.clone(),
            penalty_ineq: pen_ineq,
            penalty_eq: pen_eq,
            linear: self.config.linear_penalty.as_ref().map(|lp| LinearPenalty {
                a: lp.a.clone(),
                lb: lp.lb.clone(),
                ub: lp.ub.clone(),
                weight: lp.weight,
            }),
        };
        let mut opt = Nlopt::new(
            parse_local_algo(
                self.config
                    .polish
                    .as_ref()
                    .map(|p| p.algo.as_str())
                    .unwrap_or("neldermead"),
            ),
            x0.len(),
            nlopt_obj_wrapper::<F>,
            NlTarget::Minimize,
            pol_data,
        );
        let _ = opt.set_lower_bounds(self.lower.as_slice().unwrap());
        let _ = opt.set_upper_bounds(self.upper.as_slice().unwrap());
        let maxeval = self
            .config
            .polish
            .as_ref()
            .map(|p| p.maxeval)
            .unwrap_or(200 * x0.len());
        let _ = opt.set_maxeval(maxeval as u32);
        let _ = opt.set_ftol_rel(1e-9);
        let _ = opt.set_xtol_rel(1e-8);
        let mut x = x0.to_vec();
        let result = opt.optimize(&mut x);
        let nfev = 0usize; // NLopt doesn't expose directly here
        let f = match result {
            Ok((_status, val)) => val,
            Err((_e, val)) => val,
        };
        (Array1::from(x), f, nfev)
    }
}

fn parse_local_algo(name: &str) -> NlAlgorithm {
    match name.to_lowercase().as_str() {
        "neldermead" => NlAlgorithm::Neldermead,
        "sbplx" => NlAlgorithm::Sbplx,
        "cobyla" => NlAlgorithm::Cobyla,
        _ => NlAlgorithm::Neldermead,
    }
}

// NLopt wrapper data and function
struct PolData<'c, F>
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    func: &'c F,
    lower: Array1<f64>,
    upper: Array1<f64>,
    integrality: Option<Vec<bool>>,
    penalty_ineq: Vec<(&'c (dyn Fn(&Array1<f64>) -> f64 + Send + Sync), f64)>,
    penalty_eq: Vec<(&'c (dyn Fn(&Array1<f64>) -> f64 + Send + Sync), f64)>,
    linear: Option<LinearPenalty>,
}

fn nlopt_obj_wrapper<F>(x: &[f64], _grad: Option<&mut [f64]>, data: &mut PolData<F>) -> f64
where
    F: Fn(&Array1<f64>) -> f64 + Sync,
{
    let mut xa = Array1::from(x.to_vec());
    clip_inplace(&mut xa, &data.lower, &data.upper);
    if let Some(mask) = &data.integrality {
        apply_integrality(&mut xa, mask, &data.lower, &data.upper);
    }
    let base = (data.func)(&xa);
    let mut p = 0.0;
    for (f, w) in &data.penalty_ineq {
        let v = (f)(&xa).max(0.0);
        p += w * v * v;
    }
    for (h, w) in &data.penalty_eq {
        let v = (h)(&xa);
        p += w * v * v;
    }
    if let Some(lp) = &data.linear {
        let ax = lp.a.dot(&xa.view());
        for i in 0..ax.len() {
            let v = ax[i];
            let lo = lp.lb[i];
            let hi = lp.ub[i];
            if v < lo {
                let d = lo - v;
                p += lp.weight * d * d;
            }
            if v > hi {
                let d = v - hi;
                p += lp.weight * d * d;
            }
        }
    }
    base + p
}

// Test functions have been moved to individual test files in tests/ directory
// See tests/optde_*.rs for comprehensive optimization function tests

