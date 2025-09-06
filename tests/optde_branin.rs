use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_branin() {
    let b = [(-5.0, 10.0), (0.0, 15.0)];
    let c = DEConfigBuilder::new()
        .seed(30)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&branin, &b, c).fun < 0.5);
}
