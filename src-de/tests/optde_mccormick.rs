use autoeq_de::{differential_evolution, DEConfig, DEConfigBuilder, Strategy, run_recorded_differential_evolution};
use autoeq_testfunctions::mccormick;

extern crate blas_src;
#[test]
fn test_de_mccormick() {
    let b = [(-1.5, 4.0), (-3.0, 4.0)];
    let mut c = DEConfig::default();
    c.seed = Some(11);
    c.maxiter = 500;
    c.popsize = 30;
    assert!(differential_evolution(&mccormick, &b, c).fun < -1.7);
}

#[test]
fn test_de_mccormick_recorded() {
    let bounds = vec![(-1.5, 4.0), (-3.0, 4.0)];
    let config = DEConfigBuilder::new()
        .seed(11)
        .maxiter(500)
        .popsize(30)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .build();
    
    let result = run_recorded_differential_evolution(
        "mccormick", mccormick, &bounds, config, "./data_generated/records"
    );
    
    assert!(result.is_ok());
    let (report, _csv_path) = result.unwrap();
    assert!(report.fun < -1.7);
    
    // McCormick has global minimum f = -1.9133 at (-0.54719, -1.54719)
    assert!(report.x[0] >= -1.5 && report.x[0] <= 4.0);
    assert!(report.x[1] >= -3.0 && report.x[1] <= 4.0);
}
