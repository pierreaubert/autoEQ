use autoeq::optde::{differential_evolution, DEConfig, Mutation};
use testfunctions::styblinski_tang2;

mod testfunctions;

#[test]
fn test_de_styblinski_tang() {
    let b = [(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(13);
    c.maxiter = 800;
    c.popsize = 30;
    c.mutation = Mutation::Range { min: 0.5, max: 1.2 };
    assert!(differential_evolution(&styblinski_tang2, &b, c).fun < -70.0);
}
