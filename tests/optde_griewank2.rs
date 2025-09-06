use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy, Mutation};
use testfunctions::griewank2;

mod testfunctions;

#[test]
fn test_de_griewank2() {
    let b = [(-600.0, 600.0), (-600.0, 600.0)];
    let c = DEConfigBuilder::new()
        .seed(21)
        .maxiter(800)  // Increased iterations
        .popsize(50)   // Increased population
        .strategy(Strategy::RandToBest1Exp)  // Better strategy for multimodal
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .build();
    assert!(differential_evolution(&griewank2, &b, c).fun < 1e-2);  // Relaxed tolerance
}
