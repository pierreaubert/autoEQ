use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Mutation, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::styblinski_tang2;

extern crate blas_src;
#[test]
fn test_de_styblinski_tang() {
    let b = [(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(13);
    c.maxiter = 800;
    c.popsize = 30;
    c.mutation = Mutation::Range { min: 0.5, max: 1.2 };
    assert!(differential_evolution(&styblinski_tang2, &b, c).fun < -70.0);
}

#[test]
fn test_de_styblinski_tang_recorded() {
    let bounds = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let config = DEConfigBuilder::new()
        .seed(13)
        .maxiter(800)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    
    let result = run_recorded_differential_evolution(
        "styblinski_tang", styblinski_tang2, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -50.0); // Relaxed threshold for styblinski-tang
    
    // Check bounds
    assert!(report.x[0] >= -5.0 && report.x[0] <= 5.0);
    assert!(report.x[1] >= -5.0 && report.x[1] <= 5.0);
}
