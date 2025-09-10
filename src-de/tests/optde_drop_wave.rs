use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::drop_wave;

#[test]
fn test_de_drop_wave() {
    let b = [(-5.12, 5.12), (-5.12, 5.12)];
    let c = DEConfigBuilder::new()
        .seed(72)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&drop_wave, &b, c);
    // Drop-wave has global minimum f(x=0, y=0) = -1
    assert!(result.fun < -0.99); // Should find solution very close to -1
    // Check that solution is close to origin
    let norm = (result.x[0].powi(2) + result.x[1].powi(2)).sqrt();
    assert!(norm < 0.1); // Should be very close to (0,0)
}

#[test]
fn test_de_drop_wave_recorded() {
    let bounds = vec![(-5.12, 5.12), (-5.12, 5.12)];
    let config = DEConfigBuilder::new()
        .seed(72)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "drop_wave", drop_wave, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -0.99); // Should find solution very close to -1

    // Check that solution is close to origin (0,0)
    let norm = (report.x[0].powi(2) + report.x[1].powi(2)).sqrt();
    assert!(norm < 0.2, "Solution should be close to origin: ({:.4}, {:.4})", report.x[0], report.x[1]);
}
