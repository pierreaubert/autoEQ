use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::three_hump_camel;

extern crate blas_src;

#[test]
fn test_de_three_hump_camel() {
    let b = [(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(8);
    c.maxiter = 300;
    c.popsize = 20;
    c.strategy = Strategy::Best1Exp;
    assert!(differential_evolution(&three_hump_camel, &b, c).fun < 1e-6);
}

#[test]
fn test_de_three_hump_camel_recorded() {
    let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let config = DEConfigBuilder::new()
        .seed(8)
        .maxiter(300)
        .popsize(20)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "three_hump_camel", three_hump_camel, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-6);
    
    // Three-hump camel has global minimum at (0, 0) with f = 0
    assert!(report.x[0].abs() < 0.1, "x[0] should be close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 0.1, "x[1] should be close to 0.0: {}", report.x[1]);
}
