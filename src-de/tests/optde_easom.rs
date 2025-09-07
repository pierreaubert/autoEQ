use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::easom;

extern crate blas_src;
#[test]
fn test_de_easom() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let mut c = DEConfig::default();
    c.seed = Some(10);
    c.maxiter = 800;
    c.popsize = 40;
    c.mutation = Mutation::Range { min: 0.5, max: 1.2 };
    c.recombination = 0.95;
    c.strategy = Strategy::Rand1Exp;
    assert!(differential_evolution(&easom, &b, c).fun < -0.9);
}

#[test]
fn test_de_easom_recorded() {
    let bounds = vec![(-100.0, 100.0), (-100.0, 100.0)];
    let config = DEConfigBuilder::new()
        .seed(10)
        .maxiter(800)
        .popsize(40)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    
    let result = run_recorded_differential_evolution(
        "easom", easom, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -0.9);
    
    // Easom has global minimum at (π, π) with f = -1
    let pi = std::f64::consts::PI;
    let dist_to_optimum = ((report.x[0] - pi).powi(2) + (report.x[1] - pi).powi(2)).sqrt();
    assert!(dist_to_optimum < 1.0, "Solution should be reasonably close to (π, π)");
}
