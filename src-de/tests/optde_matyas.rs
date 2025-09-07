use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::matyas;

extern crate blas_src;

#[test]
fn test_de_matyas() {
    let bounds = [(-10.0, 10.0), (-10.0, 10.0)];
    let mut config = DEConfig::default();
    config.seed = Some(5);
    config.maxiter = 800;
    config.popsize = 40;
    config.recombination = 0.9;
    config.strategy = Strategy::RandToBest1Exp;
    
    let result = differential_evolution(&matyas, &bounds, config);
    
    // Matyas function: Global minimum f(x) = 0 at x = (0, 0)
    assert!(result.fun < 1e-5);
    
    // Check that solution is close to expected optimum
    let expected = [0.0, 0.0];
    for (actual, expected) in result.x.iter().zip(expected.iter()) {
        assert!((actual - expected).abs() < 0.1, 
               "Solution component {} should be close to {}", actual, expected);
    }
}

#[test]
fn test_de_matyas_different_strategy() {
    let bounds = [(-10.0, 10.0), (-10.0, 10.0)];
    let mut config = DEConfig::default();
    config.seed = Some(123);
    config.maxiter = 500;
    config.popsize = 30;
    config.recombination = 0.7;
    config.strategy = Strategy::CurrentToBest1Bin;
    
    let result = differential_evolution(&matyas, &bounds, config);
    
    // Should still converge to global minimum
    assert!(result.fun < 1e-4);
}

#[test]
fn test_de_matyas_recorded() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(123)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "matyas", matyas, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-5);
    
    // Check that solution is close to expected optimum (0, 0)
    for &actual in report.x.iter() {
        assert!(actual.abs() < 0.1);
    }
}
