use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_dixons_price_2d() {
    // Test 2D Dixon's Price
    let b2 = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let c2 = DEConfigBuilder::new()
        .seed(50)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&dixons_price, &b2, c2).fun < 1e-3);
}

#[test]
fn test_de_dixons_price_10d() {
    // Test 10D Dixon's Price
    let b10 = vec![(-10.0, 10.0); 10];
    let c10 = DEConfigBuilder::new()
        .seed(51)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&dixons_price, &b10, c10).fun < 5e-2);  // Relaxed for 10D
}
