use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_ackley_2d() {
    // Test 2D Ackley
    let b2 = vec![(-32.768, 32.768), (-32.768, 32.768)];
    let c2 = DEConfigBuilder::new()
        .seed(42)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&ackley, &b2, c2).fun < 1e-3);
}

#[test]
fn test_de_ackley_10d() {
    // Test 10D Ackley
    let b10 = vec![(-32.768, 32.768); 10];
    let c10 = DEConfigBuilder::new()
        .seed(43)
        .maxiter(1200)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&ackley, &b10, c10).fun < 1e-2);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_ackley_function() {
    let bounds = create_bounds(4, -32.0, 32.0);
    let result = auto_de(ackley, &bounds, None);
    
    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();
    
    // Ackley has many local minima, so we're more lenient
    assert!(f_opt < 1e-1, "Ackley function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1.0, "Solution component too far from 0: {}", xi);
    }
}
