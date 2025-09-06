use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_levy_n13() {
    // Test Lévy N.13 function
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(83)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&levy_n13, &b, c);
    // Global minimum f(x) = 0 at x = (1, 1)
    assert!(result.fun < 1e-2);
    // Check solution is close to (1, 1)
    assert!((result.x[0] - 1.0).abs() < 0.1);
    assert!((result.x[1] - 1.0).abs() < 0.1);
}

#[test]
fn test_de_freudenstein_roth() {
    // Test Freudenstein and Roth function
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(84)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&freudenstein_roth, &b, c);
    // Global minimum f(x) = 0 at x = (5, 4)
    assert!(result.fun < 1e-1); // Relaxed due to ill-conditioning
}

#[test]
fn test_de_colville() {
    // Test Colville function (4D)
    let b4 = vec![(-10.0, 10.0); 4];
    let c4 = DEConfigBuilder::new()
        .seed(85)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&colville, &b4, c4);
    // Global minimum f(x) = 0 at x = (1, 1, 1, 1)
    assert!(result.fun < 1e-2);
}

#[test]
fn test_de_rotated_hyper_ellipsoid() {
    // Test rotated hyper-ellipsoid (non-separable)
    let b5 = vec![(-65.536, 65.536); 5];
    let c5 = DEConfigBuilder::new()
        .seed(86)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&rotated_hyper_ellipsoid, &b5, c5);
    // Global minimum f(x) = 0 at origin
    assert!(result.fun < 1e-3);
}

#[test]
fn test_de_ackley_n2() {
    // Test Ackley N.2 function
    let b = [(-32.0, 32.0), (-32.0, 32.0)];
    let c = DEConfigBuilder::new()
        .seed(87)
        .maxiter(1200)
        .popsize(70)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&ackley_n2, &b, c);
    // Global minimum f(x) = -200 at x = (0, 0)
    assert!(result.fun < -190.0); // Should find solution close to global minimum
}
