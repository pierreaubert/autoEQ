//! Tests for adaptive differential evolution strategies

#[cfg(test)]
mod tests {
    use crate::{differential_evolution, DEConfigBuilder, Strategy, Mutation, AdaptiveConfig};
    use autoeq_testfunctions::quadratic;

    extern crate blas_src;

    #[test] 
    fn test_adaptive_basic() {
        // Test basic adaptive DE functionality 
        let bounds = [(-5.0, 5.0), (-5.0, 5.0)];
        
        // Configure adaptive DE with SAM approach
        let adaptive_config = AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: false, // Start with mutation only
            w_max: 0.9,
            w_min: 0.1,
            ..AdaptiveConfig::default()
        };
        
        let config = DEConfigBuilder::new()
            .seed(42)
            .maxiter(100)
            .popsize(30)
            .strategy(Strategy::AdaptiveBin)
            .mutation(Mutation::Adaptive { initial_f: 0.8 })
            .adaptive(adaptive_config)
            .build();
        
        let result = differential_evolution(&quadratic, &bounds, config);
        
        // Should converge to global minimum at (0, 0)
        assert!(result.fun < 1e-3, "Adaptive DE should converge: f={}", result.fun);
        
        // Check that solution is close to expected optimum
        for &xi in result.x.iter() {
            assert!(xi.abs() < 0.5, "Solution component should be close to 0: {}", xi);
        }
    }

    #[test]
    fn test_adaptive_with_wls() {
        // Test adaptive DE with Wrapper Local Search
        let bounds = [(-5.0, 5.0), (-5.0, 5.0)];
        
        let adaptive_config = AdaptiveConfig {
            adaptive_mutation: true,
            wls_enabled: true,
            wls_prob: 0.2, // Apply WLS to 20% of population
            wls_scale: 0.1,
            ..AdaptiveConfig::default()
        };
        
        let config = DEConfigBuilder::new()
            .seed(123)
            .maxiter(200)
            .popsize(40) 
            .strategy(Strategy::AdaptiveExp)
            .adaptive(adaptive_config)
            .build();
        
        let result = differential_evolution(&quadratic, &bounds, config);
        
        // Should converge even better with WLS
        assert!(result.fun < 1e-4, "Adaptive DE with WLS should converge well: f={}", result.fun);
        
        for &xi in result.x.iter() {
            assert!(xi.abs() < 0.2, "Solution should be very close to 0 with WLS: {}", xi);
        }
    }
}
