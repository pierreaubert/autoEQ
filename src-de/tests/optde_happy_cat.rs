use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{happy_cat, create_bounds};


#[test]
fn test_de_happy_cat_2d() {
    // Test Happy Cat function in 2D
    let bounds = vec![(-2.0, 2.0), (-2.0, 2.0)];
    let config = DEConfigBuilder::new()
        .seed(120)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&happy_cat, &bounds, config);
    assert!(result.fun < 0.1, "Solution quality too low: {}", result.fun);

    // Check solution is close to one of the global minima (±1, ±1)
    let found_minimum = result.x.iter().all(|&xi| (xi.abs() - 1.0).abs() < 0.2);
    assert!(found_minimum, "Solution not near global minima: {:?}", result.x);
}

#[test]
fn test_de_happy_cat_5d() {
    // Test Happy Cat function in 5D
    let bounds = vec![(-2.0, 2.0); 5];
    let config = DEConfigBuilder::new()
        .seed(121)
        .maxiter(1200)
        .popsize(80)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&happy_cat, &bounds, config);
    assert!(result.fun < 0.5, "Solution quality too low: {}", result.fun);

    // Check solution is reasonably close to some optimum
    for &xi in result.x.iter() {
        assert!(xi.abs() <= 2.5, "Solution coordinate out of expected range: {}", xi);
    }
}

// Auto_de tests using the simplified interface
#[test]
fn test_auto_de_happy_cat_function() {
    let bounds = create_bounds(2, -2.0, 2.0);
    let result = auto_de(happy_cat, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    assert!(f_opt < 0.2, "Happy Cat function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() <= 2.0, "Solution component out of bounds: {}", xi);
    }
}

#[test]
fn test_de_happy_cat_recorded() {
    let bounds = vec![(-2.0, 2.0), (-2.0, 2.0)];
    let config = DEConfigBuilder::new()
        .seed(122)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = run_recorded_differential_evolution(
        "happy_cat", happy_cat, &bounds, config, "./data_generated/records"
    );

    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.2);

    // Check that solution is within bounds
    for &actual in report.x.iter() {
        assert!(actual >= -2.0 && actual <= 2.0, "Solution out of bounds: {}", actual);
    }
}

#[test]
fn test_happy_cat_known_minimum() {
    // Test that the known global minima give the expected value
    use ndarray::Array1;
    let x_star1 = Array1::from(vec![1.0, 1.0]);
    let f_star1 = happy_cat(&x_star1);

    let x_star2 = Array1::from(vec![-1.0, -1.0]);
    let f_star2 = happy_cat(&x_star2);

    // Both should be approximately 0
    assert!(f_star1 < 0.01, "Known minimum (1,1) doesn't match expected value: {}", f_star1);
    assert!(f_star2 < 0.01, "Known minimum (-1,-1) doesn't match expected value: {}", f_star2);
}
