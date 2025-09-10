use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{alpine_n2, create_bounds};


#[test]
fn test_de_alpine_n2_2d() {
    // Test Alpine N.2 in 2D
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(50)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&alpine_n2, &bounds, config);
    assert!(result.fun < -7.0, "Solution quality too low: {}", result.fun);

    // Check solution is close to global minimum (2.808, 2.808)
    for &xi in result.x.iter() {
        assert!((xi - 2.808).abs() < 0.5, "Solution coordinate not near 2.808: {}", xi);
    }
}

#[test]
fn test_de_alpine_n2_3d() {
    // Test Alpine N.2 in 3D
    let bounds = vec![(0.0, 10.0); 3];
    let config = DEConfigBuilder::new()
        .seed(51)
        .maxiter(1500)
        .popsize(75)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.95)
        .build();

    let result = differential_evolution(&alpine_n2, &bounds, config);
    // For 3D: expected minimum is approximately -2.808^3 ≈ -22.2
    assert!(result.fun < -20.0, "Solution quality too low: {}", result.fun);

    // Check solution is close to global minimum (2.808, 2.808, 2.808)
    for &xi in result.x.iter() {
        assert!((xi - 2.808).abs() < 0.5, "Solution coordinate not near 2.808: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_alpine_n2_function() {
    let bounds = create_bounds(2, 0.0, 10.0);
    let result = auto_de(alpine_n2, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < -7.0, "Alpine N.2 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!((xi - 2.808).abs() < 0.5, "Solution component not near 2.808: {}", xi);
    }
}

#[test]
fn test_de_alpine_n2_recorded() {
    let bounds = vec![(0.0, 10.0), (0.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(52)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "alpine_n2", alpine_n2, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -7.0);

    // Check that solution is close to global optimum (2.808, 2.808)
    for &actual in report.x.iter() {
        assert!((actual - 2.808).abs() < 0.5, "Solution not near 2.808: {}", actual);
    }
}
