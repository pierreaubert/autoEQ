use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{xin_she_yang_n3, create_bounds};

extern crate blas_src;

#[test]
fn test_de_xin_she_yang_n3_2d() {
    // Test Xin-She Yang N.3 function in 2D - multimodal with parameter m
    let bounds = vec![(-20.0, 20.0), (-20.0, 20.0)];
    let config = DEConfigBuilder::new()
        .seed(190)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n3, &bounds, config);
    
    // Global minimum is at (0, 0) with f = -1
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.5, "Solution quality too low: {}", result.fun);
    
    // Check solution is close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -20.0 && xi <= 20.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 2.0, "Solution not reasonably near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_xin_she_yang_n3_5d() {
    // Test Xin-She Yang N.3 function in 5D 
    let bounds = vec![(-20.0, 20.0); 5];
    let config = DEConfigBuilder::new()
        .seed(191)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n3, &bounds, config);
    
    // For 5D, accept a higher tolerance
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.1, "Solution quality too low for 5D: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -20.0 && xi <= 20.0, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_xin_she_yang_n3_function() {
    let bounds = create_bounds(2, -20.0, 20.0);
    let result = auto_de(xin_she_yang_n3, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt > -1.01, "Solution too good (below theoretical minimum): {}", f_opt);
    assert!(f_opt < 0.0, "Xin-She Yang N.3 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -20.0 && xi <= 20.0, "Solution component out of bounds: {}", xi);
    }
}

#[test] 
fn test_xin_she_yang_n3_known_properties() {
    // Test some properties of the Xin-She Yang N.3 function
    use ndarray::Array1;
    
    // Test the known global optimum
    let x_global = Array1::from(vec![0.0, 0.0]);
    let f_global = xin_she_yang_n3(&x_global);
    
    // Should be -1 at the global optimum
    assert!((f_global + 1.0).abs() < 1e-10, "Global optimum value not as expected: {}", f_global);
    
    // Test that function values are bounded above by 0
    let test_points = vec![
        vec![1.0, 1.0],
        vec![-5.0, 3.0],
        vec![10.0, -10.0],
        vec![-15.0, 15.0],
    ];
    
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = xin_she_yang_n3(&x);
        
        assert!(f <= 0.0, "Function should be <= 0 at {:?}: {}", point, f);
        assert!(f >= -1.0, "Function should be >= -1 at {:?}: {}", point, f);
        assert!(f.is_finite(), "Function should be finite at {:?}: {}", point, f);
    }
}
