use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::beale;

#[test]
fn test_de_beale_multistart() {
    let bounds = [(-4.5, 4.5), (-4.5, 4.5)];
    let seeds = [42, 123, 456, 789];
    let mut best_result = f64::INFINITY;

    for &seed in &seeds {
        let mut config = DEConfig::default();
        config.seed = Some(seed);
        config.maxiter = 1000;
        config.popsize = 50;
        config.recombination = 0.8;
        config.strategy = Strategy::Rand1Bin;

        let result = differential_evolution(&beale, &bounds, config);
        best_result = best_result.min(result.fun);
    }

    // At least one run should find a good solution
    assert!(best_result < 1e-3);
}

#[test]
fn test_de_beale() {
    let bounds = vec![(-4.5, 4.5), (-4.5, 4.5)];
    let config = DEConfigBuilder::new()
        .seed(456)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "beale", beale, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2); // Relaxed tolerance for Beale

    // Check that solution is close to expected optimum (3, 0.5)
    let expected = [3.0, 0.5];
    for (actual, expected) in report.x.iter().zip(expected.iter()) {
        assert!((actual - expected).abs() < 0.5);
    }
}
