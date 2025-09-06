use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_cross_in_tray() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(71)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .build();
    let result = differential_evolution(&cross_in_tray, &b, c);
    // Cross-in-tray has global minimum f(x) = -2.06261 at (±1.34941, ±1.34941)
    assert!(result.fun < -2.0); // Should find solution close to global minimum
}
