use autoeq_de::auto_de;
use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy};
use autoeq_testfunctions::{create_bounds, quadratic};

extern crate blas_src;
#[test]
fn test_de_quadratic_2d() {
    // Test 2D quadratic function using direct DE interface
    let b2 = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let c2 = DEConfigBuilder::new()
        .seed(10)
        .maxiter(300)
        .popsize(20)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();
    assert!(differential_evolution(&quadratic, &b2, c2).fun < 1e-8);
}

#[test]
fn test_de_quadratic_5d() {
    // Test 5D quadratic function
    let b5 = vec![(-5.0, 5.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(11)
        .maxiter(500)
        .popsize(40)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&quadratic, &b5, c5).fun < 1e-7);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_quadratic_optimization() {
    let bounds = create_bounds(2, -5.0, 5.0);
    let result = auto_de(quadratic, &bounds, None);

    // Should find minimum at (0, 0)
    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Check function value is close to 0
    assert!(f_opt < 1e-6, "Function value too high: {}", f_opt);

    // Check solution is close to (0, 0)
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-3, "Solution component too far from 0: {}", xi);
    }
}

#[test]
fn test_auto_de_bounds_enforcement() {
    // Test that solution respects bounds
    let bounds = create_bounds(3, -1.0, 1.0);
    let result = auto_de(quadratic, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, _, _) = result.unwrap();

    // Check all components are within bounds
    for &xi in x_opt.iter() {
        assert!(
            xi >= -1.0 && xi <= 1.0,
            "Solution {} violates bounds [-1, 1]",
            xi
        );
    }
}

#[test]
fn test_auto_de_asymmetric_bounds() {
    use ndarray::Array2;

    // Test with asymmetric bounds
    let mut bounds = Array2::zeros((2, 2));
    bounds[[0, 0]] = -10.0;
    bounds[[1, 0]] = -5.0; // x[0] ∈ [-10, -5]
    bounds[[0, 1]] = 2.0;
    bounds[[1, 1]] = 8.0; // x[1] ∈ [2, 8]

    let result = auto_de(quadratic, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, _, _) = result.unwrap();

    // Solution should be at the bounds closest to origin
    assert!(
        (x_opt[0] - (-5.0)).abs() < 1e-2,
        "x[0] should be close to -5.0: {}",
        x_opt[0]
    );
    assert!(
        (x_opt[1] - 2.0).abs() < 1e-2,
        "x[1] should be close to 2.0: {}",
        x_opt[1]
    );
}

#[test]
fn test_auto_de_single_dimension() {
    // Test 1D optimization
    let bounds = create_bounds(1, -10.0, 10.0);

    // Simple parabola: f(x) = (x - 3)^2
    let parabola = |x: &ndarray::Array1<f64>| (x[0] - 3.0).powi(2);

    let result = auto_de(parabola, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(
        (x_opt[0] - 3.0).abs() < 1e-2,
        "x[0] should be close to 3.0: {}",
        x_opt[0]
    );
    assert!(
        f_opt < 1e-6,
        "1D parabola function value too high: {}",
        f_opt
    );
}

#[test]
fn test_auto_de_large_dimension() {
    // Test higher dimensional optimization
    let n = 10;
    let bounds = create_bounds(n, -5.0, 5.0);
    let result = auto_de(quadratic, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert_eq!(x_opt.len(), n);
    assert!(
        f_opt < 1e-3,
        "10D quadratic function value too high: {}",
        f_opt
    );

    // All components should be close to 0
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-1, "Solution component too far from 0: {}", xi);
    }
}
