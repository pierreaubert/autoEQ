use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy};
use testfunctions::{schaffer_n2, schaffer_n4};

mod testfunctions;

#[test]
fn test_de_schaffer_n2() {
    let b = [(-100.0, 100.0), (-100.0, 100.0)];
    let c = DEConfigBuilder::new()
        .seed(25)
        .maxiter(300)
        .popsize(25)
        .strategy(Strategy::Best1Exp)
        .build();
    assert!(differential_evolution(&schaffer_n2, &b, c).fun < 1e-3);
}

#[test]
fn test_de_schaffer_n4() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(32)
        .maxiter(800)
        .popsize(35)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&schaffer_n4, &b, c).fun < 0.35);
}
