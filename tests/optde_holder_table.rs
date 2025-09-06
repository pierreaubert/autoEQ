use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy, Mutation};
use testfunctions::holder_table;

mod testfunctions;

#[test]
fn test_de_holder_table() {
    let b = [(-10.0, 10.0), (-10.0, 10.0)];
    let c = DEConfigBuilder::new()
        .seed(73)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.4, max: 1.2 })
        .build();
    let result = differential_evolution(&holder_table, &b, c);
    // Holder table has global minimum f(x) = -19.2085 at (±8.05502, ±9.66459)
    assert!(result.fun < -18.0); // Should find solution close to global minimum
}
