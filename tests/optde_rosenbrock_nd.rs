use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_rosenbrock_2d() {
    // Test 2D Rosenbrock
    let b2 = vec![(-2.048, 2.048), (-2.048, 2.048)];
    let c2 = DEConfigBuilder::new()
        .seed(48)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&rosenbrock, &b2, c2).fun < 1e-4);
}

#[test]
fn test_de_rosenbrock_10d() {
    // Test 10D Rosenbrock
    let b10 = vec![(-2.048, 2.048); 10];
    let c10 = DEConfigBuilder::new()
        .seed(49)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&rosenbrock, &b10, c10).fun < 1e-1);
}
