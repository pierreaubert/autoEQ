use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_levi13() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let mut c = DEConfig::default();
    c.seed = Some(12);
    c.maxiter = 600;
    c.popsize = 25;
    assert!(differential_evolution(&levi13, &b, c).fun < 1e-3);
}
