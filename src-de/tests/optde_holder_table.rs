use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::holder_table;

#[test]
fn test_de_holder_table() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(73)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    let result = differential_evolution(&holder_table, &b, c);
    // Holder table has global minimum f(x) = -19.2085 at (±8.05502, ±9.66459)
    assert!(result.fun < -18.0); // Should find solution close to global minimum
}

#[test]
fn test_de_holder_table_recorded() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(73)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();

    let result = run_recorded_differential_evolution(
        "holder_table", holder_table, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -15.0); // Relaxed threshold for holder table

    // Check bounds
    assert!(report.x[0] >= -10.0 && report.x[0] <= 10.0);
    assert!(report.x[1] >= -10.0 && report.x[1] <= 10.0);
}
