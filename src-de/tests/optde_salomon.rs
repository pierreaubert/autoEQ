use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::salomon;

extern crate blas_src;
#[test]
fn test_de_salomon_2d() {
    // Test 2D Salomon function (multimodal)
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(81)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&salomon, &b, c);
    // Global minimum at origin with f(x) = 0
    assert!(result.fun < 1e-2, "Function value too high: {}", result.fun); // Relaxed due to multimodal nature
}

#[test]
fn test_de_salomon_3d() {
    // Test 3D Salomon function (multimodal)
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
    assert!(result.fun < 1e-1, "Function value too high: {}", result.fun); // Relaxed due to multimodal nature
}

#[test]
fn test_de_salomon_5d() {
    // Test 5D Salomon function
    let b = vec![(-50.0, 50.0); 5]; // Smaller bounds for higher dimensions
    let c = DEConfigBuilder::new()
        .seed(82)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&salomon, &b, c);
    // Global minimum at origin, but multimodal makes it challenging
    assert!(result.fun < 5e-1, "Function value too high: {}", result.fun);
}

#[test]
fn test_salomon_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0]);
    let f_origin = salomon(&x_origin);
    // f(0) = 1 - cos(2π*0) + 0.1*0 = 1 - 1 + 0 = 0
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the function structure: 1 - cos(2π*||x||) + 0.1*||x||
    let x_test = Array1::from(vec![1.0, 0.0]);
    let f_test = salomon(&x_test);
    let norm = 1.0;
    let expected = 1.0 - (2.0 * std::f64::consts::PI * norm).cos() + 0.1 * norm;
    assert!(
        (f_test - expected).abs() < 1e-15,
        "Function calculation incorrect: {} vs {}",
        f_test,
        expected
    );

    // Test multimodal nature - there should be local minima at multiples where cos term = 1
    // At norm = 1, cos(2π) = 1, so f = 1 - 1 + 0.1 = 0.1
    // At norm = 2, cos(4π) = 1, so f = 1 - 1 + 0.2 = 0.2
    let x_norm1 = Array1::from(vec![1.0, 0.0]);
    let f_norm1 = salomon(&x_norm1);
    assert!(
        (f_norm1 - 0.1).abs() < 1e-10,
        "f at norm=1 should be 0.1: {}",
        f_norm1
    );

    let x_norm2 = Array1::from(vec![2.0, 0.0]);
    let f_norm2 = salomon(&x_norm2);
    assert!(
        (f_norm2 - 0.2).abs() < 1e-10,
        "f at norm=2 should be 0.2: {}",
        f_norm2
    );

    // The global minimum should be better than local minima
    assert!(
        f_origin < f_norm1,
        "Global minimum should be better than local minima"
    );
    assert!(f_norm1 < f_norm2, "Closer local minima should be better");
}

#[test]
fn test_de_salomon_recorded() {
    // Test 2D Salomon function with recording
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(81)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    
    let result = run_recorded_differential_evolution(
        "salomon", salomon, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.5); // Relaxed threshold for multimodal salomon
    
    // Global minimum at origin (0, 0)
    assert!(report.x[0].abs() < 10.0, "x[0] should be reasonably close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 10.0, "x[1] should be reasonably close to 0.0: {}", report.x[1]);
}
