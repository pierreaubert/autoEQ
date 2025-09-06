use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy};
use testfunctions::drop_wave;

mod testfunctions;

#[test]
fn test_de_drop_wave() {
    let b = [(-5.12, 5.12), (-5.12, 5.12)];
    let c = DEConfigBuilder::new()
        .seed(72)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Best1Exp)
        .recombination(0.9)
        .build();
    let result = differential_evolution(&drop_wave, &b, c);
    // Drop-wave has global minimum f(x=0, y=0) = -1
    assert!(result.fun < -0.99); // Should find solution very close to -1
    // Check that solution is close to origin
    let norm = (result.x[0].powi(2) + result.x[1].powi(2)).sqrt();
    assert!(norm < 0.1); // Should be very close to (0,0)
}
