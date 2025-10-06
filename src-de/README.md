<!-- markdownlint-disable-file MD013 -->

# AutoEQ Differential Evolution

This crate provides a pure Rust implementation of Differential Evolution (DE) global optimization algorithm with advanced features.

## Features

- **Pure Rust Implementation**: No external dependencies for core optimization
- **Multiple DE Strategies**: Various mutation and crossover strategies
- **Constraint Handling**: Linear and nonlinear constraint support
- **Adaptive Parameters**: Self-adjusting F and CR parameters
- **Evaluation Recording**: Track optimization progress and convergence
- **Visualization Tools**: Plot test functions and optimization traces

## Optimization Strategies

### Mutation Strategies

- `DE/rand/1`: `x_trial = x_r1 + F * (x_r2 - x_r3)`
- `DE/best/1`: `x_trial = x_best + F * (x_r1 - x_r2)`
- `DE/current-to-best/1`: Combines current and best vectors
- `DE/rand/2`: Uses five random vectors for mutation

### Crossover Strategies

- **Binomial**: Random parameter-wise crossover
- **Exponential**: Sequential parameter crossover

## Usage

```rust
use autoeq_de::{differential_evolution, DEConfig, Strategy};

let config = DEConfig {
    bounds: bounds.clone(),
    func: my_objective_function,
    strategy: Strategy::Rand1Bin,
    max_iter: 1000,
    pop_size: 50,
    f: 0.8,
    cr: 0.9,
    seed: Some(42),
    ..Default::default()
};

let result = differential_evolution(config)?;
println!("Best solution: {:?}", result.x);
println!("Best fitness: {}", result.fx);
```

## Constraint Support

### Linear Constraints

```rust
use autoeq_de::LinearConstraint;

let constraint = LinearConstraint::new(
    vec![1.0, 1.0], // coefficients
    1.0,            // upper bound: x1 + x2 <= 1.0
    ConstraintType::LessEqual
);
```

### Nonlinear Constraints

```rust
let nonlinear_constraint = |x: &[f64]| -> f64 {
    x[0].powi(2) + x[1].powi(2) - 1.0 // circle constraint
};
```

## Visualization

The crate includes a `plot_functions` binary for visualizing test functions and optimization traces:

```bash
# Plot test functions as contour plots
cargo run --bin plot_functions -- --functions rosenbrock,sphere

# Show optimization traces from CSV files
cargo run --bin plot_functions -- --csv-dir traces/ --show-traces
```

## Integration

This crate is part of the AutoEQ ecosystem:

- Used by `autoeq` for filter parameter optimization
- Integrates with `autoeq-testfunctions` for validation
- Works with `autoeq-iir` for audio filter optimization

## Examples

The crate includes several example programs demonstrating different DE capabilities:

- `basic_de`: Simple unconstrained optimization
- `linear_constraints`: Linear constraint handling
- `nonlinear_constraints`: Complex constraint optimization

## [References](./REFERENCES.md)

## License

GPL-3.0-or-later
