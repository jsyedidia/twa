use twa::{FactorGraph, MessageWeight, WeightedValue};

#[test]
fn diagnostics_expose_topology_and_edge_state_without_mutation() {
    let mut graph = FactorGraph::default();
    let first_variable = graph.create_variable(2.0, MessageWeight::Standard);
    let second_variable = graph.create_variable(4.0, MessageWeight::Standard);
    let first_edge = graph.create_edge(first_variable);
    let second_edge = graph.create_edge(second_variable);
    let orphan_edge = graph.create_edge(first_variable);
    let factor = graph.create_factor(
        &[second_edge, first_edge],
        Box::new(|exchanges, _| {
            exchanges[0].set(WeightedValue::new(20.0, MessageWeight::Standard));
            exchanges[1].set(WeightedValue::new(10.0, MessageWeight::Standard));
        }),
    );

    assert_eq!(
        graph.edges().collect::<Vec<_>>(),
        vec![first_edge, second_edge, orphan_edge]
    );
    assert_eq!(graph.factors().collect::<Vec<_>>(), vec![factor]);
    assert_eq!(
        graph.factor_edges(factor).collect::<Vec<_>>(),
        vec![second_edge, first_edge]
    );

    let initial = graph.edge_diagnostics(first_edge);
    assert_eq!(initial.variable, first_variable);
    assert!(initial.enabled);
    assert_eq!(initial.local_value, 0.0);
    assert_eq!(initial.consensus_value, 2.0);
    assert_eq!(initial.disagreement, 0.0);
    assert_eq!(
        initial.message_to_factor,
        WeightedValue::new(2.0, MessageWeight::Standard)
    );
    assert_eq!(
        initial.message_to_variable,
        WeightedValue::new(0.0, MessageWeight::Zero)
    );
    assert_eq!(initial.message_difference, None);

    assert!(!graph.iterate());
    assert!(graph.iterate());
    let iterations = graph.iterations();
    let converged = graph.converged();
    let diagnostics = graph.edge_diagnostics(first_edge);

    assert_eq!(diagnostics.variable, first_variable);
    assert!(diagnostics.enabled);
    assert_eq!(diagnostics.local_value, 10.0);
    assert_eq!(diagnostics.consensus_value, 10.0);
    assert_eq!(diagnostics.disagreement, 0.0);
    assert_eq!(
        diagnostics.message_to_factor,
        WeightedValue::new(10.0, MessageWeight::Standard)
    );
    assert_eq!(
        diagnostics.message_to_variable,
        WeightedValue::new(10.0, MessageWeight::Standard)
    );
    assert_eq!(diagnostics.message_difference, Some(8.0));
    assert_eq!(graph.iterations(), iterations);
    assert_eq!(graph.converged(), converged);
    assert_eq!(graph.num_edges(), 3);
    assert_eq!(graph.num_factors(), 1);
}

#[test]
#[should_panic(expected = "invalid edge")]
fn diagnostics_reject_an_invalid_edge() {
    let graph = FactorGraph::default();
    graph.edge_diagnostics(Default::default());
}

#[test]
#[should_panic(expected = "invalid factor")]
fn factor_edges_reject_an_invalid_factor() {
    let graph = FactorGraph::default();
    let _ = graph.factor_edges(Default::default());
}
