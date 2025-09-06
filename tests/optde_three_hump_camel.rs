use autoeq::optde::{differential_evolution, DEConfig, Strategy};
use testfunctions::three_hump_camel;

mod testfunctions;

#[test]
fn test_de_three_hump_camel() {
    let b = [(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(8);
    c.maxiter = 300;
    c.popsize = 20;
    c.strategy = Strategy::Best1Exp;
    assert!(differential_evolution(&three_hump_camel, &b, c).fun < 1e-6);
}
