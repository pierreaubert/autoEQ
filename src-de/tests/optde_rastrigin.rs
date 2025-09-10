use autoeq_de::auto_de;
use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{create_bounds, rastrigin};

#[test]
fn test_de_rastrigin_2d() {
    // Test 2D Rastrigin
    let b2 = vec![(-5.12, 5.12), (-5.12, 5.12)];
    let c2 = DEConfigBuilder::new()
        .seed(40)
        .maxiter(1000)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&rastrigin, &b2, c2).fun < 1e-2);
}

#[test]
fn test_de_rastrigin_5d() {
    // Test 5D Rastrigin
    let b5 = vec![(-5.12, 5.12); 5];
    let c5 = DEConfigBuilder::new()
        .seed(41)
        .maxiter(1500)
        .popsize(75)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&rastrigin, &b5, c5).fun < 1e-1);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_rastrigin_function() {
    let bounds = create_bounds(3, -5.12, 5.12);
    let result = auto_de(rastrigin, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Rastrigin is highly multimodal, so we allow larger tolerance
    assert!(f_opt < 1e-1, "Rastrigin function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-1, "Solution component too far from 0: {}", xi);
    }
}

#[test]
fn test_de_rastrigin_recorded() {
    // Test Rastrigin with recording (2D version)
    let b2 = vec![(-5.12, 5.12), (-5.12, 5.12)];
    let config = DEConfigBuilder::new()
        .seed(40)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution("rastrigin_2d", rastrigin, &b2, config, "./data_generated/records");
    assert!(result.is_ok(), "Recorded optimization should succeed");

    let (solution, _csv_path) = result.unwrap();
    assert!(solution.fun < 1e-1, "Solution quality should be good: {}", solution.fun);

    // Check that solution is close to (0, 0)
    for (i, &xi) in solution.x.iter().enumerate() {
        assert!(xi.abs() < 1e-1, "x[{}] should be close to 0.0: {}", i, xi);
    }
}
