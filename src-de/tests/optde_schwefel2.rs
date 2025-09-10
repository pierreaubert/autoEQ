use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::schwefel2;

#[test]
fn test_de_schwefel2() {
    let b = [(-500.0, 500.0), (-500.0, 500.0)];
    let c = DEConfigBuilder::new()
        .seed(23)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&schwefel2, &b, c).fun < 1e2);
}

#[test]
fn test_de_schwefel2_recorded() {
    let bounds = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let config = DEConfigBuilder::new()
        .seed(23)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();

    let result = run_recorded_differential_evolution(
        "schwefel2", schwefel2, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 200.0); // Relaxed threshold for schwefel2

    // Check bounds
    assert!(report.x[0] >= -500.0 && report.x[0] <= 500.0);
    assert!(report.x[1] >= -500.0 && report.x[1] <= 500.0);
}
