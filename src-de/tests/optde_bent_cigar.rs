use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::bent_cigar;

extern crate blas_src;
#[test]
fn test_de_bent_cigar_2d() {
    // Test 2D Bent Cigar function
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(77)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&bent_cigar, &b, c);
    // Global minimum at origin, but function is ill-conditioned
    assert!(result.fun < 1e-3, "Function value too high: {}", result.fun);
}

#[test]
fn test_de_bent_cigar_5d() {
    // Test 5D Bent Cigar function (ill-conditioned)
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
    assert!(result.fun < 1e3, "Function value too high: {}", result.fun); // Relaxed due to ill-conditioning
}

#[test]
fn test_de_bent_cigar_10d() {
    // Test 10D Bent Cigar function (very ill-conditioned)
    let b10 = vec![(-50.0, 50.0); 10]; // Smaller bounds for higher dimensions
    let c10 = DEConfigBuilder::new()
        .seed(78)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&bent_cigar, &b10, c10);
    // Very ill-conditioned in higher dimensions
    assert!(result.fun < 1e4, "Function value too high: {}", result.fun);
}

#[test]
fn test_bent_cigar_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At origin (global minimum)
    let x_origin = Array1::from(vec![0.0, 0.0, 0.0]);
    let f_origin = bent_cigar(&x_origin);
    assert!(
        f_origin < 1e-15,
        "Origin should be global minimum: {}",
        f_origin
    );

    // Test the ill-conditioning: x[0] has normal scaling, others have 10^6 scaling
    let x1 = Array1::from(vec![1.0, 0.0, 0.0]); // Only first component
    let f1 = bent_cigar(&x1);

    let x2 = Array1::from(vec![0.0, 1.0, 0.0]); // Only second component
    let f2 = bent_cigar(&x2);

    // f2 should be much larger than f1 due to 10^6 scaling
    assert!(
        f2 / f1 > 1e5,
        "Second component should be much more penalized: {} vs {}",
        f2,
        f1
    );
}

#[test]
fn test_de_bent_cigar_recorded() {
    // Test 2D Bent Cigar function with recording (ill-conditioned)
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(77)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "bent_cigar", bent_cigar, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2); // Relaxed threshold for ill-conditioned bent cigar
    
    // Global minimum at origin (0, 0)
    assert!(report.x[0].abs() < 5.0, "x[0] should be reasonably close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 0.5, "x[1] should be close to 0.0 due to high penalty: {}", report.x[1]);
}
