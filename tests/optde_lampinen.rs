use autoeq::optde::*;
use autoeq::optim::AutoDEParams;
use common::*;

mod common;

#[test]
fn test_de_lampinen_simplified() {
    // Test Lampinen simplified using direct DE interface
    let b6 = vec![(0.0, 1.0); 6];  // Simplified bounds
    let c6 = DEConfigBuilder::new()
        .seed(50)
        .maxiter(500)
        .popsize(60)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&lampinen_simplified, &b6, c6);
    
    // For this simplified version, optimum should be at bounds
    // x[0..4] should be around 2.5 (but clamped to 1.0 by bounds)
    // x[4..] should be at 0.0
    for i in 0..4.min(result.x.len()) {
        assert!(result.x[i] > 0.5, "First 4 variables should be large: x[{}] = {}", i, result.x[i]);
    }
    for i in 4..result.x.len() {
        assert!(result.x[i] < 0.5, "Last variables should be small: x[{}] = {}", i, result.x[i]);
    }
    
    // Should reach the optimal value close to -16.0
    assert!(result.fun < -15.0, "Function value should be good: {}", result.fun);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_lampinen_simplified() {
    // Simplified version of Lampinen L1 without constraints
    // f(x) = sum(5*x[i]) - sum(x[i]^2) for i in 0..4, - sum(x[j]) for j in 4..
    
    let bounds = create_bounds(6, 0.0, 1.0);  // Simplified bounds
    let result = auto_de(lampinen_simplified, &bounds, None);
    
    assert!(result.is_some(), "Lampinen simplified should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();
    
    // For this simplified version, optimum should be at bounds
    // x[0..4] should be around 2.5 (but clamped to 1.0 by bounds)
    // x[4..] should be at 0.0
    for i in 0..4.min(x_opt.len()) {
        assert!(x_opt[i] > 0.5, "First 4 variables should be large: x[{}] = {}", i, x_opt[i]);
    }
    for i in 4..x_opt.len() {
        assert!(x_opt[i] < 0.5, "Last variables should be small: x[{}] = {}", i, x_opt[i]);
    }
    
    println!("Lampinen L1 simplified: f_opt = {:.6}", f_opt);
}

#[test]
fn test_auto_de_lampinen_with_params() {
    // Test Lampinen with specific parameters
    let bounds = create_bounds(6, 0.0, 1.0);
    
    let params = AutoDEParams {
        max_iterations: 800,
        population_size: Some(80),
        f: 0.9,
        cr: 0.95,
        tolerance: 1e-6,
        seed: Some(789),
    };
    
    let result = auto_de(lampinen_simplified, &bounds, Some(params));
    
    assert!(result.is_some(), "Lampinen with custom params should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();
    
    // Verify the solution structure
    for i in 0..4.min(x_opt.len()) {
        assert!(x_opt[i] > 0.8, "First 4 variables should be at upper bound: x[{}] = {}", i, x_opt[i]);
    }
    for i in 4..x_opt.len() {
        assert!(x_opt[i] < 0.2, "Last variables should be at lower bound: x[{}] = {}", i, x_opt[i]);
    }
    
    // Should achieve near-optimal value
    assert!(f_opt < -15.5, "Should reach near-optimal value: {}", f_opt);
    
    println!("Lampinen with custom params: f_opt = {:.6}", f_opt);
}

#[test]
fn test_lampinen_function_properties() {
    use ndarray::Array1;
    
    // Test that the function behaves as expected
    
    // At optimal solution (all first 4 variables at 1.0, last 2 at 0.0)
    let x_optimal = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    let f_optimal = lampinen_simplified(&x_optimal);
    
    // Should be -(4 * 5 * 1 - 4 * 1 + 0) = -(20 - 4) = -16
    assert!((f_optimal - (-16.0)).abs() < 1e-10, "Optimal value should be -16.0: {}", f_optimal);
    
    // Test suboptimal solution
    let x_suboptimal = Array1::from(vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
    let f_suboptimal = lampinen_simplified(&x_suboptimal);
    
    // Should be worse than optimal
    assert!(f_suboptimal > f_optimal, "Suboptimal should be worse: {} vs {}", f_suboptimal, f_optimal);
    
    println!("Lampinen function test: optimal = {:.6}, suboptimal = {:.6}", f_optimal, f_suboptimal);
}
