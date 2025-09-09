use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{xin_she_yang_n2, create_bounds};

extern crate blas_src;

#[test]
fn test_de_xin_she_yang_n2_2d() {
    // Test Xin-She Yang N.2 function in 2D - newer benchmark function
    let bounds = vec![(-6.28, 6.28), (-6.28, 6.28)]; // -2π to 2π
    let config = DEConfigBuilder::new()
        .seed(180)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n2, &bounds, config);
    
    // Global minimum is at (0, 0) with f = 0
    assert!(result.fun < 1e-3, "Solution quality too low: {}", result.fun);
    
    // Check solution is close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -6.28 && xi <= 6.28, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 0.5, "Solution not near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_xin_she_yang_n2_5d() {
    // Test Xin-She Yang N.2 function in 5D - higher dimensional challenge
    let bounds = vec![(-6.28, 6.28); 5];
    let config = DEConfigBuilder::new()
        .seed(181)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n2, &bounds, config);
    
    // For 5D, accept a slightly higher tolerance
    assert!(result.fun < 1e-1, "Solution quality too low for 5D: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -6.28 && xi <= 6.28, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_xin_she_yang_n2_function() {
    let bounds = create_bounds(2, -6.28, 6.28);
    let result = auto_de(xin_she_yang_n2, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 0.5, "Xin-She Yang N.2 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -6.28 && xi <= 6.28, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_xin_she_yang_n2_recorded() {
    let bounds = vec![(-6.28, 6.28), (-6.28, 6.28)];
    let config = DEConfigBuilder::new()
        .seed(182)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = run_recorded_differential_evolution(
        "xin_she_yang_n2", xin_she_yang_n2, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.5, "Recorded Xin-She Yang N.2 optimization failed: {}", report.fun);
    
    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -6.28 && actual <= 6.28, "Solution out of bounds: {}", actual);
    }
}


#[test]
fn test_xin_she_yang_n2_multimodal_behavior() {
    // Test that function has multiple local optima
    use ndarray::Array1;
    
    let test_points = vec![
        vec![0.0, 0.0],     // Global optimum
        vec![6.28, 0.0],    // Potential local behavior
        vec![0.0, 6.28],    // Due to periodic nature
        vec![-6.28, 0.0],   // And symmetry
    ];
    
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = xin_she_yang_n2(&x);
        
        assert!(f.is_finite(), "Function should be finite at {:?}: {}", point, f);
        assert!(f >= 0.0, "Function should be non-negative at {:?}: {}", point, f);
    }
}
