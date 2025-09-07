use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{pinter, create_bounds};

extern crate blas_src;

#[test]
fn test_de_pinter_2d() {
    // Test Pinter function in 2D - challenging multimodal function
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(140)
        .maxiter(2000)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&pinter, &bounds, config);
    
    // Global minimum is at (0, 0) with f = 0
    assert!(result.fun < 1.0, "Solution quality too low: {}", result.fun);
    
    // Check solution is reasonably close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 2.0, "Solution not reasonably near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_pinter_5d() {
    // Test Pinter function in 5D - higher dimensional challenge
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(141)
        .maxiter(3000)
        .popsize(120)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&pinter, &bounds, config);
    
    // For 5D, accept a slightly higher tolerance due to increased complexity
    assert!(result.fun < 1e-1, "Solution quality too low for 5D: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_pinter_function() {
    let bounds = create_bounds(2, -10.0, 10.0);
    let result = auto_de(pinter, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 2.0, "Pinter function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_pinter_recorded() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(142)
        .maxiter(2000)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "pinter", pinter, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 2.0, "Recorded Pinter optimization failed: {}", report.fun);
    
    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -10.0 && actual <= 10.0, "Solution out of bounds: {}", actual);
    }
}

#[test]
fn test_pinter_known_properties() {
    // Test some properties of the Pinter function
    use ndarray::Array1;
    
    // Test the known global optimum
    let x_global = Array1::from(vec![0.0, 0.0]);
    let f_global = pinter(&x_global);
    
    // Should be exactly 0 at the global optimum
    assert!(f_global.abs() < 1e-10, "Global optimum value not as expected: {}", f_global);
    
    // Test boundary behavior - should be finite
    let x_bound = Array1::from(vec![-10.0, 10.0]);
    let f_bound = pinter(&x_bound);
    assert!(f_bound.is_finite(), "Function at boundary should be finite");
    
    // Test a point away from optimum - should be positive
    let x_away = Array1::from(vec![1.0, 1.0]);
    let f_away = pinter(&x_away);
    assert!(f_away > 0.0, "Function away from optimum should be positive: {}", f_away);
}
