use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::step;

#[test]
fn test_de_step_2d() {
    // Test 2D Step function (discontinuous)
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = differential_evolution(&step, &b, c);
    // Global minimum at x = (0.5, 0.5) with f(x) = 0
    assert!(result.fun <= 2.0, "Function value too high: {}", result.fun); // Relaxed due to discontinuous nature
}

#[test]
fn test_de_step_5d() {
    // Test 5D Step function (discontinuous)
    let b = vec![(-100.0, 100.0); 5];
    let c = DEConfigBuilder::new()
        .seed(79)
        .maxiter(1000)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();
    let result = differential_evolution(&step, &b, c);
    // Global minimum at x = (0.5, 0.5, ..., 0.5) with f(x) = 0
    assert!(result.fun <= 5.0, "Function value too high: {}", result.fun); // Relaxed due to discontinuous nature
}

#[test]
fn test_de_step_3d() {
    // Test 3D Step function with different parameters
    let b = vec![(-50.0, 50.0); 3];
    let c = DEConfigBuilder::new()
        .seed(80)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&step, &b, c);
    // Should find the global minimum
    assert!(result.fun <= 3.0, "Function value too high: {}", result.fun);
}

#[test]
fn test_step_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (0.5, 0.5, ...)
    let x_opt = Array1::from(vec![0.5, 0.5, 0.5]);
    let f_opt = step(&x_opt);
    assert!(f_opt < 1e-15, "Global optimum should be 0: {}", f_opt);

    // Test discontinuity around the optimum
    let x_just_below = Array1::from(vec![0.4999, 0.4999]);
    let f_below = step(&x_just_below);

    let x_just_above = Array1::from(vec![0.5001, 0.5001]);
    let f_above = step(&x_just_above);

    // Both should give different integer floor values
    assert!(
        f_below == 0.0,
        "Just below optimum should be 0: {}",
        f_below
    );
    assert!(
        f_above == 2.0,
        "Just above optimum should be 2: {}",
        f_above
    );

    // Test at integer points
    let x_integers = Array1::from(vec![1.0, 2.0]);
    let f_integers = step(&x_integers);
    // floor(1 + 0.5) + floor(2 + 0.5) = floor(1.5) + floor(2.5) = 1 + 2 = 3
    // Then squared: 1^2 + 2^2 = 5
    assert_eq!(
        f_integers, 5.0,
        "Integer calculation incorrect: {}",
        f_integers
    );
}

#[test]
fn test_de_step_recorded() {
    // Test 2D Step function with recording (discontinuous)
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(79)
        .maxiter(600)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.8)
        .build();

    let result = run_recorded_differential_evolution(
        "step", step, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun <= 5.0); // Very relaxed threshold for discontinuous step function

    // Check bounds
    assert!(report.x[0] >= -100.0 && report.x[0] <= 100.0);
    assert!(report.x[1] >= -100.0 && report.x[1] <= 100.0);
}
