use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::himmelblau;

extern crate blas_src;

#[test]
fn test_de_himmelblau() {
    let bounds = [(-6.0, 6.0), (-6.0, 6.0)];
    let mut config = DEConfig::default();
    config.seed = Some(5);
    config.maxiter = 800;
    config.popsize = 40;
    config.recombination = 0.9;
    config.strategy = Strategy::RandToBest1Exp;
    
    let result = differential_evolution(&himmelblau, &bounds, config);
    
    // Himmelblau function: Global minima f(x) = 0 at multiple points:
    // (3, 2), (-2.805118, 3.131312), (-3.779310, -3.283186), (3.584428, -1.848126)
    assert!(result.fun < 1e-2); // Relaxed tolerance as in original
}

#[test]
fn test_de_himmelblau_find_minima() {
    let bounds = [(-6.0, 6.0), (-6.0, 6.0)];
    let seeds = [5, 42, 123, 456, 789, 999];
    let mut solutions = Vec::new();
    
    // Try multiple seeds to potentially find different minima
    for &seed in &seeds {
        let mut config = DEConfig::default();
        config.seed = Some(seed);
        config.maxiter = 1000;
        config.popsize = 50;
        config.recombination = 0.8;
        config.strategy = Strategy::RandToBest1Exp;
        
        let result = differential_evolution(&himmelblau, &bounds, config);
        
        if result.fun < 1e-2 {
            solutions.push((result.x[0], result.x[1], result.fun));
        }
    }
    
    // Should find at least one valid minimum
    assert!(!solutions.is_empty(), "Should find at least one minimum");
    
    // All solutions should be close to one of the known minima
    let known_minima = [
        (3.0, 2.0),
        (-2.805118, 3.131312),
        (-3.779310, -3.283186),
        (3.584428, -1.848126),
    ];
    
    for (x, y, _f) in solutions {
        let mut found_match = false;
        for &(mx, my) in &known_minima {
            if (x - mx).abs() < 0.5 && (y - my).abs() < 0.5 {
                found_match = true;
                break;
            }
        }
        assert!(found_match, "Solution ({}, {}) should be close to a known minimum", x, y);
    }
}

#[test]
fn test_himmelblau_function_properties() {
    use ndarray::Array1;
    
    // Test known minima
    let known_minima = [
        (3.0, 2.0),
        (-2.805118, 3.131312),
        (-3.779310, -3.283186),
        (3.584428, -1.848126),
    ];
    
    for &(x, y) in &known_minima {
        let point = Array1::from(vec![x, y]);
        let value = himmelblau(&point);
        assert!(value < 1e-6, "Known minimum ({}, {}) should have f(x) ≈ 0, got {}", x, y, value);
    }
}

#[test]
fn test_de_himmelblau_recorded() {
    let bounds = vec![(-6.0, 6.0), (-6.0, 6.0)];
    let config = DEConfigBuilder::new()
        .seed(789)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "himmelblau", himmelblau, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 1e-2); // Relaxed tolerance for Himmelblau
    
    // Check that solution is close to one of the known minima
    let known_minima = [
        (3.0, 2.0),
        (-2.805118, 3.131312),
        (-3.779310, -3.283186),
        (3.584428, -1.848126),
    ];
    
    let mut found_match = false;
    for &(mx, my) in &known_minima {
        if (report.x[0] - mx).abs() < 0.5 && (report.x[1] - my).abs() < 0.5 {
            found_match = true;
            break;
        }
    }
    assert!(found_match, "Solution ({}, {}) should be close to a known minimum", report.x[0], report.x[1]);
}
