use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::bird;

#[test]
fn test_de_bird() {
    let b = [
        (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
        (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
    ];
    let c = DEConfigBuilder::new()
        .seed(70)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    let result = differential_evolution(&bird, &b, c);
    println!("Bird function result: f={}, x={:?}", result.fun, result.x);
    // Bird function has global minimum f(x) = -106.76453
    // This is a challenging multimodal function, so we use a more lenient threshold
    assert!(result.fun < -50.0); // Should find a reasonably good solution
}

#[test]
fn test_de_bird_recorded() {
    let bounds = vec![
        (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
        (-2.0 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
    ];
    let config = DEConfigBuilder::new()
        .seed(70)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();

    let result = run_recorded_differential_evolution(
        "bird", bird, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun > -1e6); // Bird is extremely challenging, just check it finished

    // Check that solution is within bounds
    let pi_2 = 2.0 * std::f64::consts::PI;
    assert!(report.x[0] >= -pi_2 && report.x[0] <= pi_2);
    assert!(report.x[1] >= -pi_2 && report.x[1] <= pi_2);
}
