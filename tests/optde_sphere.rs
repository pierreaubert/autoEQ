use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy};
use autoeq::optim::{auto_de, AutoDEParams};
use testfunctions::{sphere, rastrigin, create_bounds};

mod testfunctions;

#[test]
fn test_de_sphere_2d() {
    // Test 2D Sphere function using direct DE interface
    let b2 = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let c2 = DEConfigBuilder::new()
        .seed(30)
        .maxiter(500)
        .popsize(30)
        .strategy(Strategy::Rand1Bin)
        .recombination(0.8)
        .build();
    assert!(differential_evolution(&sphere, &b2, c2).fun < 1e-6);
}

#[test]
fn test_de_sphere_5d() {
    // Test 5D Sphere function
    let b5 = vec![(-5.0, 5.0); 5];
    let c5 = DEConfigBuilder::new()
        .seed(31)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Bin)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&sphere, &b5, c5).fun < 1e-5);
}

// Auto_de tests using the simplified interface

#[test]
fn test_auto_de_sphere_function() {
    let bounds = create_bounds(5, -10.0, 10.0);
    let result = auto_de(sphere, &bounds, None);

    assert!(result.is_some(), "AutoDE should find a solution");
    let (x_opt, f_opt, _) = result.unwrap();

    // Should find global minimum at origin
    assert!(f_opt < 1e-6, "Sphere function value too high: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-3, "Solution component too far from 0: {}", xi);
    }
}

#[test]
fn test_auto_de_sphere_performance_comparison() {
    // Compare performance on sphere (convex) vs other functions
    let bounds = create_bounds(5, -10.0, 10.0);

    let params = AutoDEParams {
        max_iterations: 300,
        population_size: Some(50),
        f: 0.8,
        cr: 0.9,
        tolerance: 1e-6,
        seed: Some(12345),
    };

    // Test on sphere function (should be fast)
    let result_sphere = auto_de(sphere, &bounds, Some(params.clone()));
    assert!(result_sphere.is_some(), "Sphere optimization should succeed");
    let (_, f_sphere, iter_sphere) = result_sphere.unwrap();

    // Test on multimodal rastrigin function (should be harder)
    let result_rastrigin = auto_de(rastrigin, &bounds, Some(params));
    assert!(result_rastrigin.is_some(), "Rastrigin optimization should succeed");
    let (_, f_rastrigin, iter_rastrigin) = result_rastrigin.unwrap();

    // Sphere should converge better than Rastrigin
    assert!(f_sphere < f_rastrigin, "Sphere should have better final value: {} vs {}", f_sphere, f_rastrigin);

    println!("Sphere: f={:.2e}, iter={}", f_sphere, iter_sphere);
    println!("Rastrigin: f={:.2e}, iter={}", f_rastrigin, iter_rastrigin);
}
