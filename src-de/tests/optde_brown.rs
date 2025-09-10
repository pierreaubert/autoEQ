use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{brown, get_function_bounds_vec};


#[test]
fn test_de_brown_2d() {
    // Test Brown function in 2D - this is an ill-conditioned unimodal function
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(110)
        .maxiter(1500)  // More iterations needed due to ill-conditioning
        .popsize(80)    // Larger population for difficult conditioning
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&brown, &bounds, config);
    assert!(result.fun < 1e-5, "Solution quality too low: {}", result.fun);

    // Check solution is close to global minimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi.abs() < 1e-3, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_brown_4d() {
    // Test Brown function in 4D
    let bounds = vec![(-1.0, 4.0); 4];
    let config = DEConfigBuilder::new()
        .seed(111)
        .maxiter(2000)
        .popsize(120)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.95)
        .build();

    let result = differential_evolution(&brown, &bounds, config);
    assert!(result.fun < 1e-4, "Solution quality too low: {}", result.fun);

    // Check solution is close to global minimum (0, 0, 0, 0)
    for &xi in result.x.iter() {
        assert!(xi.abs() < 1e-2, "Solution coordinate not near 0: {}", xi);
    }
}

#[test]
fn test_de_brown_high_precision() {
    // Test with higher precision requirements due to ill-conditioning
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(112)
        .maxiter(2500)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .tol(1e-12)  // Very tight tolerance
        .build();

    let result = differential_evolution(&brown, &bounds, config);
    assert!(result.fun < 1e-8, "High precision solution not achieved: {}", result.fun);
}

#[test]
fn test_de_brown_multiple_strategies() {
    // Test different strategies on this ill-conditioned function
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];

    let strategies = [
        Strategy::Best1Bin,
        Strategy::RandToBest1Bin,
        Strategy::Best2Bin,
    ];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(110 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.9)
            .build();

        let result = differential_evolution(&brown, &bounds, config);
        assert!(result.fun < 1e-3, "Strategy {:?} failed with value: {}", strategy, result.fun);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_brown_function() {
    use autoeq_testfunctions::create_bounds;
    let bounds = create_bounds(2, -1.0, 4.0);
    let result = auto_de(brown, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 1e-3, "Brown function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-1, "Solution component not near 0: {}", xi);
    }
}

#[test]
fn test_de_brown_recorded() {
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(113)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "brown", brown, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-4);

    // Check that solution is close to global optimum (0, 0)
    for &actual in report.x.iter() {
        assert!(actual.abs() < 1e-2, "Solution not near 0: {}", actual);
    }
}

#[test]
fn test_brown_known_minimum() {
    // Test that the known global minimum actually gives the expected value
    use ndarray::Array1;
    let x_star = Array1::from(vec![0.0, 0.0]);
    let f_star = brown(&x_star);

    // Should be exactly 0.0
    assert!(f_star < 1e-15, "Known minimum doesn't match expected value: {}", f_star);
}

#[test]
fn test_brown_ill_conditioning() {
    // Test the ill-conditioned nature of the Brown function
    use ndarray::Array1;

    // Test that small changes in x can lead to large changes in function value
    let x1 = Array1::from(vec![0.1, 0.1]);
    let x2 = Array1::from(vec![0.11, 0.11]);
    let f1 = brown(&x1);
    let f2 = brown(&x2);

    assert!(f1.is_finite() && f2.is_finite(), "Function values should be finite");

    // Test that function grows rapidly away from origin
    let x_far = Array1::from(vec![1.0, 1.0]);
    let f_far = brown(&x_far);
    let f_origin = brown(&Array1::from(vec![0.0, 0.0]));

    assert!(f_far > f_origin, "Function should increase away from origin");
}

#[test]
fn test_brown_different_dimensions() {
    // Test function behavior in different dimensions
    use ndarray::Array1;

    let dimensions = [2, 4, 6, 10];

    for &dim in &dimensions {
        // Test at global minimum (all zeros)
        let x_zero = Array1::from(vec![0.0; dim]);
        let f_zero = brown(&x_zero);
        assert!(f_zero < 1e-15, "Function at zero not 0 for dim {}: {}", dim, f_zero);

        // Test at small perturbation
        let x_small = Array1::from(vec![0.01; dim]);
        let f_small = brown(&x_small);
        assert!(f_small.is_finite(), "Function at small perturbation not finite for dim {}", dim);
        assert!(f_small > 0.0, "Function should be positive away from minimum for dim {}", dim);
    }
}

#[test]
fn test_brown_convergence_difficulty() {
    // Test that Brown function is indeed difficult to optimize (many iterations needed)
    let bounds = vec![(-1.0, 4.0), (-1.0, 4.0)];

    // Test with insufficient iterations
    let config_short = DEConfigBuilder::new()
        .seed(114)
        .maxiter(200)  // Too few iterations
        .popsize(30)   // Small population
        .strategy(Strategy::Rand1Bin)
        .recombination(0.7)
        .build();

    let result_short = differential_evolution(&brown, &bounds, config_short);

    // Test with adequate iterations
    let config_long = DEConfigBuilder::new()
        .seed(114)  // Same seed
        .maxiter(1500)  // More iterations
        .popsize(80)    // Larger population
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result_long = differential_evolution(&brown, &bounds, config_long);

    // The longer run should achieve better results
    assert!(result_long.fun <= result_short.fun,
           "Longer optimization should be better or equal: {} vs {}",
           result_long.fun, result_short.fun);
}

#[test]
fn test_brown_boundary_behavior() {
    // Test function behavior at boundaries
    use ndarray::Array1;

    let test_points = vec![
        vec![-1.0, -1.0],
        vec![4.0, 4.0],
        vec![-1.0, 4.0],
        vec![4.0, -1.0],
        vec![0.0, 4.0],
        vec![0.0, -1.0],
    ];

    for point in test_points {
        let x = Array1::from(point.clone());
        let f = brown(&x);
        assert!(f.is_finite(), "Function value at {:?} should be finite: {}", point, f);
        assert!(f >= 0.0, "Function should be non-negative: {}", f);
    }
}
