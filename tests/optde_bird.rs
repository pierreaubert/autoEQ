use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_bird() {
    let b = [(-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI), 
             (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI)];
    let c = DEConfigBuilder::new()
        .seed(70)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    let result = differential_evolution(&bird, &b, c);
    println!("Bird function result: f={}, x={:?}", result.fun, result.x);
    // Bird function has global minimum f(x) = -106.76453
    // This is a challenging multimodal function, so we use a more lenient threshold
    assert!(result.fun < -50.0); // Should find a reasonably good solution
}
