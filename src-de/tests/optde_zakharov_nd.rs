use autoeq_de::{differential_evolution, DEConfigBuilder, Strategy};
use autoeq_testfunctions::zakharov;


#[test]
fn test_de_zakharov_2d() {
    // Test 2D Zakharov
    let b2 = vec![(-5.0, 10.0), (-5.0, 10.0)];
    let c2 = DEConfigBuilder::new()
        .seed(52)
        .maxiter(400)
        .popsize(25)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    assert!(differential_evolution(&zakharov, &b2, c2).fun < 1e-4);
}

#[test]
fn test_de_zakharov_10d() {
    // Test 10D Zakharov
    let b10 = vec![(-5.0, 10.0); 10];
    let c10 = DEConfigBuilder::new()
        .seed(53)
        .maxiter(800)
        .popsize(60)
        .strategy(Strategy::Best1Exp)
        .recombination(0.95)
        .build();
    assert!(differential_evolution(&zakharov, &b10, c10).fun < 1e-3);
}
