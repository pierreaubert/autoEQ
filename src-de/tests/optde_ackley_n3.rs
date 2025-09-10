use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{ackley_n3, create_bounds};


#[test]
fn test_de_ackley_n3_2d() {
    // Test Ackley N.3 function in 2D - variant with additional complexity
    let bounds = vec![(-32.0, 32.0), (-32.0, 32.0)];
    let config = DEConfigBuilder::new()
        .seed(150)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&ackley_n3, &bounds, config);

    // Ackley N.3 has global minimum around -195.6
    assert!(result.fun < -100.0, "Solution quality too low: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -32.0 && xi <= 32.0, "Solution coordinate out of bounds: {}", xi);
    }
}

#[test]
fn test_de_ackley_n3_different_strategies() {
    // Test multiple strategies to ensure robustness
    let bounds = vec![(-32.0, 32.0), (-32.0, 32.0)];

    let strategies = [Strategy::RandToBest1Bin, Strategy::Best2Bin, Strategy::Rand1Exp];

    for (i, strategy) in strategies.iter().enumerate() {
        let config = DEConfigBuilder::new()
            .seed(151 + i as u64)
            .maxiter(1500)
            .popsize(80)
            .strategy(*strategy)
            .recombination(0.8)
            .build();

        let result = differential_evolution(&ackley_n3, &bounds, config);
        assert!(result.fun < -50.0, "Strategy {:?} failed: {}", strategy, result.fun);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_ackley_n3_function() {
    let bounds = create_bounds(2, -32.0, 32.0);
    let result = auto_de(ackley_n3, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < -50.0, "Ackley N.3 function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi >= -32.0 && xi <= 32.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_ackley_n3_recorded() {
    let bounds = vec![(-32.0, 32.0), (-32.0, 32.0)];
    let config = DEConfigBuilder::new()
        .seed(152)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "ackley_n3", ackley_n3, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -50.0, "Recorded Ackley N.3 optimization failed: {}", report.fun);

    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -32.0 && actual <= 32.0, "Solution out of bounds: {}", actual);
    }
}
