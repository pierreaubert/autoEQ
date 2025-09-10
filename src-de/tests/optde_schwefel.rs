use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::schwefel;

#[test]
fn test_de_schwefel_2d() {
    // Test 2D Schwefel
    let b2 = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let c2 = DEConfigBuilder::new()
        .seed(46)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&schwefel, &b2, c2).fun < 1e-2);
}

#[test]
fn test_de_schwefel_5d() {
    // Test 5D Schwefel
    let b5 = vec![(-500.0, 500.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(47)
        .maxiter(1500)
        .popsize(100)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    assert!(differential_evolution(&schwefel, &b5, c5).fun < 1e-1);
}

#[test]
fn test_de_schwefel_recorded() {
    // Test Schwefel with recording (2D version)
    let b2 = vec![(-500.0, 500.0), (-500.0, 500.0)];
    let config = DEConfigBuilder::new()
        .seed(46)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();

    let result = run_recorded_differential_evolution("schwefel_2d", schwefel, &b2, config, "./data_generated/records");
    assert!(result.is_ok(), "Recorded optimization should succeed");

    let (solution, _csv_path) = result.unwrap();
    assert!(solution.fun < 1e-1, "Solution quality should be good: {}", solution.fun);

    // Check that solution is close to (420.9687, 420.9687) - global minimum of Schwefel
    for (i, &xi) in solution.x.iter().enumerate() {
        assert!((xi - 420.9687).abs() < 10.0, "x[{}] should be close to 420.9687: {}", i, xi);
    }
}
