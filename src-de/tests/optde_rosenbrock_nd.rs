use autoeq_de::auto_de;
use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{create_bounds, rosenbrock};

extern crate blas_src;
#[test]
fn test_de_rosenbrock_2d() {
    // Test 2D Rosenbrock
    let b2 = vec![(-2.048, 2.048), (-2.048, 2.048)];
    let c2 = DEConfigBuilder::new()
        .seed(48)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&rosenbrock, &b2, c2).fun < 1e-4);
}

#[test]
fn test_de_rosenbrock_10d() {
    // Test 10D Rosenbrock
    let b10 = vec![(-2.048, 2.048); 10];
    let c10 = DEConfigBuilder::new()
        .seed(49)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&rosenbrock, &b10, c10).fun < 1e-1);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_rosenbrock_2d() {
    let bounds = create_bounds(2, -2.0, 2.0);
    let result = auto_de(rosenbrock, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Should find minimum at (1, 1) with f = 0
    assert!(
        f_opt < 1e-3,
        "Rosenbrock function value too high: {}",
        f_opt
    );
    assert!(
        (x_opt[0] - 1.0).abs() < 1e-2,
        "x[0] should be close to 1.0: {}",
        x_opt[0]
    );
    assert!(
        (x_opt[1] - 1.0).abs() < 1e-2,
        "x[1] should be close to 1.0: {}",
        x_opt[1]
    );
}

#[test]
fn test_auto_de_rosenbrock_4d() {
    let bounds = create_bounds(4, -2.0, 2.0);
    let result = auto_de(rosenbrock, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Should find minimum at (1, 1, 1, 1)
    assert!(
        f_opt < 1e-2,
        "4D Rosenbrock function value too high: {}",
        f_opt
    );
    for &xi in x_opt.iter() {
        assert!(
            (xi - 1.0).abs() < 1e-1,
            "Solution component should be close to 1.0: {}",
            xi
        );
    }
}

#[test]
fn test_de_rosenbrock_recorded() {
    // Test Rosenbrock with recording (2D version)
    let b2 = vec![(-2.048, 2.048), (-2.048, 2.048)];
    let config = DEConfigBuilder::new()
        .seed(48)
        .maxiter(400)
        .popsize(40)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution("rosenbrock_2d", rosenbrock, &b2, config, "./data_generated/records");
    assert!(result.is_ok(), "Recorded optimization should succeed");
    
    let (solution, _csv_path) = result.unwrap();
    assert!(solution.fun < 1e-3, "Solution quality should be good: {}", solution.fun);
    
    // Check that solution is close to (1, 1)
    assert!((solution.x[0] - 1.0).abs() < 1e-2, "x[0] should be close to 1.0: {}", solution.x[0]);
    assert!((solution.x[1] - 1.0).abs() < 1e-2, "x[1] should be close to 1.0: {}", solution.x[1]);
}
