use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_rastrigin_2d() {
    // Test 2D Rastrigin
    let b2 = vec![(-5.12, 5.12), (-5.12, 5.12)];
    let c2 = DEConfigBuilder::new()
        .seed(40)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&rastrigin, &b2, c2).fun < 1e-2);
}

#[test]
fn test_de_rastrigin_5d() {
    // Test 5D Rastrigin
    let b5 = vec![(-5.12, 5.12); 5];
    let c5 = DEConfigBuilder::new()
        .seed(41)
        .maxiter(1500)
        .popsize(75)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&rastrigin, &b5, c5).fun < 1e-1);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_rastrigin_function() {
    let bounds = create_bounds(3, -5.12, 5.12);
    let result = auto_de(rastrigin, &bounds, None);
    
    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();
    
    // Rastrigin is highly multimodal, so we allow larger tolerance
    assert!(f_opt < 1e-1, "Rastrigin function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-1, "Solution component too far from 0: {}", xi);
    }
}
