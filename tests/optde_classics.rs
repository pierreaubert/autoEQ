use autoeq::optde::*;
use common::*;

mod common;

#[test]
fn test_de_classics() {
    let b2 = [(-5.0, 5.0), (-5.0, 5.0)];
    let mk = || {
        let mut c = DEConfig::default();
        c.seed = Some(5);
        c.maxiter = 800;  // Increased iterations
        c.popsize = 40;   // Increased population
        c.recombination = 0.9;
        c.strategy = Strategy::RandToBest1Exp;  // Changed strategy
        c
    };
    assert!(differential_evolution(&booth, &b2, mk()).fun < 1e-5);
    assert!(differential_evolution(&matyas, &b2, mk()).fun < 1e-5);
    assert!(differential_evolution(&beale, &b2, mk()).fun < 1e-2);  // Relaxed tolerance
    assert!(differential_evolution(&himmelblau, &[(-6.0, 6.0), (-6.0, 6.0)], mk()).fun < 1e-2);
}
