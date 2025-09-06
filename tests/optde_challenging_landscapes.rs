use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_trid() {
    // Test 2D Trid function
    let b = [(-4.0, 4.0), (-4.0, 4.0)]; // bounds: [-d^2, d^2]
    let c = DEConfigBuilder::new()
        .seed(76)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&trid, &b, c);
    // 2D Trid has global minimum f(x) = -2 at x = (2, 2)
    assert!(result.fun < -1.8);
}

#[test]
fn test_de_bent_cigar() {
    // Test Bent Cigar function (ill-conditioned)
    let b5 = vec![(-100.0, 100.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(77)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&bent_cigar, &b5, c5);
    // Global minimum at origin, very ill-conditioned
    assert!(result.fun < 1e3); // Relaxed due to ill-conditioning
}

#[test]
fn test_de_sum_of_different_powers() {
    // Test Sum of Different Powers
    let b = vec![(-1.0, 1.0); 5];
    let c = DEConfigBuilder::new()
        .seed(78)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&sum_of_different_powers, &b, c);
    // Global minimum at origin
    assert!(result.fun < 1e-2);
}

#[test]
fn test_de_step() {
    // Test Step function (discontinuous)
    let b = vec![(-100.0, 100.0); 5];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = differential_evolution(&step, &b, c);
    // Global minimum at x = (0.5, 0.5, ..., 0.5) with f(x) = 0
    assert!(result.fun <= 5.0); // Relaxed due to discontinuous nature
}

#[test]
fn test_de_quartic() {
    // Test Quartic function (high-order polynomial)
    let b = vec![(-1.28, 1.28); 5];
    let c = DEConfigBuilder::new()
        .seed(80)
        .maxiter(1000)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&quartic, &b, c);
    // Global minimum at origin
    assert!(result.fun < 1e-3);
}

#[test]
fn test_de_salomon() {
    // Test Salomon function (multimodal)
    let b = vec![(-100.0, 100.0); 3];
    let c = DEConfigBuilder::new()
        .seed(81)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&salomon, &b, c);
    // Global minimum at origin with f(x) = 0
    assert!(result.fun < 1e-1); // Relaxed due to multimodal nature
}

#[test]
fn test_de_cosine_mixture() {
    // Test Cosine Mixture function
    let b = vec![(-1.0, 1.0); 4];
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&cosine_mixture, &b, c);
    // Global minimum depends on dimension
    assert!(result.fun < 0.1);
}
