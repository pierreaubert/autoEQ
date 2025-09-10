use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{exponential, create_bounds};


#[test]
fn test_de_exponential_2d() {
    // Test exponential function in 2D - unimodal function
    let bounds = vec![(-1.0, 1.0), (-1.0, 1.0)];
    let config = DEConfigBuilder::new()
        .seed(170)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.7)
        .build();

    let result = differential_evolution(&exponential, &bounds, config);

    // Global minimum is at (0, 0) with f = -1
    assert!(result.fun > -1.001, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.9, "Solution quality too low: {}", result.fun);

    // Check solution is close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -1.0 && xi <= 1.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 1e-2, "Solution not near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_exponential_5d() {
    // Test exponential function in 5D - should still be easy to optimize being unimodal
    let bounds = vec![(-1.0, 1.0); 5];
    let config = DEConfigBuilder::new()
        .seed(171)
        .maxiter(1200)
        .popsize(70)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();

    let result = differential_evolution(&exponential, &bounds, config);

    // Should still achieve good precision for unimodal function
    assert!(result.fun > -1.001, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.8, "Solution quality too low for 5D: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -1.0 && xi <= 1.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 0.1, "Solution not near global optimum: {}", xi);
    }
}

#[test]
fn test_de_exponential_10d() {
    // Test exponential function in 10D - higher dimensional unimodal
    let bounds = vec![(-1.0, 1.0); 10];
    let config = DEConfigBuilder::new()
        .seed(172)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Best2Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&exponential, &bounds, config);

    // Still should converge well for unimodal function
    assert!(result.fun > -1.001, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.5, "Solution quality too low for 10D: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -1.0 && xi <= 1.0, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_exponential_function() {
    let bounds = create_bounds(2, -1.0, 1.0);
    let result = auto_de(exponential, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt > -1.001, "Solution too good (below theoretical minimum): {}", f_opt);
    assert!(f_opt < -0.7, "Exponential function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -1.0 && xi <= 1.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_exponential_recorded() {
    let bounds = vec![(-1.0, 1.0), (-1.0, 1.0)];
    let config = DEConfigBuilder::new()
        .seed(173)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.7)
        .build();

    let result = run_recorded_differential_evolution(
        "exponential", exponential, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun > -1.001, "Solution too good (below theoretical minimum): {}", report.fun);
    assert!(report.fun < -0.7, "Recorded exponential optimization failed: {}", report.fun);

    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -1.0 && actual <= 1.0, "Solution out of bounds: {}", actual);
    }
}

