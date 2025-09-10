use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy};
use autoeq_testfunctions::{
    keanes_bump_constraint1, keanes_bump_constraint2, keanes_bump_objective,
};

#[test]
fn test_de_constrained_keanes_bump() {
    // Test 2D Keane's bump function
    let b = vec![(0.1, 9.9), (0.1, 9.9)];
    let c = DEConfigBuilder::new()
        .seed(58)
        .maxiter(2000)
        .popsize(100)
        .strategy(Strategy::RandToBest1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.3, max: 1.0 })
        .add_penalty_ineq(Box::new(keanes_bump_constraint1), 1e6)
        .add_penalty_ineq(Box::new(keanes_bump_constraint2), 1e6)
        .build();
    let result = differential_evolution(&keanes_bump_objective, &b, c);
    // Check constraints: product > 0.75 and sum < 15.0 (for 2D)
    let product = result.x.iter().product::<f64>();
    let sum = result.x.iter().sum::<f64>();
    assert!(product > 0.749); // Should satisfy product constraint
    assert!(sum < 15.1); // Should satisfy sum constraint
    assert!(result.fun < -0.1); // Should find feasible solution with negative objective
}
