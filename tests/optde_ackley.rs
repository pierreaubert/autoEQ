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
