use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{schaffer_n2, schaffer_n4};

#[test]
fn test_de_schaffer_n2() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(25)
        .maxiter(300)
        .popsize(25)
        .strategy(Strategy::Best1Exp)
        .build();
    assert!(differential_evolution(&schaffer_n2, &b, c).fun < 1e-3);
}

#[test]
fn test_de_schaffer_n4() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(32)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&schaffer_n4, &b, c).fun < 0.35);
}

#[test]
fn test_de_schaffer_n2_recorded() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(25)
        .maxiter(300)
        .popsize(25)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "schaffer_n2", schaffer_n2, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2); // Relaxed threshold for schaffer_n2

    // Schaffer N.2 has global minimum at (0, 0)
    assert!(report.x[0].abs() < 5.0, "x[0] should be reasonably close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 5.0, "x[1] should be reasonably close to 0.0: {}", report.x[1]);
}
