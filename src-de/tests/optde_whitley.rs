use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{whitley, create_bounds};

extern crate blas_src;

#[test]
fn test_de_whitley_2d() {
    // Test Whitley function in 2D - challenging multimodal function
    let bounds = vec![(-10.24, 10.24), (-10.24, 10.24)];
    let config = DEConfigBuilder::new()
        .seed(230)
        .maxiter(3000)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&whitley, &bounds, config);
    
    // Global minimum is at (1, 1) with f = 0
    assert!(result.fun < 10.0, "Solution quality too low: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -10.24 && xi <= 10.24, "Solution coordinate out of bounds: {}", xi);
    }
}

#[test]
fn test_de_whitley_5d() {
    // Test Whitley function in 5D - very challenging
    let bounds = vec![(-10.24, 10.24); 5];
    let config = DEConfigBuilder::new()
        .seed(231)
        .maxiter(4000)
        .popsize(200)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&whitley, &bounds, config);
    
    // For 5D, this is extremely challenging - accept reasonable improvements
    assert!(result.fun < 100.0, "Solution quality too low for 5D: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -10.24 && xi <= 10.24, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_whitley_function() {
    let bounds = create_bounds(2, -10.24, 10.24);
    let result = auto_de(whitley, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Whitley is extremely challenging, accept any reasonable improvement
    assert!(f_opt < 50.0, "Whitley function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -10.24 && xi <= 10.24, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_whitley_recorded() {
    let bounds = vec![(-10.24, 10.24), (-10.24, 10.24)];
    let config = DEConfigBuilder::new()
        .seed(232)
        .maxiter(3000)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "whitley", whitley, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 50.0, "Recorded Whitley optimization failed: {}", report.fun);
    
    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -10.24 && actual <= 10.24, "Solution out of bounds: {}", actual);
    }
}


#[test]
fn test_whitley_challenging_nature() {
    // Test that demonstrates the challenging nature of Whitley function
    use ndarray::Array1;
    
    // Test points away from optimum should have significantly higher values
    let test_points = vec![
        vec![0.0, 0.0],     // Should be higher than (1,1)
        vec![5.0, 5.0],     // Even higher
        vec![-3.0, -3.0],   // Also higher
    ];
    
    let x_optimum = Array1::from(vec![1.0, 1.0]);
    let f_optimum = whitley(&x_optimum);
    
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = whitley(&x);
        
        if point != vec![1.0, 1.0] {
            assert!(f >= f_optimum, "Point {:?} should have f >= optimum: {} vs {}", point, f, f_optimum);
        }
        assert!(f.is_finite(), "Function should be finite at {:?}: {}", point, f);
    }
}
