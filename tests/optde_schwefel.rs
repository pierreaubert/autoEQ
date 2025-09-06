use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_schwefel_2d() {
    // Test 2D Schwefel
    let b2 = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let c2 = DEConfigBuilder::new()
        .seed(46)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&schwefel, &b2, c2).fun < 1e-2);
}

#[test]
fn test_de_schwefel_5d() {
    // Test 5D Schwefel
    let b5 = vec![(-500.0, 500.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(47)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    assert!(differential_evolution(&schwefel, &b5, c5).fun < 1e-1);
}
