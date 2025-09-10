use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy};
use autoeq_testfunctions::{de_jong_step2, dejong_f5_foxholes};
use ndarray::Array1;

#[test]
fn test_de_dejong_sphere() {
    let b10 = vec![(-5.12, 5.12); 10];
    let c = DEConfigBuilder::new()
        .seed(34)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    // f1 sphere 10D
    let f1 = |x: &Array1<f64>| x.iter().map(|v| v * v).sum::<f64>();
    assert!(differential_evolution(&f1, &b10, c).fun < 1e-3);
}

#[test]
fn test_de_dejong_step() {
    let b10 = vec![(-5.12, 5.12); 10];
    let c = DEConfigBuilder::new()
        .seed(34)
        .maxiter(800)
        .popsize(50)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.9)
        .build();
    // f3 step 10D
    assert!(differential_evolution(&de_jong_step2, &b10, c).fun <= 10.0);
}

#[test]
fn test_de_dejong_foxholes() {
    let bfox = [(-65.536, 65.536), (-65.536, 65.536)];
    let cfgf5 = DEConfigBuilder::new()
        .seed(35)
        .maxiter(1500)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    assert!(differential_evolution(&dejong_f5_foxholes, &bfox, cfgf5).fun < 1.0);
}
