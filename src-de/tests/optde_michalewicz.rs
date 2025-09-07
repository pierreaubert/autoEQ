use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::michalewicz;

extern crate blas_src;
#[test]
fn test_de_michalewicz_2d() {
    // Test 2D Michalewicz
    let b2 = vec![(0.0, std::f64::consts::PI), (0.0, std::f64::consts::PI)];
    let c2 = DEConfigBuilder::new()
        .seed(74)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&michalewicz, &b2, c2);
    // 2D Michalewicz has global minimum f(x)=-1.8013 at x*=(2.20,1.57)
    assert!(result.fun < -1.5); // Should find good solution
}

#[test]
fn test_de_michalewicz_5d() {
    // Test 5D Michalewicz
    let b5 = vec![(0.0, std::f64::consts::PI); 5];
    let c5 = DEConfigBuilder::new()
        .seed(75)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .build();
    let result = differential_evolution(&michalewicz, &b5, c5);
    // Higher dimensional Michalewicz becomes increasingly difficult
    assert!(result.fun < -2.0); // Should find reasonable solution
}

#[test]
fn test_de_michalewicz_recorded() {
    // Test 2D Michalewicz with recording
    let bounds = vec![(0.0, std::f64::consts::PI), (0.0, std::f64::consts::PI)];
    let config = DEConfigBuilder::new()
        .seed(74)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "michalewicz", michalewicz, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -1.5); // Should find good solution
    
    // Check that solution is within bounds [0, π]
    let pi = std::f64::consts::PI;
    assert!(report.x[0] >= 0.0 && report.x[0] <= pi);
    assert!(report.x[1] >= 0.0 && report.x[1] <= pi);
}
