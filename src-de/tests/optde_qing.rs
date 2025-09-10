use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{qing, create_bounds};


#[test]
fn test_de_qing_2d() {
    // Test Qing function in 2D - separable multimodal function
    let bounds = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let config = DEConfigBuilder::new()
        .seed(220)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = differential_evolution(&qing, &bounds, config);

    // Global minimum is at (√1, √2) = (1, 1.414...) with f = 0
    assert!(result.fun < 1e-2, "Solution quality too low: {}", result.fun);

    // Check solution is close to known optimum (1, √2)
    assert!(result.x[0] >= -500.0 && result.x[0] <= 500.0, "x1 coordinate out of bounds: {}", result.x[0]);
    assert!(result.x[1] >= -500.0 && result.x[1] <= 500.0, "x2 coordinate out of bounds: {}", result.x[1]);

    // Check if it found the positive or negative optima
    let expected_x1 = [1.0, -1.0];
    let expected_x2 = [1.41421356, -1.41421356]; // √2

    let found_x1 = expected_x1.iter().any(|&exp| (result.x[0] - exp).abs() < 0.1);
    let found_x2 = expected_x2.iter().any(|&exp| (result.x[1] - exp).abs() < 0.1);

    assert!(found_x1, "x1 not near expected values ±1: {}", result.x[0]);
    assert!(found_x2, "x2 not near expected values ±√2: {}", result.x[1]);
}

#[test]
fn test_de_qing_5d() {
    // Test Qing function in 5D - should be tractable being separable
    let bounds = vec![(-500.0, 500.0); 5];
    let config = DEConfigBuilder::new()
        .seed(221)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&qing, &bounds, config);

    // For 5D, should still converge well being separable
    assert!(result.fun < 0.1, "Solution quality too low for 5D: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -500.0 && xi <= 500.0, "Solution coordinate out of bounds: {}", xi);
    }
}

#[test]
fn test_de_qing_10d() {
    // Test Qing function in 10D - separable should scale well
    let bounds = vec![(-500.0, 500.0); 10];
    let config = DEConfigBuilder::new()
        .seed(222)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::Best2Bin)
        .recombination(0.8)
        .build();

    let result = differential_evolution(&qing, &bounds, config);

    // Should still converge for separable function
    assert!(result.fun < 1.0, "Solution quality too low for 10D: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -500.0 && xi <= 500.0, "Solution coordinate out of bounds: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_qing_function() {
    let bounds = create_bounds(2, -500.0, 500.0);
    let result = auto_de(qing, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 0.5, "Qing function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -500.0 && xi <= 500.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_qing_recorded() {
    let bounds = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let config = DEConfigBuilder::new()
        .seed(223)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Best1Bin)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution(
        "qing", qing, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.5, "Recorded Qing optimization failed: {}", report.fun);

    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -500.0 && actual <= 500.0, "Solution out of bounds: {}", actual);
    }
}

