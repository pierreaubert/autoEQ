use autoeq_de::{auto_de, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{alpine_n1, create_bounds};


#[test]
fn test_de_alpine_n1_2d() {
    // Test Alpine N.1 in 2D
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(42)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "alpine_n1_2d", alpine_n1, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2, "Solution quality too low: {}", report.fun);

    // Check solution is close to global minimum (0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.2, "Solution coordinate too far from 0: {}", xi);
    }
}

#[test]
fn test_de_alpine_n1_5d() {
    // Test Alpine N.1 in 5D
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(43)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "alpine_n1_5d", alpine_n1, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2, "Solution quality too low: {}", report.fun);

    // Check solution is close to global minimum (0, 0, 0, 0, 0)
    for &xi in report.x.iter() {
        assert!(xi.abs() < 0.1, "Solution coordinate too far from 0: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_alpine_n1_function() {
    let bounds = create_bounds(2, -10.0, 10.0);
    let result = auto_de(alpine_n1, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 1e-2, "Alpine N.1 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 0.1, "Solution component too far from 0: {}", xi);
    }
}

