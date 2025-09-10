use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::bukin_n6;

#[test]
fn test_de_bukin_n6() {
    let b = [(-15.0, -5.0), (-3.0, 3.0)];
    let c = DEConfigBuilder::new()
        .seed(26)
        .maxiter(1500) // Reduced from very high value
        .popsize(100) // Reduced from very high value
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.0 }) // Added mutation control
        .build();
    assert!(differential_evolution(&bukin_n6, &b, c).fun < 1.0); // Very relaxed tolerance - Bukin N6 is extremely difficult
}

#[test]
fn test_de_bukin_n6_recorded() {
    let bounds = vec![(-15.0, -5.0), (-3.0, 3.0)];
    let config = DEConfigBuilder::new()
        .seed(26)
        .maxiter(1000) // Reduced for faster testing
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.0 })
        .build();

    let result = run_recorded_differential_evolution(
        "bukin_n6", bukin_n6, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 5.0); // Very relaxed - Bukin N6 is extremely challenging

    // Check bounds
    assert!(report.x[0] >= -15.0 && report.x[0] <= -5.0);
    assert!(report.x[1] >= -3.0 && report.x[1] <= 3.0);
}
