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
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use ndarray::{Array1, Array2};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

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

        if self.config.disp {
            eprintln!(
                "DE Init: {} dimensions ({} free), population={}, maxiter={}",
                n, n_free, npop, self.config.maxiter
            );
            eprintln!(
                "  Strategy: {:?}, Mutation: {:?}, Crossover: CR={:.3}",
                self.config.strategy, self.config.mutation, self.config.recombination
            );
            eprintln!(
                "  Tolerances: tol={:.2e}, atol={:.2e}",
                self.config.tol, self.config.atol
            );
        }

        // RNG
        let mut rng: StdRng = match self.config.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => {
                let mut thread_rng = rand::rng();
                StdRng::from_rng(&mut thread_rng)
            }
        };

        // Initialize population in [lower, upper]
        let mut pop = match self.config.init {
            Init::LatinHypercube => {
                if self.config.disp {
                    eprintln!("  Using Latin Hypercube initialization");
                }
                init_latin_hypercube(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
            Init::Random => {
                if self.config.disp {
                    eprintln!("  Using Random initialization");
                }
                init_random(n, npop, &self.lower, &self.upper, &is_free, &mut rng)
            }
        };

        // Evaluate energies (objective + penalties)
        let mut nfev: usize = 0;
        let mut energies = Array1::zeros(npop);
        if self.config.disp {
            eprintln!("  Evaluating initial population of {} individuals...", npop);
        }
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
        if self.config.disp {
            eprintln!(
                "  Initial population: mean={:.6e}, std={:.6e}",
                pop_mean, pop_std
            );
        }

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

        if self.config.disp {
            eprintln!(
                "  Initial best: fitness={:.6e} at index {}",
                best_f, best_idx
            );
            let param_summary: Vec<String> = (0..best_x.len() / 3)
                .map(|i| {
                    let freq = 10f64.powf(best_x[i * 3]);
                    let q = best_x[i * 3 + 1];
                    let gain = best_x[i * 3 + 2];
                    format!("f{:.0}Hz/Q{:.2}/G{:.2}dB", freq, q, gain)
                })
                .collect();
            eprintln!("  Initial best params: [{}]", param_summary.join(", "));
        }

        if self.config.disp {
            eprintln!("DE iter {:4}  best_f={:.6e}", 0, best_f);
        }

        // Main loop
        let mut success = false;
        let mut message = String::new();
        let mut nit = 0;
        let mut accepted_trials;
        let mut improvement_count;

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
                            if self.config.disp && (iter % 10 == 0 || old_best - ft > 0.01) {
                                eprintln!(
                                    "  --> NEW BEST at iter {}: {:.6e} (improvement: {:.3e})",
                                    iter,
                                    ft,
                                    old_best - ft
                                );
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
            if self.config.disp && (iter % 10 == 0 || conv) {
                eprintln!("DE iter {:4}  best_f={:.6e}  mean={:.6e}  std={:.6e}  conv_thresh={:.6e} conv={}",
                         iter, best_f, e_mean, e_std, conv_threshold, conv);
                eprintln!(
                    "  --> Trials: {}/{} accepted ({:.1}%), {} improvements",
                    accepted_trials,
                    npop,
                    (accepted_trials as f64 / npop as f64) * 100.0,
                    improvement_count
                );
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
                let u: f64 = rng.random::<f64>();
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
            let u: f64 = rng.random::<f64>();
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
        if j == jrand || rng.random::<f64>() < cr {
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
        if rng.random::<f64>() >= cr || l >= n {
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
        // Simple polish: just return the input solution (polishing disabled)
        // In a full implementation, this would use a local optimizer like Nelder-Mead
        let f = self.energy(x0);
        (x0.clone(), f, 1)
    }
}

/// Examples for using the differential evolution optimizer
pub mod examples {
    //! Example programs demonstrating differential evolution usage
}

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

/// Simplified AutoDE interface for general optimization problems
///
/// This function provides an easy-to-use interface to our differential evolution
/// implementation for general optimization problems outside of filter optimization.
///
/// # Arguments
/// * `objective` - Objective function to minimize: f(x) -> f64
/// * `bounds` - Bounds matrix (2 × n): bounds[[0, i]] = lower bound, bounds[[1, i]] = upper bound
/// * `params` - Optional parameters (uses default if None)
///
/// # Returns
/// * Some((x_opt, f_opt, iterations)) on success
/// * None on failure (invalid parameters or bounds)
///
/// # Example
/// ```ignore
/// use ndarray::Array2;
/// use autoeq::optim::{auto_de, AutoDEParams};
///
/// // Minimize f(x) = x[0]^2 + x[1]^2 subject to -5 <= x[i] <= 5
/// let quadratic = |x: &ndarray::Array1<f64>| x.iter().map(|&xi| xi * xi).sum();
/// let bounds = Array2::from_shape_vec((2, 2), vec![-5.0, -5.0, 5.0, 5.0]).unwrap();
///
/// if let Some((x_opt, f_opt, iterations)) = auto_de(quadratic, &bounds, None) {
///     println!("Found optimum: x = {:?}, f = {:.6}, iterations = {}", x_opt, f_opt, iterations);
/// }
/// ```
pub fn auto_de<F>(
    objective: F,
    bounds: &Array2<f64>,
    params: Option<AutoDEParams>,
) -> Option<(Array1<f64>, f64, usize)>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    let params = params.unwrap_or_default();

    // Validate parameters
    if params.f < 0.0 || params.f > 2.0 {
        return None; // Invalid mutation factor
    }
    if params.cr < 0.0 || params.cr > 1.0 {
        return None; // Invalid crossover probability
    }

    // Validate bounds
    if bounds.shape().len() != 2 || bounds.shape()[0] != 2 {
        return None; // Invalid bounds shape
    }

    let n_vars = bounds.shape()[1];
    if n_vars == 0 {
        return None; // Empty bounds
    }

    // Check bounds validity and convert to tuples
    let mut bounds_tuples = Vec::with_capacity(n_vars);
    for i in 0..n_vars {
        let lower = bounds[[0, i]];
        let upper = bounds[[1, i]];
        if lower > upper {
            return None; // Lower bound > upper bound
        }
        bounds_tuples.push((lower, upper));
    }

    // Set up population size
    let pop_size = params.population_size.unwrap_or_else(|| {
        // Default: 15 * dimension, with reasonable min/max
        (15 * n_vars).max(30).min(300)
    });

    // Create DE configuration using builder pattern
    let mut config_builder = DEConfigBuilder::new()
        .strategy(Strategy::RandToBest1Bin)
        .mutation(Mutation::Factor(params.f))
        .recombination(params.cr)
        .crossover(Crossover::Binomial)
        .popsize(pop_size)
        .maxiter(params.max_iterations)
        .init(Init::Random)
        .tol(params.tolerance)
        .atol(params.tolerance * 0.1);

    // Set seed if provided
    if let Some(seed) = params.seed {
        config_builder = config_builder.seed(seed);
    }

    let config = config_builder.build();
    let report = differential_evolution(&objective, &bounds_tuples, config);

    Some((report.x, report.fun, report.nit))
}

/// Records optimization progress via DE callbacks
#[derive(Debug)]
pub struct OptimizationRecorder {
    /// Function name (used for CSV filename)
    function_name: String,
    /// Shared records storage
    records: Arc<Mutex<Vec<OptimizationRecord>>>,
    /// Best function value seen so far
    best_value: Arc<Mutex<Option<f64>>>,
}

/// A single optimization iteration record
#[derive(Debug, Clone)]
pub struct OptimizationRecord {
    /// Iteration number
    pub iteration: usize,
    /// Best x found so far
    pub x: Vec<f64>,
    /// Best function result so far
    pub best_result: f64,
    /// Convergence measure (standard deviation of population)
    pub convergence: f64,
    /// Whether this iteration improved the best known result
    pub is_improvement: bool,
}

impl OptimizationRecorder {
    /// Create a new optimization recorder for the given function
    pub fn new(function_name: String) -> Self {
        Self {
            function_name,
            records: Arc::new(Mutex::new(Vec::new())),
            best_value: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a callback function that records optimization progress
    pub fn create_callback(&self) -> Box<dyn FnMut(&DEIntermediate) -> CallbackAction + Send> {
        let records = self.records.clone();
        let best_value = self.best_value.clone();

        Box::new(move |intermediate: &DEIntermediate| -> CallbackAction {
            let mut best_guard = best_value.lock().unwrap();
            let is_improvement = match *best_guard {
                Some(best) => intermediate.fun < best,
                None => true,
            };

            if is_improvement {
                *best_guard = Some(intermediate.fun);
            }
            drop(best_guard);

            // Record the iteration
            let mut records_guard = records.lock().unwrap();
            records_guard.push(OptimizationRecord {
                iteration: intermediate.iter,
                x: intermediate.x.to_vec(),
                best_result: intermediate.fun,
                convergence: intermediate.convergence,
                is_improvement,
            });
            drop(records_guard);

            CallbackAction::Continue
        })
    }

    /// Save all recorded iterations to a CSV file
    pub fn save_to_csv(&self, output_dir: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Create output directory if it doesn't exist
        create_dir_all(output_dir)?;

        let filename = format!("{}/{}.csv", output_dir, self.function_name);
        let mut file = File::create(&filename)?;

        let records_guard = self.records.lock().unwrap();

        if records_guard.is_empty() {
            return Ok(filename);
        }

        // Write CSV header
        let num_dimensions = records_guard[0].x.len();
        write!(file, "iteration,")?;
        for i in 0..num_dimensions {
            write!(file, "x{},", i)?;
        }
        writeln!(file, "best_result,convergence,is_improvement")?;

        // Write data rows
        for record in records_guard.iter() {
            write!(file, "{},", record.iteration)?;
            for &xi in &record.x {
                write!(file, "{:.16},", xi)?;
            }
            writeln!(
                file,
                "{:.16},{:.16},{}",
                record.best_result, record.convergence, record.is_improvement
            )?;
        }

        Ok(filename)
    }

    /// Get a copy of all recorded iterations
    pub fn get_records(&self) -> Vec<OptimizationRecord> {
        self.records.lock().unwrap().clone()
    }

    /// Get the number of iterations recorded
    pub fn num_iterations(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    /// Clear all recorded iterations
    pub fn clear(&self) {
        self.records.lock().unwrap().clear();
        *self.best_value.lock().unwrap() = None;
    }

    /// Get the final best solution if any iterations were recorded
    pub fn get_best_solution(&self) -> Option<(Vec<f64>, f64)> {
        let records_guard = self.records.lock().unwrap();
        if let Some(last_record) = records_guard.last() {
            Some((last_record.x.clone(), last_record.best_result))
        } else {
            None
        }
    }
}

/// Helper function for running differential evolution with recording
pub fn run_recorded_differential_evolution<F>(
    function_name: &str,
    func: F,
    bounds: &[(f64, f64)],
    mut config: DEConfig,
    output_dir: &str,
) -> Result<(DEReport, String), Box<dyn std::error::Error>>
where
    F: Fn(&Array1<f64>) -> f64 + Send + Sync,
{
    use differential_evolution;

    // Create the recorder
    let recorder = OptimizationRecorder::new(function_name.to_string());

    // Set up the callback to record progress
    config.callback = Some(recorder.create_callback());

    // Run the optimization
    let result = differential_evolution(&func, bounds, config);

    // Save the recording to CSV
    let csv_path = recorder.save_to_csv(output_dir)?;

    Ok((result, csv_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;
    use autoeq_testfunctions::quadratic;

    #[test]
    fn test_optimization_recorder() {
        let recorder = OptimizationRecorder::new("test_function".to_string());

        // Create a callback
        let mut callback = recorder.create_callback();

        // Test a few callback invocations
        let intermediate1 = DEIntermediate {
            x: Array1::from(vec![1.0, 2.0]),
            fun: 5.0,
            convergence: 0.1,
            iter: 0,
        };
        let action1 = callback(&intermediate1);
        assert!(matches!(action1, CallbackAction::Continue));

        let intermediate2 = DEIntermediate {
            x: Array1::from(vec![0.5, 1.0]),
            fun: 1.25,
            convergence: 0.05,
            iter: 1,
        };
        let action2 = callback(&intermediate2);
        assert!(matches!(action2, CallbackAction::Continue));

        // Check records
        let records = recorder.get_records();
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].iteration, 0);
        assert_eq!(records[0].x, vec![1.0, 2.0]);
        assert_eq!(records[0].best_result, 5.0);
        assert!(records[0].is_improvement);

        assert_eq!(records[1].iteration, 1);
        assert_eq!(records[1].x, vec![0.5, 1.0]);
        assert_eq!(records[1].best_result, 1.25);
        assert!(records[1].is_improvement);
    }

    #[test]
    fn test_recorded_optimization() {
        // Test recording with simple quadratic function
        let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(50) // Keep it short for testing
            .popsize(10)
            .build();

        let result = run_recorded_differential_evolution(
            "quadratic",
            quadratic,
            &bounds,
            config,
            "./data_generated/records",
        );

        assert!(result.is_ok());
        let (_de_report, csv_path) = result.unwrap();

        // Check that CSV file was created
        assert!(std::path::Path::new(&csv_path).exists());
        println!("CSV saved to: {}", csv_path);

        // Read and verify CSV content
        let csv_content = std::fs::read_to_string(&csv_path).expect("Failed to read CSV");
        let lines: Vec<&str> = csv_content.trim().split('\n').collect();

        // Should have header plus at least a few iterations
        assert!(lines.len() > 1, "CSV should have header plus data rows");

        // Check header format
        let header = lines[0];
        assert!(header.starts_with("iteration,x0,x1,best_result,convergence,is_improvement"));

        println!(
            "Recording test passed - {} iterations recorded",
            lines.len() - 1
        );

        use autoeq_testfunctions::{create_bounds, quadratic};

        extern crate blas_src;
        // Tests for auto_de parameter handling and validation

        #[test]
        fn test_auto_de_custom_parameters() {
            // Test with custom parameters
            let bounds = create_bounds(2, -5.0, 5.0);

            let params = AutoDEParams {
                max_iterations: 500,
                population_size: None, // Will use default based on dimension
                f: 0.7,                // Mutation factor
                cr: 0.8,               // Crossover probability
                tolerance: 1e-8,
                seed: Some(12345),
            };

            let result = auto_de(quadratic, &bounds, Some(params));

            assert!(
                result.is_some(),
                "AutoDE should find a solution with custom params"
            );
            let (x_opt, f_opt, iterations) = result.unwrap();

            // Should still find the optimum
            assert!(
                f_opt < 1e-6,
                "Function value too high with custom params: {}",
                f_opt
            );
            for &xi in x_opt.iter() {
                assert!(xi.abs() < 1e-3, "Solution component too far from 0: {}", xi);
            }

            // Should use specified max iterations
            assert!(
                iterations <= 500,
                "Used more iterations than specified: {}",
                iterations
            );
        }

        #[test]
        fn test_auto_de_parameter_validation() {
            let bounds = create_bounds(2, -5.0, 5.0);

            // Test invalid mutation factor
            let invalid_params = AutoDEParams {
                max_iterations: 100,
                population_size: None,
                f: 2.5, // Invalid: should be in [0, 2]
                cr: 0.5,
                tolerance: 1e-6,
                seed: None,
            };

            let result = auto_de(quadratic, &bounds, Some(invalid_params));
            assert!(result.is_none(), "Should fail with invalid mutation factor");

            // Test invalid crossover probability
            let invalid_params2 = AutoDEParams {
                max_iterations: 100,
                population_size: None,
                f: 0.5,
                cr: 1.5, // Invalid: should be in [0, 1]
                tolerance: 1e-6,
                seed: None,
            };

            let result2 = auto_de(quadratic, &bounds, Some(invalid_params2));
            assert!(
                result2.is_none(),
                "Should fail with invalid crossover probability"
            );
        }

        #[test]
        fn test_auto_de_convergence_tolerance() {
            let bounds = create_bounds(2, -5.0, 5.0);

            // Test with loose tolerance - should converge faster
            let loose_params = AutoDEParams {
                max_iterations: 1000,
                population_size: None,
                f: 0.5,
                cr: 0.7,
                tolerance: 1e-2, // Loose tolerance
                seed: Some(42),
            };

            let result = auto_de(quadratic, &bounds, Some(loose_params));
            assert!(result.is_some());
            let (_, f_opt, iterations) = result.unwrap();

            // Should converge with loose tolerance
            assert!(f_opt < 1e-2, "Function value should meet loose tolerance");

            // Test with tight tolerance
            let tight_params = AutoDEParams {
                max_iterations: 1000,
                population_size: None,
                f: 0.5,
                cr: 0.7,
                tolerance: 1e-10, // Very tight tolerance
                seed: Some(42),
            };

            let result2 = auto_de(quadratic, &bounds, Some(tight_params));
            if let Some((_, f_opt2, iterations2)) = result2 {
                // If it converges, should meet tight tolerance
                assert!(f_opt2 < 1e-8, "Function value should meet tight tolerance");
                // Might take more iterations
                assert!(
                    iterations2 >= iterations,
                    "Tight tolerance should take more iterations"
                );
            }
            // If it doesn't converge within max_iterations, that's also acceptable
        }

        #[test]
        fn test_auto_de_reproducibility() {
            // Test that same seed gives same results
            let bounds = create_bounds(3, -2.0, 2.0);

            let params = AutoDEParams {
                max_iterations: 200,
                population_size: Some(30),
                f: 0.6,
                cr: 0.8,
                tolerance: 1e-6,
                seed: Some(98765),
            };

            let result1 = auto_de(quadratic, &bounds, Some(params.clone()));
            let result2 = auto_de(quadratic, &bounds, Some(params));

            assert!(
                result1.is_some() && result2.is_some(),
                "Both runs should succeed"
            );
            let (x1, f1, iter1) = result1.unwrap();
            let (x2, f2, iter2) = result2.unwrap();

            // Same seed should give same results
            assert!(
                (f1 - f2).abs() < 1e-12,
                "Function values should be identical: {} vs {}",
                f1,
                f2
            );
            assert_eq!(iter1, iter2, "Iteration counts should be identical");
            for (i, (a, b)) in x1.iter().zip(x2.iter()).enumerate() {
                assert!(
                    (a - b).abs() < 1e-12,
                    "Solution components should be identical: x[{}] = {} vs {}",
                    i,
                    a,
                    b
                );
            }
        }

        #[test]
        fn test_auto_de_invalid_bounds() {
            use ndarray::Array2;

            // Test with invalid bounds (lower > upper)
            let mut bounds = Array2::zeros((2, 2));
            bounds[[0, 0]] = 5.0;
            bounds[[1, 0]] = 1.0; // Invalid: 5 > 1
            bounds[[0, 1]] = -1.0;
            bounds[[1, 1]] = 1.0; // Valid: -1 < 1

            let result = auto_de(quadratic, &bounds, None);
            assert!(result.is_none(), "Should fail with invalid bounds");
        }

        #[test]
        fn test_auto_de_empty_bounds() {
            use ndarray::Array2;

            // Test with empty bounds
            let bounds = Array2::zeros((2, 0));
            let result = auto_de(quadratic, &bounds, None);
            assert!(result.is_none(), "Should fail with empty bounds");
        }

        #[test]
        fn test_auto_de_default_parameters() {
            // Test that default parameters work correctly
            let bounds = create_bounds(3, -5.0, 5.0);
            let result = auto_de(quadratic, &bounds, None);

            assert!(
                result.is_some(),
                "AutoDE should work with default parameters"
            );
            let (x_opt, f_opt, _) = result.unwrap();

            assert!(
                f_opt < 1e-6,
                "Should find good solution with defaults: {}",
                f_opt
            );
            for &xi in x_opt.iter() {
                assert!(
                    xi.abs() < 1e-2,
                    "Solution component should be close to 0: {}",
                    xi
                );
            }
        }

        #[test]
        fn test_auto_de_population_size_scaling() {
            let bounds = create_bounds(2, -5.0, 5.0);

            // Test explicit small population
            let small_pop_params = AutoDEParams {
                max_iterations: 100,
                population_size: Some(10), // Small population
                f: 0.8,
                cr: 0.9,
                tolerance: 1e-6,
                seed: Some(111),
            };

            let result1 = auto_de(quadratic, &bounds, Some(small_pop_params));
            assert!(result1.is_some(), "Should work with small population");

            // Test explicit large population
            let large_pop_params = AutoDEParams {
                max_iterations: 100,
                population_size: Some(100), // Large population
                f: 0.8,
                cr: 0.9,
                tolerance: 1e-6,
                seed: Some(111),
            };

            let result2 = auto_de(quadratic, &bounds, Some(large_pop_params));
            assert!(result2.is_some(), "Should work with large population");

            // Both should find good solutions
            let (_, f1, _) = result1.unwrap();
            let (_, f2, _) = result2.unwrap();
            assert!(f1 < 1e-4 && f2 < 1e-4, "Both should find good solutions");
        }
    }
}
