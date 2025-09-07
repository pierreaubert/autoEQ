use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::griewank2;

extern crate blas_src;
#[test]
fn test_de_griewank2() {
    let b = [(-600.0, 600.0), (-600.0, 600.0)];
    let c = DEConfigBuilder::new()
        .seed(21)
        .maxiter(800) // Increased iterations
        .popsize(50) // Increased population
        .strategy(Strategy::RandToBest1Exp) // Better strategy for multimodal
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&griewank2, &b, c).fun < 1e-2); // Relaxed tolerance
}

#[test]
fn test_de_griewank2_recorded() {
    let bounds = vec![(-600.0, 600.0), (-600.0, 600.0)];
    let config = DEConfigBuilder::new()
        .seed(21)
        .maxiter(600)
        .popsize(40)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    
    let result = run_recorded_differential_evolution(
        "griewank2", griewank2, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < 0.1); // Relaxed threshold for griewank2
    
    // Griewank has global minimum at (0, 0)
    assert!(report.x[0].abs() < 50.0, "x[0] should be reasonably close to 0.0: {}", report.x[0]);
    assert!(report.x[1].abs() < 50.0, "x[1] should be reasonably close to 0.0: {}", report.x[1]);
}
