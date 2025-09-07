use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{langermann, create_bounds};

extern crate blas_src;

#[test]
fn test_de_langermann_2d() {
    // Test Langermann function in 2D - complex multimodal with parameters
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(210)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&langermann, &bounds, config);
    
    // Global minimum is approximately -5.1621 
    assert!(result.fun < -3.0, "Solution quality too low: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= 0.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
    }
}

#[test]
fn test_de_langermann_different_strategies() {
    // Test multiple strategies since this is a complex multimodal function
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    
    let strategies = [Strategy::RandToBest1Bin, Strategy::Best2Bin, Strategy::Rand1Exp];
    
    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(211 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.8)
            .build();
        
        let result = differential_evolution(&langermann, &bounds, config);
        assert!(result.fun < -1.0, "Strategy {:?} failed to find reasonable solution: {}", strategy, result.fun);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_langermann_function() {
    let bounds = create_bounds(2, 0.0, 10.0);
    let result = auto_de(langermann, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < -1.0, "Langermann function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= 0.0 && xi <= 10.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_langermann_recorded() {
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(212)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "langermann", langermann, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -1.0, "Recorded Langermann optimization failed: {}", report.fun);
    
    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= 0.0 && actual <= 10.0, "Solution out of bounds: {}", actual);
    }
}

#[test] 
fn test_langermann_known_properties() {
    // Test some properties of the Langermann function
    use ndarray::Array1;
    
    // Test that function is finite at various points within bounds
    let test_points = vec![
        vec![2.0, 1.0],     // Near one of the parameter points
        vec![5.0, 2.0],     // Near another parameter point
        vec![7.0, 9.0],     // Near the third parameter point
        vec![1.0, 4.0],     // Near the fourth parameter point
        vec![0.5, 0.5],     // Corner region
        vec![9.5, 9.5],     // Other corner
    ];
    
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = langermann(&x);
        
        assert!(f.is_finite(), "Function should be finite at {:?}: {}", point, f);
        // Langermann can have both positive and negative values
    }
    
    // Test boundary behavior
    let x_boundary = Array1::from(vec![0.0, 10.0]);
    let f_boundary = langermann(&x_boundary);
    assert!(f_boundary.is_finite(), "Function at boundary should be finite");
    
    let x_corner = Array1::from(vec![10.0, 0.0]);
    let f_corner = langermann(&x_corner);
    assert!(f_corner.is_finite(), "Function at corner should be finite");
}
