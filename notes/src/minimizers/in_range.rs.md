# src/minimizers/in_range.rs

## Role In The System

`in_range` creates a single-edge factor that clamps a variable into a closed
interval. It has no opinion when the incoming value already satisfies the
constraint.

Related concept notes:

- [notes/concepts/weights.md](../../concepts/weights.md)
- [notes/concepts/message_passing.md](../../concepts/message_passing.md)

## Code Walkthrough

### Imports

```rust
use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};
```

The factory builds a factor graph edge and emits weighted values during the
factor pass.

### `create_in_range_factor`

```rust
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
```

The bounds check catches invalid intervals at construction. During iteration,
values below or above the interval are projected to the nearest bound with
standard weight. Values already inside the interval are returned with zero
weight so other factors can decide the variable.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }
```

The tests compare converged floating-point values with a small tolerance.

```rust
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
```

This covers both projections: below to `lower` and above to `upper`.

```rust
    #[test]
    fn has_no_opinion_inside_range() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.25, MessageWeight::Standard);

        create_in_range_factor(&mut graph, variable, 0.0, 1.0);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 0.25));
        assert_eq!(graph.weight(variable), MessageWeight::Zero);
    }
```

An already-valid value is preserved, and the factor ends with zero weight.

```rust
    #[test]
    fn preserves_certain_initial_values_inside_range() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.75, MessageWeight::Infinite);

        create_in_range_factor(&mut graph, variable, 0.0, 1.0);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 0.75));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
    }
```

Factor-side certainty preservation keeps an incoming infinite weight from
being downgraded when the certain value is inside range.

```rust
    #[test]
    #[should_panic(expected = "lower <= upper")]
    fn rejects_inverted_bounds() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);

        create_in_range_factor(&mut graph, variable, 1.0, 0.0);
    }
}
```

The factory rejects inverted bounds immediately.

## Important Invariants

- Inside-range messages are zero weight.
- Outside-range messages clamp to the nearest bound with standard weight.
- Incoming infinite weight can still be preserved by `FactorData`.

## Algorithm Context

This is the basic projection factor for interval constraints. It will be reused
by circle-packing boundaries and other bounded variables.

## Extension Notes

Keep the interval closed: values exactly equal to `lower` or `upper` are inside
the range and therefore emit zero weight.
