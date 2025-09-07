use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy};
use autoeq_testfunctions::ackley_n2;

extern crate blas_src;

#[test]
fn test_de_ackley_n2_basic() {
    // Test Ackley N.2 function - challenging multimodal function
    let b = [(-32.0, 32.0), (-32.0, 32.0)];
    let c = DEConfigBuilder::new()
        .seed(87)
        .maxiter(1200)
        .popsize(70)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    let result = differential_evolution(&ackley_n2, &b, c);
    // Global minimum f(x*) = -200 at x = (0, 0)
    assert!(
        result.fun < -190.0,
        "Function value too high: {}",
        result.fun
    ); // Should find solution close to global minimum
}

#[test]
fn test_de_ackley_n2_focused_bounds() {
    // Test with bounds closer to the optimum
    let b = [(-5.0, 5.0), (-5.0, 5.0)]; // Smaller bounds around the global optimum
    let c = DEConfigBuilder::new()
        .seed(88)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&ackley_n2, &b, c);
    // Should find better solution with focused bounds
    assert!(
        result.fun < -195.0,
        "Function value too high with focused bounds: {}",
        result.fun
    );
    // Check solution is close to (0, 0)
    for (i, &xi) in result.x.iter().enumerate() {
        assert!(xi.abs() < 0.5, "x[{}] should be close to 0: {}", i, xi);
    }
}

#[test]
fn test_de_ackley_n2_multistart() {
    // Test with multiple random starts to verify robustness
    let b = [(-20.0, 20.0), (-20.0, 20.0)];
    let seeds = [700, 800, 900];

    let mut best_result = f64::INFINITY;
    let mut results = Vec::new();

    for (i, &seed) in seeds.iter().enumerate() {
        let c = DEConfigBuilder::new()
            .seed(seed)
            .maxiter(1500)
            .popsize(100)
            .strategy(Strategy::Rand1Bin)
            .recombination(0.8)
            .build();
        let result = differential_evolution(&ackley_n2, &b, c);
        best_result = best_result.min(result.fun);
        results.push(result.fun);
        println!("Run {} (seed {}): f = {}", i, seed, result.fun);
    }

    // At least one run should find a good solution
    assert!(
        best_result < -180.0,
        "Best result across runs too high: {}",
        best_result
    );

    // Most runs should find reasonable solutions (multimodal, but should converge)
    let good_runs = results.iter().filter(|&&f| f < -150.0).count();
    assert!(
        good_runs >= 2,
        "Too few good runs: {} out of {}",
        good_runs,
        results.len()
    );
}

#[test]
fn test_de_ackley_n2_large_bounds() {
    // Test with the standard large bounds
    let b = [(-32.0, 32.0), (-32.0, 32.0)];
    let c = DEConfigBuilder::new()
        .seed(89)
        .maxiter(2000)
        .popsize(150)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&ackley_n2, &b, c);
    // Should still find decent solution even with large bounds
    assert!(
        result.fun < -150.0,
        "Function value too high with large bounds: {}",
        result.fun
    );
}

#[test]
fn test_ackley_n2_function_properties() {
    use ndarray::Array1;

    // Test that the function behaves as expected at known points

    // At global optimum (0, 0)
    let x_opt = Array1::from(vec![0.0, 0.0]);
    let f_opt = ackley_n2(&x_opt);
    // Should be exactly -200
    assert!(
        (f_opt - (-200.0)).abs() < 1e-10,
        "Global optimum should be -200: {}",
        f_opt
    );

    // Test the function structure:
    // f(x1,x2) = -200 * exp(-0.02 * sqrt(x1^2 + x2^2)) * cos(2π*x1) * cos(2π*x2)

    // At unit distance from origin
    let x_unit = Array1::from(vec![1.0, 0.0]);
    let f_unit = ackley_n2(&x_unit);
    let expected_unit =
        -200.0 * (-0.02 * 1.0_f64.sqrt()).exp() * (2.0 * std::f64::consts::PI).cos() * 1.0;
    assert!(
        (f_unit - expected_unit).abs() < 1e-10,
        "Unit distance calculation incorrect: {} vs {}",
        f_unit,
        expected_unit
    );

    // The function should be worse (closer to 0) as we move away from origin
    assert!(
        f_unit > f_opt,
        "Moving away from origin should worsen function value: {} vs {}",
        f_unit,
        f_opt
    );

    // Test symmetry - function should be symmetric around origin
    let x_pos = Array1::from(vec![1.0, 1.0]);
    let f_pos = ackley_n2(&x_pos);

    let x_neg = Array1::from(vec![-1.0, -1.0]);
    let f_neg = ackley_n2(&x_neg);

    // Should be identical due to symmetry (cos is even function)
    assert!(
        (f_pos - f_neg).abs() < 1e-15,
        "Function should be symmetric: {} vs {}",
        f_pos,
        f_neg
    );

    // Test the multimodal nature - cosine terms create oscillations
    // At points where cos terms are -1, function should be positive
    let x_cos_neg = Array1::from(vec![0.5, 0.5]); // cos(π) = -1 for both terms
    let f_cos_neg = ackley_n2(&x_cos_neg);

    let norm = (0.5_f64.powi(2) + 0.5_f64.powi(2)).sqrt();
    let expected_cos_neg = -200.0 * (-0.02 * norm).exp() * (-1.0) * (-1.0); // Both cos terms = -1, so result is positive
    assert!(
        (f_cos_neg - expected_cos_neg).abs() < 1e-10,
        "Cosine negative test incorrect: {} vs {}",
        f_cos_neg,
        expected_cos_neg
    );
    assert!(
        f_cos_neg > 0.0,
        "Function should be positive when cosines are negative: {}",
        f_cos_neg
    );

    // Test exponential decay - further from origin should have smaller magnitude
    let x_far = Array1::from(vec![10.0, 0.0]);
    let f_far = ackley_n2(&x_far);
    assert!(
        f_far.abs() < f_unit.abs(),
        "Function magnitude should decay with distance: |{}| vs |{}|",
        f_far,
        f_unit
    );
}
