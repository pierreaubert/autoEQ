use autoeq_de::{differential_evolution, DEConfigBuilder, Mutation, Strategy};
use autoeq_testfunctions::{mishras_bird_constraint, mishras_bird_objective};

#[test]
fn test_de_constrained_mishras_bird() {
    let b = vec![(-10.0, 0.0), (-6.5, 0.0)];
    let c = DEConfigBuilder::new()
        .seed(57)
        .maxiter(1500)
        .popsize(80)
        .strategy(Strategy::Rand1Exp)
        .recombination(0.95)
        .mutation(Mutation::Range { min: 0.5, max: 1.2 })
        .add_penalty_ineq(Box::new(mishras_bird_constraint), 1e6)
        .build();
    let result = differential_evolution(&mishras_bird_objective, &b, c);
    // Check that solution respects constraint (x+5)^2 + (y+5)^2 <= 25
    let constraint_value = (result.x[0] + 5.0).powi(2) + (result.x[1] + 5.0).powi(2);
    assert!(constraint_value <= 25.1); // Should be inside circle
    assert!(result.fun < -50.0); // Should find good solution within constraint
}
