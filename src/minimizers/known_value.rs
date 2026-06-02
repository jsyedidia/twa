use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};

/// Creates a single-variable factor that always emits `value` with infinite weight.
pub fn create_known_value_factor(
    graph: &mut FactorGraph,
    variable: VariableNode,
    value: f64,
) -> FactorNode {
    let edge = graph.create_edge(variable);

    graph.create_factor(
        &[edge],
        Box::new(move |exchanges, _| {
            exchanges[0].set(WeightedValue::new(value, MessageWeight::Infinite));
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }

    #[test]
    fn known_value_converges_with_infinite_weight() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);

        create_known_value_factor(&mut graph, variable, 3.5);

        assert_eq!(graph.num_edges(), 1);
        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 3.5));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
    }
}
