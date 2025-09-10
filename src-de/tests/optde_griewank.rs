use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::griewank;

#[test]
fn test_de_griewank_2d() {
    // Test 2D Griewank
    let b2 = vec![(-600.0, 600.0), (-600.0, 600.0)];
    let c2 = DEConfigBuilder::new()
        .seed(44)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&griewank, &b2, c2).fun < 1e-2);
}

#[test]
fn test_de_griewank_10d() {
    // Test 10D Griewank
    let b10 = vec![(-600.0, 600.0); 10];
    let c10 = DEConfigBuilder::new()
        .seed(45)
        .maxiter(1000)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&griewank, &b10, c10).fun < 1e-1); // Relaxed for 10D
}

#[test]
fn test_de_griewank_recorded() {
    // Test Griewank with recording (2D version)
    let b2 = vec![(-600.0, 600.0), (-600.0, 600.0)];
    let config = DEConfigBuilder::new()
        .seed(44)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("griewank_2d", griewank, &b2, config, "./data_generated/records");
    assert!(result.is_ok(), "Recorded optimization should succeed");

    let (solution, _csv_path) = result.unwrap();
    assert!(solution.fun < 1e-1, "Solution quality should be good: {}", solution.fun);

    // Check that solution is close to (0, 0) - global minimum of Griewank
    for (i, &xi) in solution.x.iter().enumerate() {
        assert!(xi.abs() < 1e-1, "x[{}] should be close to 0.0: {}", i, xi);
    }
}
