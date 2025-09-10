use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::eggholder;

#[test]
fn test_de_eggholder() {
    let b = [(-512.0, 512.0), (-512.0, 512.0)];
    let c = DEConfigBuilder::new()
        .seed(27)
        .maxiter(1200)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&eggholder, &b, c).fun < -700.0);
}

#[test]
fn test_de_eggholder_recorded() {
    let bounds = vec![(-512.0, 512.0), (-512.0, 512.0)];
    let config = DEConfigBuilder::new()
        .seed(27)
        .maxiter(1000)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();

    let result = run_recorded_differential_evolution(
        "eggholder", eggholder, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -500.0); // Eggholder is challenging, so relaxed threshold

    // Check that solution is within reasonable bounds
    assert!(report.x[0].abs() <= 512.0);
    assert!(report.x[1].abs() <= 512.0);
}
