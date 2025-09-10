use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy};
use autoeq_testfunctions::freudenstein_roth;

#[test]
fn test_de_freudenstein_roth_basic() {
    // Test Freudenstein and Roth function
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(84)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&freudenstein_roth, &b, c);
    // Global minimum f(x) = 0 at x = (5, 4)
    assert!(result.fun < 1e-1, "Function value too high: {}", result.fun); // Relaxed due to ill-conditioning
}

#[test]
fn test_de_freudenstein_roth_focused() {
    // Test with bounds closer to the known optimum
    let b = [(0.0, 8.0), (0.0, 8.0)]; // Focused around the optimum (5, 4)
    let c = DEConfigBuilder::new()
        .seed(85)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&freudenstein_roth, &b, c);
    // Should find better solution with focused bounds
    assert!(result.fun < 5e-2, "Function value too high: {}", result.fun);
    // Check solution is reasonably close to (5, 4)
    assert!(
        (result.x[0] - 5.0).abs() < 1.0,
        "x[0] should be close to 5: {}",
        result.x[0]
    );
    assert!(
        (result.x[1] - 4.0).abs() < 1.0,
        "x[1] should be close to 4: {}",
        result.x[1]
    );
}

#[test]
fn test_de_freudenstein_roth_multistart() {
    // Test with multiple random starts to handle multimodality
    let b = [(-5.0, 10.0), (-2.0, 8.0)];
    let seeds = [100, 200, 300];

    let mut best_result = f64::INFINITY;
    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1000)
            .popsize(60)
            .strategy(Strategy::Rand1Exp)
            .recombination(0.8)
            .build();
        let result = differential_evolution(&freudenstein_roth, &b, c);
        best_result = best_result.min(result.fun);
        println!("Run {} (seed {}): f = {}", i, seed, result.fun);
    }

    // At least one run should find a decent solution
    assert!(
        best_result < 1.0,
        "Best result across runs too high: {}",
        best_result
    );
}

#[test]
fn test_freudenstein_roth_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (5, 4)
    let x_opt = Array1::from(vec![5.0, 4.0]);
    let f_opt = freudenstein_roth(&x_opt);
    // Should be exactly 0
    assert!(f_opt < 1e-10, "Global optimum should be 0: {}", f_opt);

    // Test the function structure:
    // f(x1,x2) = (-13 + x1 + ((5-x2)*x2 - 2)*x2)^2 + (-29 + x1 + ((x2+1)*x2 - 14)*x2)^2

    // At origin (should be worse)
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = freudenstein_roth(&x_origin);
    // f(0,0) = (-13 + 0 + ((5-0)*0 - 2)*0)^2 + (-29 + 0 + ((0+1)*0 - 14)*0)^2 = (-13)^2 + (-29)^2 = 169 + 841 = 1010
    let expected_origin = (-13.0_f64).powi(2) + (-29.0_f64).powi(2);
    assert!(
        (f_origin - expected_origin).abs() < 1e-10,
        "Origin calculation incorrect: {} vs {}",
        f_origin,
        expected_origin
    );

    // The function should be much worse at origin than at optimum
    assert!(
        f_origin > f_opt + 500.0,
        "Origin should be much worse than optimum: {} vs {}",
        f_origin,
        f_opt
    );

    // Test at a different point to verify calculation
    let x_test = Array1::from(vec![1.0, 2.0]);
    let f_test = freudenstein_roth(&x_test);

    // Manual calculation for (1, 2):
    // First term: (-13 + 1 + ((5-2)*2 - 2)*2) = (-13 + 1 + (3*2 - 2)*2) = (-13 + 1 + 4*2) = (-13 + 1 + 8) = -4
    // Second term: (-29 + 1 + ((2+1)*2 - 14)*2) = (-29 + 1 + (3*2 - 14)*2) = (-29 + 1 + (-8)*2) = (-29 + 1 - 16) = -44
    // f = (-4)^2 + (-44)^2 = 16 + 1936 = 1952
    let expected_test = 16.0 + 1936.0;
    assert!(
        (f_test - expected_test).abs() < 1e-10,
        "Test point calculation incorrect: {} vs {}",
        f_test,
        expected_test
    );
}
