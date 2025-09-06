use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_mccormick() {
    let b = [(-1.5, 4.0), (-3.0, 4.0)];
    let mut c = DEConfig::default();
    c.seed = Some(11);
    c.maxiter = 500;
    c.popsize = 30;
    assert!(differential_evolution(&mccormick, &b, c).fun < -1.7);
}
