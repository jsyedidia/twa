use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};

/// Creates a single-variable factor that constrains a value to `[lower, upper]`.
///
/// The factor emits zero weight when the incoming value is already inside the
/// range, and standard weight when it must clamp to a bound.
///
/// # Panics
///
/// Panics if `upper < lower`.
pub fn create_in_range_factor(
    graph: &mut FactorGraph,
    variable: VariableNode,
    lower: f64,
    upper: f64,
) -> FactorNode {
    assert!(
        lower <= upper,
        "create_in_range_factor requires lower <= upper"
    );

    let edge = graph.create_edge(variable);

    graph.create_factor(
        &[edge],
        Box::new(move |exchanges, _| {
            let incoming = exchanges[0].get();

            if incoming.value < lower {
                exchanges[0].set(WeightedValue::new(lower, MessageWeight::Standard));
            } else if incoming.value > upper {
                exchanges[0].set(WeightedValue::new(upper, MessageWeight::Standard));
            } else {
                exchanges[0].set(WeightedValue::new(incoming.value, MessageWeight::Zero));
            }
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
    fn clamps_below_and_above() {
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(-10.0, MessageWeight::Standard);

            create_in_range_factor(&mut graph, variable, 0.0, 1.0);

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 0.0));
        }

        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(10.0, MessageWeight::Standard);

            create_in_range_factor(&mut graph, variable, 0.0, 1.0);

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 1.0));
        }
    }

    #[test]
    fn has_no_opinion_inside_range() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.25, MessageWeight::Standard);

        create_in_range_factor(&mut graph, variable, 0.0, 1.0);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 0.25));
        assert_eq!(graph.weight(variable), MessageWeight::Zero);
    }

    #[test]
    fn preserves_certain_initial_values_inside_range() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.75, MessageWeight::Infinite);

        create_in_range_factor(&mut graph, variable, 0.0, 1.0);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 0.75));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
    }

    #[test]
    #[should_panic(expected = "lower <= upper")]
    fn rejects_inverted_bounds() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);

        create_in_range_factor(&mut graph, variable, 1.0, 0.0);
    }
}
