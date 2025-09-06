use autoeq::optde::*;
use common::*;

mod common;

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
