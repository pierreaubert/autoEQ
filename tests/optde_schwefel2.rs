use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_schwefel2() {
    let b = [(-500.0, 500.0), (-500.0, 500.0)];
    let c = DEConfigBuilder::new()
        .seed(23)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&schwefel2, &b, c).fun < 1e2);
}
