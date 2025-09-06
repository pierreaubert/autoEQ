use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_goldstein_price() {
    let b = [(-2.0, 2.0), (-2.0, 2.0)];
    let mut c = DEConfig::default();
    c.seed = Some(7);
    c.maxiter = 600;
    c.popsize = 30;
    c.strategy = Strategy::Rand1Exp;
    assert!(differential_evolution(&goldstein_price, &b, c).fun < 3.01);
}
