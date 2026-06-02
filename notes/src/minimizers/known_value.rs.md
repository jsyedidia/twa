# src/minimizers/known_value.rs

## Role In The System

`known_value` creates a single-edge factor that always asserts a fixed scalar
value with infinite weight. It is used for givens and other hard assignments.

Related concept notes:

- [notes/concepts/weights.md](../../concepts/weights.md)
- [notes/concepts/message_passing.md](../../concepts/message_passing.md)

## Code Walkthrough

### Imports

```rust
use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};
```

The factory creates an edge and factor in a `FactorGraph`, returns a
`FactorNode`, and writes `WeightedValue` messages with infinite weight.

### `create_known_value_factor`

```rust
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
```

The helper creates one graph edge for the variable and attaches a minimizer
closure. Every factor pass overwrites the exchange entry with the fixed value
and `Infinite` weight.

### Tests

```rust
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
```

The test verifies the factory creates one edge and drives the variable to the
known value with certainty.

## Important Invariants

- The minimizer always emits infinite weight.
- The factory owns edge creation so callers pass variables, not edge handles.

## Algorithm Context

Known values are hard constraints. In the variable pass, infinite weight wins
over standard and zero-weight messages.

## Extension Notes

Keep this factor single-edge. Multi-variable fixed patterns should be modeled
as separate factors or a dedicated minimizer.
