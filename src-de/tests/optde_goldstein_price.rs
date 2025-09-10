use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::goldstein_price;

#[test]
fn test_de_goldstein_price() {
    let b = [(-2.0, 2.0), (-2.0, 2.0)];
    let mut c = DEConfig::default();
    c.seed = Some(7);
    c.maxiter = 600;
    c.popsize = 30;
    c.strategy = Strategy::Rand1Exp;
    assert!(differential_evolution(&goldstein_price, &b, c).fun < 3.01);
}

#[test]
fn test_de_goldstein_price_recorded() {
    let bounds = vec![(-2.0, 2.0), (-2.0, 2.0)];
    let config = DEConfigBuilder::new()
        .seed(7)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "goldstein_price", goldstein_price, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 3.01);

    // Check that solution is close to global minimum at (0, -1) with f = 3
    assert!((report.x[0] - 0.0).abs() < 0.5, "x[0] should be close to 0.0: {}", report.x[0]);
    assert!((report.x[1] + 1.0).abs() < 0.5, "x[1] should be close to -1.0: {}", report.x[1]);
}
