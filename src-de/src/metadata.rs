//! Tests for metadata-driven optimization examples

#[cfg(test)]
mod tests {
    use crate::{
        differential_evolution, run_recorded_differential_evolution, DEConfig, DEConfigBuilder,
        Strategy,
    };
    use autoeq_testfunctions::{get_function_bounds_2d, get_function_bounds_vec, rosenbrock};

    /// Example test showing how to use metadata for bounds
    #[test]
    fn test_de_rosenbrock_with_metadata_bounds() {
        // Use metadata bounds instead of hardcoded ones
        let bounds = get_function_bounds_2d("rosenbrock", (-5.0, 5.0)); // fallback if not found

        let mut config = DEConfig::default();
        config.seed = Some(42);
        config.maxiter = 500;
        config.popsize = 40;
        config.recombination = 0.9;
        config.strategy = Strategy::RandToBest1Exp;

        let result = differential_evolution(&rosenbrock, &bounds, config);

        // Rosenbrock function: Global minimum f(x) = 0 at x = (1, 1)
        assert!(result.fun < 1e-3);

        // Check that solution is close to expected optimum
        assert!(
            (result.x[0] - 1.0).abs() < 0.1,
            "x[0] should be close to 1.0: {}",
            result.x[0]
        );
        assert!(
            (result.x[1] - 1.0).abs() < 0.1,
            "x[1] should be close to 1.0: {}",
            result.x[1]
        );

        println!("Used bounds: {:?}", bounds);
        println!(
            "Found solution: ({:.4}, {:.4}) with f = {:.6}",
            result.x[0], result.x[1], result.fun
        );
    }

    #[test]
    fn test_de_rosenbrock_recorded_with_metadata() {
        // Use metadata bounds for recorded optimization
        let bounds = get_function_bounds_vec("rosenbrock", (-5.0, 5.0));
        let config = DEConfigBuilder::new()
            .seed(123)
            .maxiter(400)
            .popsize(30)
            .strategy(Strategy::RandToBest1Exp)
            .recombination(0.9)
            .build();

        let result = run_recorded_differential_evolution(
            "rosenbrock_metadata",
            rosenbrock,
            &bounds,
            config,
        );

        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < 1e-3);

        // Check that solution is close to expected optimum (1, 1)
        assert!(
            (report.x[0] - 1.0).abs() < 0.1,
            "x[0] should be close to 1.0: {}",
            report.x[0]
        );
        assert!(
            (report.x[1] - 1.0).abs() < 0.1,
            "x[1] should be close to 1.0: {}",
            report.x[1]
        );

        println!("Used bounds: {:?}", bounds);
        println!(
            "Found solution: ({:.4}, {:.4}) with f = {:.6}",
            report.x[0], report.x[1], report.fun
        );
    }
}
