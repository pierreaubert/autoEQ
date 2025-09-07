use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::zakharov2;

extern crate blas_src;

#[test]
fn test_de_zakharov2() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(22)
        .maxiter(300)
        .popsize(25)
        .build();
    assert!(differential_evolution(&zakharov2, &b, c).fun < 1e-4);
}

#[test]
fn test_de_zakharov2_recorded() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(22)
        .maxiter(300)
        .popsize(25)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "zakharov2", zakharov2, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-3); // Relaxed threshold for zakharov2
    
    // Zakharov has global minimum at (0, 0)
    assert!(report.x[0].abs() < 0.5, "x[0] should be close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 0.5, "x[1] should be close to 0.0: {}", report.x[1]);
}
