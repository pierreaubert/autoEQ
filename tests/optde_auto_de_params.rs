use autoeq::optde::*;
use autoeq::optim::AutoDEParams;
use common::*;

mod common;

// Tests for auto_de parameter handling and validation

#[test]
fn test_auto_de_custom_parameters() {
    // Test with custom parameters
    let bounds = create_bounds(2, -5.0, 5.0);
    
    let params = AutoDEParams {
        max_iterations: 500,
        population_size: None,  // Will use default based on dimension
        f: 0.7,                 // Mutation factor
        cr: 0.8,                // Crossover probability  
        tolerance: 1e-8,
        seed: Some(12345),
    };
    
    let result = auto_de(quadratic, &bounds, Some(params));
    
    assert!(result.is_some(), "AutoDE should find a solution with custom params");
    let (x_opt, f_opt, iterations) = result.unwrap();
    
    // Should still find the optimum
    assert!(f_opt < 1e-6, "Function value too high with custom params: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-3, "Solution component too far from 0: {}", xi);
    }
    
    // Should use specified max iterations
    assert!(iterations <= 500, "Used more iterations than specified: {}", iterations);
}

#[test]
fn test_auto_de_parameter_validation() {
    let bounds = create_bounds(2, -5.0, 5.0);
    
    // Test invalid mutation factor
    let invalid_params = AutoDEParams {
        max_iterations: 100,
        population_size: None,
        f: 2.5,  // Invalid: should be in [0, 2]
        cr: 0.5,
        tolerance: 1e-6,
        seed: None,
    };
    
    let result = auto_de(quadratic, &bounds, Some(invalid_params));
    assert!(result.is_none(), "Should fail with invalid mutation factor");
    
    // Test invalid crossover probability
    let invalid_params2 = AutoDEParams {
        max_iterations: 100,
        population_size: None,
        f: 0.5,
        cr: 1.5,  // Invalid: should be in [0, 1]
        tolerance: 1e-6,
        seed: None,
    };
    
    let result2 = auto_de(quadratic, &bounds, Some(invalid_params2));
    assert!(result2.is_none(), "Should fail with invalid crossover probability");
}

#[test]
fn test_auto_de_convergence_tolerance() {
    let bounds = create_bounds(2, -5.0, 5.0);
    
    // Test with loose tolerance - should converge faster
    let loose_params = AutoDEParams {
        max_iterations: 1000,
        population_size: None,
        f: 0.5,
        cr: 0.7,
        tolerance: 1e-2,  // Loose tolerance
        seed: Some(42),
    };
    
    let result = auto_de(quadratic, &bounds, Some(loose_params));
    assert!(result.is_some());
    let (_, f_opt, iterations) = result.unwrap();
    
    // Should converge with loose tolerance
    assert!(f_opt < 1e-2, "Function value should meet loose tolerance");
    
    // Test with tight tolerance  
    let tight_params = AutoDEParams {
        max_iterations: 1000,
        population_size: None,
        f: 0.5,
        cr: 0.7,
        tolerance: 1e-10,  // Very tight tolerance
        seed: Some(42),
    };
    
    let result2 = auto_de(quadratic, &bounds, Some(tight_params));
    if let Some((_, f_opt2, iterations2)) = result2 {
        // If it converges, should meet tight tolerance
        assert!(f_opt2 < 1e-8, "Function value should meet tight tolerance");
        // Might take more iterations
        assert!(iterations2 >= iterations, "Tight tolerance should take more iterations");
    }
    // If it doesn't converge within max_iterations, that's also acceptable
}

#[test]
fn test_auto_de_reproducibility() {
    // Test that same seed gives same results
    let bounds = create_bounds(3, -2.0, 2.0);
    
    let params = AutoDEParams {
        max_iterations: 200,
        population_size: Some(30),
        f: 0.6,
        cr: 0.8,
        tolerance: 1e-6,
        seed: Some(98765),
    };
    
    let result1 = auto_de(quadratic, &bounds, Some(params.clone()));
    let result2 = auto_de(quadratic, &bounds, Some(params));
    
    assert!(result1.is_some() && result2.is_some(), "Both runs should succeed");
    let (x1, f1, iter1) = result1.unwrap();
    let (x2, f2, iter2) = result2.unwrap();
    
    // Same seed should give same results
    assert!((f1 - f2).abs() < 1e-12, "Function values should be identical: {} vs {}", f1, f2);
    assert_eq!(iter1, iter2, "Iteration counts should be identical");
    for (i, (a, b)) in x1.iter().zip(x2.iter()).enumerate() {
        assert!((a - b).abs() < 1e-12, "Solution components should be identical: x[{}] = {} vs {}", i, a, b);
    }
}

#[test]
fn test_auto_de_invalid_bounds() {
    use ndarray::Array2;
    
    // Test with invalid bounds (lower > upper)
    let mut bounds = Array2::zeros((2, 2));
    bounds[[0, 0]] = 5.0;  bounds[[1, 0]] = 1.0;  // Invalid: 5 > 1
    bounds[[0, 1]] = -1.0; bounds[[1, 1]] = 1.0;  // Valid: -1 < 1
    
    let result = auto_de(quadratic, &bounds, None);
    assert!(result.is_none(), "Should fail with invalid bounds");
}

#[test]
fn test_auto_de_empty_bounds() {
    use ndarray::Array2;
    
    // Test with empty bounds
    let bounds = Array2::zeros((2, 0));
    let result = auto_de(quadratic, &bounds, None);
    assert!(result.is_none(), "Should fail with empty bounds");
}

#[test]
fn test_auto_de_default_parameters() {
    // Test that default parameters work correctly
    let bounds = create_bounds(3, -5.0, 5.0);
    let result = auto_de(quadratic, &bounds, None);
    
    assert!(result.is_some(), "AutoDE should work with default parameters");
    let (x_opt, f_opt, _) = result.unwrap();
    
    assert!(f_opt < 1e-6, "Should find good solution with defaults: {}", f_opt);
    for &xi in x_opt.iter() {
        assert!(xi.abs() < 1e-2, "Solution component should be close to 0: {}", xi);
    }
}

#[test]
fn test_auto_de_population_size_scaling() {
    let bounds = create_bounds(2, -5.0, 5.0);
    
    // Test explicit small population
    let small_pop_params = AutoDEParams {
        max_iterations: 100,
        population_size: Some(10),  // Small population
        f: 0.8,
        cr: 0.9,
        tolerance: 1e-6,
        seed: Some(111),
    };
    
    let result1 = auto_de(quadratic, &bounds, Some(small_pop_params));
    assert!(result1.is_some(), "Should work with small population");
    
    // Test explicit large population  
    let large_pop_params = AutoDEParams {
        max_iterations: 100,
        population_size: Some(100), // Large population
        f: 0.8,
        cr: 0.9,
        tolerance: 1e-6,
        seed: Some(111),
    };
    
    let result2 = auto_de(quadratic, &bounds, Some(large_pop_params));
    assert!(result2.is_some(), "Should work with large population");
    
    // Both should find good solutions
    let (_, f1, _) = result1.unwrap();
    let (_, f2, _) = result2.unwrap();
    assert!(f1 < 1e-4 && f2 < 1e-4, "Both should find good solutions");
}
