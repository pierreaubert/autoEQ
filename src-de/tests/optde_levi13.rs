use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy};
use autoeq_testfunctions::{levi13, levy_n13};

extern crate blas_src;
#[test]
fn test_de_levi13_basic() {
    // Test Lévy N.13 function using basic DE config
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let mut c = DEConfig::default();
    c.seed = Some(12);
    c.maxiter = 600;
    c.popsize = 25;
    assert!(differential_evolution(&levi13, &b, c).fun < 1e-3);
}

#[test]
fn test_de_levi13_advanced() {
    // Test Lévy N.13 function with advanced parameters
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
    assert!(result.fun < 1e-2, "Function value too high: {}", result.fun);
    // Check solution is close to (1, 1)
    assert!(
        (result.x[0] - 1.0).abs() < 0.1,
        "x[0] should be close to 1: {}",
        result.x[0]
    );
    assert!(
        (result.x[1] - 1.0).abs() < 0.1,
        "x[1] should be close to 1: {}",
        result.x[1]
    );
}

#[test]
fn test_de_levi13_multistart() {
    // Test with multiple random starts to verify robustness
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let seeds = [42, 123, 456];

    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1000)
            .popsize(60)
            .strategy(Strategy::RandToBest1Exp)
            .recombination(0.95)
            .build();
        let result = differential_evolution(&levy_n13, &b, c);
        assert!(
            result.fun < 5e-2,
            "Run {} (seed {}) failed: f = {}",
            i,
            seed,
            result.fun
        );
    }
}

#[test]
fn test_levi13_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (1, 1)
    let x_opt = Array1::from(vec![1.0, 1.0]);
    let f_opt = levy_n13(&x_opt);
    // Should be very close to 0
    assert!(f_opt < 1e-10, "Global optimum should be near 0: {}", f_opt);

    // Test the function structure
    // levy_n13 is same as levi13 - they should be identical
    let x_test = Array1::from(vec![0.5, -0.5]);
    let f_levy = levy_n13(&x_test);
    let f_levi = levi13(&x_test);
    assert!(
        (f_levy - f_levi).abs() < 1e-15,
        "levy_n13 and levi13 should be identical: {} vs {}",
        f_levy,
        f_levi
    );

    // Test at origin - should be worse than optimum
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = levy_n13(&x_origin);
    assert!(
        f_origin > f_opt,
        "Origin should be worse than optimum: {} vs {}",
        f_origin,
        f_opt
    );
}
