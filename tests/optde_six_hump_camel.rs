use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_six_hump_camel() {
    let b = [(-3.0, 3.0), (-2.0, 2.0)];
    let mut c = DEConfig::default();
    c.seed = Some(9);
    c.maxiter = 500;
    c.popsize = 30;
    c.strategy = Strategy::RandToBest1Exp;
    assert!(differential_evolution(&six_hump_camel, &b, c).fun < -1.0);
}
