# src/minimizers/spy.rs

## Role In The System

`spy` creates a single-edge factor that exposes incoming messages to a user
callback. The callback can inspect the message and either replace it or let it
pass through as a hidden zero-weight observation.

Related concept notes:

- [notes/concepts/weights.md](../../concepts/weights.md)
- [notes/concepts/message_passing.md](../../concepts/message_passing.md)

## Code Walkthrough

### Imports

```rust
use crate::{FactorGraph, FactorNode, MessageWeight, VariableNode, WeightedValue};
```

The factory creates the factor and uses `WeightedValue` for callback input and
output.

### `create_spy_factor`

```rust
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
```

The callback is `FnMut` so it can record state between calls. Returning `None`
keeps the scalar value but changes its weight to zero, which makes the spy an
observer rather than a constraint.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }
```

The first spy test shares state with the callback through `Rc<Cell<bool>>`.

```rust
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
```

This verifies that the callback sees the incoming value and that `None` emits a
zero-weight message.

```rust
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
```

Returning `Some` lets the callback act as a custom one-edge minimizer.

## Important Invariants

- `None` means zero weight, not "leave the previous factor output untouched."
- The callback receives the incoming variable-to-factor message.

## Algorithm Context

Spy factors are useful for inspection and custom lightweight constraints. They
participate in the same factor pass as built-in minimizers.

## Extension Notes

If a future API needs borrowed callbacks, it will require a lifetime-carrying
graph or a separate adapter. The current `'static` bound keeps `FactorGraph`
simple because it stores minimizers as boxed trait objects.
