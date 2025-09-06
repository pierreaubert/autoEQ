use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_bohachevsky1() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky1, &b, c).fun < 1e-4);
}

#[test]
fn test_de_bohachevsky2() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky2, &b, c).fun < 1e-4);
}

#[test]
fn test_de_bohachevsky3() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(31)
        .maxiter(400)
        .popsize(30)
        .build();
    assert!(differential_evolution(&bohachevsky3, &b, c).fun < 1e-4);
}
