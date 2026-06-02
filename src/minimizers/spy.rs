use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};

/// Creates a single-variable factor that observes and optionally replaces messages.
///
/// The callback receives the incoming weighted value. Returning `Some(value)`
/// emits that value. Returning `None` hides the incoming value by re-emitting
/// its scalar value with zero weight.
pub fn create_spy_factor<F>(
    graph: &mut FactorGraph,
    variable: VariableNode,
    mut value_function: F,
) -> FactorNode
where
    F: FnMut(WeightedValue) -> Option<WeightedValue> + 'static,
{
    let edge = graph.create_edge(variable);

    graph.create_factor(
        &[edge],
        Box::new(move |exchanges, _| {
            let incoming = exchanges[0].get();

            if let Some(result) = value_function(incoming) {
                exchanges[0].set(result);
            } else {
                exchanges[0].set(WeightedValue::new(incoming.value, MessageWeight::Zero));
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }

    #[test]
    fn hides_with_zero_weight() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(2.0, MessageWeight::Standard);
        let saw_incoming_value = Rc::new(Cell::new(false));
        let saw_clone = Rc::clone(&saw_incoming_value);

        create_spy_factor(&mut graph, variable, move |incoming| {
            saw_clone.set(nearly_equal(incoming.value, 2.0));
            None
        });

        assert!(graph.iterate_until_converged(100));
        assert!(saw_incoming_value.get());
        assert!(nearly_equal(graph.value(variable), 2.0));
        assert_eq!(graph.weight(variable), MessageWeight::Zero);
    }

    #[test]
    fn emits_user_value() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(2.0, MessageWeight::Standard);

        create_spy_factor(&mut graph, variable, |_| {
            Some(WeightedValue::new(5.0, MessageWeight::Infinite))
        });

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 5.0));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
    }
}
