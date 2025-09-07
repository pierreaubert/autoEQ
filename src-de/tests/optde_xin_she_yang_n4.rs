use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{xin_she_yang_n4, create_bounds};

extern crate blas_src;

#[test]
fn test_de_xin_she_yang_n4_2d() {
    // Test Xin-She Yang N.4 function in 2D - challenging multimodal
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(200)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n4, &bounds, config);
    
    // Global minimum is at (0, 0) with f = -1
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.3, "Solution quality too low: {}", result.fun);
    
    // Check solution is close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 3.0, "Solution not reasonably near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_xin_she_yang_n4_5d() {
    // Test Xin-She Yang N.4 function in 5D - very challenging
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(201)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.8)
        .build();
    
    let result = differential_evolution(&xin_she_yang_n4, &bounds, config);
    
    // For 5D, accept much higher tolerance due to complexity
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < 0.5, "Solution quality too low for 5D: {}", result.fun);
    
    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_xin_she_yang_n4_function() {
    let bounds = create_bounds(2, -10.0, 10.0);
    let result = auto_de(xin_she_yang_n4, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // This function is very challenging, so accept reasonable improvements
    assert!(f_opt > -1.01, "Solution too good (below theoretical minimum): {}", f_opt);
    assert!(f_opt < 1.0, "Xin-She Yang N.4 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution component out of bounds: {}", xi);
    }
}

#[test] 
fn test_xin_she_yang_n4_known_properties() {
    // Test some properties of the Xin-She Yang N.4 function
    use ndarray::Array1;
    
    // Test the known global optimum
    let x_global = Array1::from(vec![0.0, 0.0]);
    let f_global = xin_she_yang_n4(&x_global);
    
    // Should be -1 at the global optimum
    assert!((f_global + 1.0).abs() < 1e-10, "Global optimum value not as expected: {}", f_global);
    
    // Test that function is finite at various points
    let test_points = vec![
        vec![1.0, 1.0],
        vec![-3.0, 2.0],
        vec![5.0, -5.0],
        vec![-8.0, 8.0],
    ];
    
    for point in test_points {
        let x = Array1::from(point.clone());
        let f = xin_she_yang_n4(&x);
        
        assert!(f.is_finite(), "Function should be finite at {:?}: {}", point, f);
        // This is a very complex function, so just check it's bounded reasonably
        assert!(f > -2.0, "Function seems too negative at {:?}: {}", point, f);
        assert!(f < 100.0, "Function seems too positive at {:?}: {}", point, f);
    }
    
    // Test boundary behavior
    let x_boundary = Array1::from(vec![10.0, -10.0]);
    let f_boundary = xin_she_yang_n4(&x_boundary);
    assert!(f_boundary.is_finite(), "Function at boundary should be finite");
}
