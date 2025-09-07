use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{bohachevsky1, bohachevsky2, bohachevsky3};

extern crate blas_src;
#[test]
fn test_de_bohachevsky1() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky1, &b, c).fun < 1e-4);
}

#[test]
fn test_de_bohachevsky2() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky2, &b, c).fun < 1e-4);
}

#[test]
fn test_de_bohachevsky3() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky3, &b, c).fun < 1e-4);
}

#[test]
fn test_de_bohachevsky1_recorded() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "bohachevsky1", bohachevsky1, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-3);
    
    // Bohachevsky has global minimum at (0, 0)
    assert!(report.x[0].abs() < 0.5, "x[0] should be close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 0.5, "x[1] should be close to 0.0: {}", report.x[1]);
}
