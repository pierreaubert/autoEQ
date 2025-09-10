use autoeq_de::{auto_de, differential_evolution, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::{xin_she_yang_n4, create_bounds};


#[test]
fn test_de_xin_she_yang_n4_2d() {
    // Test Xin-She Yang N.4 function in 2D - challenging multimodal
    let bounds = vec![(-10.0, 10.0), (-10.0, 10.0)];
    let config = DEConfigBuilder::new()
        .seed(200)
        .maxiter(2500)
        .popsize(120)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();

    let result = differential_evolution(&xin_she_yang_n4, &bounds, config);

    // Global minimum is at (0, 0) with f = -1
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < -0.3, "Solution quality too low: {}", result.fun);

    // Check solution is close to known optimum (0, 0)
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
        assert!(xi.abs() < 3.0, "Solution not reasonably near global optimum (0, 0): {}", xi);
    }
}

#[test]
fn test_de_xin_she_yang_n4_5d() {
    // Test Xin-She Yang N.4 function in 5D - very challenging
    let bounds = vec![(-10.0, 10.0); 5];
    let config = DEConfigBuilder::new()
        .seed(201)
        .maxiter(3000)
        .popsize(150)
        .strategy(Strategy::RandToBest1Bin)
        .recombination(0.8)
        .build();

    let result = differential_evolution(&xin_she_yang_n4, &bounds, config);

    // For 5D, accept much higher tolerance due to complexity
    assert!(result.fun > -1.01, "Solution too good (below theoretical minimum): {}", result.fun);
    assert!(result.fun < 0.5, "Solution quality too low for 5D: {}", result.fun);

    // Check solution is within bounds
    for &xi in result.x.iter() {
        assert!(xi >= -10.0 && xi <= 10.0, "Solution coordinate out of bounds: {}", xi);
    }
}

