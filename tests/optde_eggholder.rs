use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_eggholder() {
    let b = [(-512.0, 512.0), (-512.0, 512.0)];
    let c = DEConfigBuilder::new()
        .seed(27)
        .maxiter(1200)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&eggholder, &b, c).fun < -700.0);
}
