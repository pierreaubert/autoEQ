use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::cross_in_tray;

extern crate blas_src;
#[test]
fn test_de_cross_in_tray() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(71)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .build();
    let result = differential_evolution(&cross_in_tray, &b, c);
    // Cross-in-tray has global minimum f(x) = -2.06261 at (±1.34941, ±1.34941)
    assert!(result.fun < -2.0); // Should find solution close to global minimum
}

#[test]
fn test_de_cross_in_tray_recorded() {
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(71)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .build();
    
    let result = run_recorded_differential_evolution(
        "cross_in_tray", cross_in_tray, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -1.8); // Relaxed threshold for cross-in-tray
    
    // Check bounds
    assert!(report.x[0] >= -10.0 && report.x[0] <= 10.0);
    assert!(report.x[1] >= -10.0 && report.x[1] <= 10.0);
}
