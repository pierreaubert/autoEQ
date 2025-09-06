use autoeq::optde::{differential_evolution, DEConfigBuilder, Strategy};
use testfunctions::{binh_korn_constraint1, binh_korn_constraint2, binh_korn_weighted};
mod testfunctions;

#[test]
fn test_de_constrained_binh_korn() {
    // Test Binh-Korn constrained multi-objective problem as single objective
    let b = vec![(0.0, 5.0), (0.0, 3.0)];
    let c = DEConfigBuilder::new()
        .seed(59)
        .maxiter(1200)
        .popsize(60)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.9)
        .add_penalty_ineq(Box::new(binh_korn_constraint1), 1e6)
        .add_penalty_ineq(Box::new(binh_korn_constraint2), 1e6)
        .build();
    let result = differential_evolution(&binh_korn_weighted, &b, c);

    // Check constraints
    let g1 = (result.x[0] - 5.0).powi(2) + result.x[1].powi(2);
    let g2 = (result.x[0] - 8.0).powi(2) + (result.x[1] + 3.0).powi(2);
    assert!(g1 <= 25.1); // g1 <= 25
    assert!(g2 >= 7.6); // g2 >= 7.7
    assert!(result.fun < 50.0); // Should find reasonable objective value
}
