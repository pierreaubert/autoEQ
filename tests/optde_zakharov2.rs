use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_zakharov2() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(22)
        .maxiter(300)
        .popsize(25)
        .build();
    assert!(differential_evolution(&zakharov2, &b, c).fun < 1e-4);
}
