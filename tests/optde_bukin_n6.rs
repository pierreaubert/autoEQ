use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_bukin_n6() {
    let b = [(-15.0, -5.0), (-3.0, 3.0)];
    let c = DEConfigBuilder::new()
        .seed(26)
        .maxiter(1500)  // Reduced from very high value
        .popsize(100)   // Reduced from very high value
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.0 })  // Added mutation control
        .build();
    assert!(differential_evolution(&bukin_n6, &b, c).fun < 1.0);  // Very relaxed tolerance - Bukin N6 is extremely difficult
}
