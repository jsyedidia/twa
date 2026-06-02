# src/minimizers/one_hot.rs

## Role In The System

`one_hot` creates a multi-edge factor that chooses exactly one attached
variable to be `1.0` and drives all others to `0.0`. It is the core constraint
for Sudoku candidate groups and any "pick exactly one" decision.

Related concept notes:

- [notes/concepts/weights.md](../../concepts/weights.md)
- [notes/concepts/message_passing.md](../../concepts/message_passing.md)

## State At A Glance

The minimizer closure captures three reusable scratch vectors:

```rust
let mut infinite_one_indices = Vec::with_capacity(num_variables);
let mut biggest_standard_indices = Vec::with_capacity(num_variables);
let mut biggest_zero_indices = Vec::with_capacity(num_variables);
```

They are cleared on each factor pass and reused to avoid repeated allocation
on the hot path.

## Code Walkthrough

### Imports

```rust
use crate::{
    FactorGraph, FactorNode, MessageWeight, MinimizationFn, VariableNode, WeightedValue,
    WeightedValueExchange,
};
use rand::{Rng, RngCore};
```

The factory builds graph factors and stores the one-hot rule as a
`MinimizationFn`. `Rng` supplies range sampling for random tie-breaking; the
graph passes minimizers a `dyn RngCore`.

### `create_one_hot_factor`

```rust
/// Creates a factor that constrains exactly one variable to be `1.0`.
///
/// All other variables attached to the factor are driven to `0.0`.
///
/// # Panics
///
/// Panics if `variables` is empty.
pub fn create_one_hot_factor(graph: &mut FactorGraph, variables: &[VariableNode]) -> FactorNode {
    assert!(
        !variables.is_empty(),
        "create_one_hot_factor requires at least one variable"
    );

    let edges: Vec<_> = variables
        .iter()
        .map(|&variable| graph.create_edge(variable))
        .collect();

    graph.create_factor(&edges, create_one_hot_minimizer(variables.len()))
}
```

The public helper accepts variables, creates one edge per variable, and
attaches the one-hot minimizer to those edges.

### `create_one_hot_minimizer`

```rust
fn create_one_hot_minimizer(num_variables: usize) -> MinimizationFn {
    let mut infinite_one_indices = Vec::with_capacity(num_variables);
    let mut biggest_standard_indices = Vec::with_capacity(num_variables);
    let mut biggest_zero_indices = Vec::with_capacity(num_variables);
```

The scratch vectors track tie sets for the priority cases. They are allocated
once when the factor is created.

```rust
    Box::new(move |exchanges, random| {
        infinite_one_indices.clear();
        biggest_standard_indices.clear();
        biggest_zero_indices.clear();

        let mut only_non_certain_zero_index = None;
        let mut num_infinite_zero = 0;
        let mut biggest_standard_value = 0.0;
        let mut biggest_zero_value = 0.0;
```

Every factor pass starts with empty tie sets and fresh counters. The
`only_non_certain_zero_index` is meaningful when all but one variables are
certain zero.

```rust
        for (index, exchange) in exchanges.iter().enumerate() {
            let incoming = exchange.get();

            if incoming.weight == MessageWeight::Infinite {
                if incoming.value == 0.0 {
                    num_infinite_zero += 1;
                } else {
                    infinite_one_indices.push(index);
                }
            } else {
                only_non_certain_zero_index = Some(index);

                if incoming.weight == MessageWeight::Zero {
                    if biggest_zero_indices.is_empty() || incoming.value > biggest_zero_value {
                        biggest_zero_value = incoming.value;
                        biggest_zero_indices.clear();
                        biggest_zero_indices.push(index);
                    } else if incoming.value == biggest_zero_value {
                        biggest_zero_indices.push(index);
                    }
                } else if biggest_standard_indices.is_empty()
                    || incoming.value > biggest_standard_value
                {
                    biggest_standard_value = incoming.value;
                    biggest_standard_indices.clear();
                    biggest_standard_indices.push(index);
                } else if incoming.value == biggest_standard_value {
                    biggest_standard_indices.push(index);
                }
            }
        }
```

The scan classifies incoming messages. Infinite nonzero values are treated as
certain "one" candidates. Infinite zero values are counted as unavailable.
Among non-certain messages, standard-weight candidates take priority over
zero-weight candidates, and each group tracks the largest incoming value.

```rust
        let mut outgoing_weight = MessageWeight::Standard;
        let one_index = if !infinite_one_indices.is_empty() {
            outgoing_weight = MessageWeight::Infinite;
            Some(choose_uniform(&infinite_one_indices, random))
        } else if num_infinite_zero == num_variables - 1 {
            outgoing_weight = MessageWeight::Infinite;
            only_non_certain_zero_index
        } else if !biggest_standard_indices.is_empty() {
            Some(choose_uniform(&biggest_standard_indices, random))
        } else if !biggest_zero_indices.is_empty() {
            Some(choose_uniform(&biggest_zero_indices, random))
        } else {
            None
        };
```

The decision priority is:

1. If any incoming value is certainly one, choose among those with infinite
   outgoing weight.
2. If all but one variables are certainly zero, the remaining variable must be
   one with infinite outgoing weight.
3. Otherwise choose the largest standard-weight incoming value.
4. Otherwise choose the largest zero-weight incoming value.

Ties are randomized.

```rust
        let one_index =
            one_index.expect("one_hot constraint has no feasible non-certain-zero variable");
        set_all(exchanges, WeightedValue::new(0.0, outgoing_weight));
        exchanges[one_index].set(WeightedValue::new(1.0, outgoing_weight));
    })
}
```

After choosing the winning exchange, the minimizer emits a full one-hot vector:
all zeros, then one `1.0` at the selected index.

### Helpers

```rust
fn set_all(exchanges: &mut [WeightedValueExchange], weighted_value: WeightedValue) {
    for exchange in exchanges {
        exchange.set(weighted_value);
    }
}
```

`set_all` writes the common zero value to every exchange before the winner is
overwritten with one.

```rust
fn choose_uniform(candidates: &[usize], random: &mut dyn RngCore) -> usize {
    assert!(!candidates.is_empty(), "one_hot tie set is empty");
    let candidate_index = random.random_range(0..candidates.len());
    candidates[candidate_index]
}
```

`choose_uniform` samples one index from the candidate tie set using the graph's
seeded random number generator.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn values(graph: &FactorGraph, variables: &[VariableNode]) -> Vec<f64> {
        variables
            .iter()
            .map(|&variable| graph.value(variable))
            .collect()
    }
```

The `values` helper reads the current graph values for a variable slice.

```rust
    fn tied_one_hot_choice(seed: u64) -> usize {
        let mut graph = FactorGraph::new(1.0, 1e-5, seed);
        let variables = vec![
            graph.create_variable(0.8, MessageWeight::Zero),
            graph.create_variable(0.8, MessageWeight::Zero),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        match values(&graph, &variables).as_slice() {
            [1.0, 0.0] => 0,
            [0.0, 1.0] => 1,
            result => panic!("unexpected one_hot result: {result:?}"),
        }
    }
```

This helper builds a tied two-variable graph and returns which variable won.

```rust
    fn tied_one_hot_choice_after_reseed(seed: u64) -> usize {
        let mut graph = FactorGraph::default();
        graph.set_random_seed(seed);
        let variables = vec![
            graph.create_variable(0.8, MessageWeight::Zero),
            graph.create_variable(0.8, MessageWeight::Zero),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        match values(&graph, &variables).as_slice() {
            [1.0, 0.0] => 0,
            [0.0, 1.0] => 1,
            result => panic!("unexpected one_hot result: {result:?}"),
        }
    }
```

This variant checks the graph's reseeding API.

```rust
    #[test]
    fn seeded_ties_are_reproducible() {
        assert_eq!(tied_one_hot_choice(1234), tied_one_hot_choice(1234));
        assert_eq!(
            tied_one_hot_choice_after_reseed(5678),
            tied_one_hot_choice_after_reseed(5678)
        );
    }
```

The same seed must make the same randomized tie choice.

```rust
    #[test]
    fn prefers_standard_over_zero_weight() {
        let mut graph = FactorGraph::default();
        let variables = vec![
            graph.create_variable(0.9, MessageWeight::Zero),
            graph.create_variable(0.1, MessageWeight::Standard),
            graph.create_variable(0.2, MessageWeight::Zero),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        assert_eq!(values(&graph, &variables), vec![0.0, 1.0, 0.0]);
    }
```

A standard-weight candidate wins over larger zero-weight candidates.

```rust
    #[test]
    fn clear_standard_winner_is_chosen() {
        let mut graph = FactorGraph::default();
        let variables = vec![
            graph.create_variable(0.2, MessageWeight::Standard),
            graph.create_variable(0.9, MessageWeight::Standard),
            graph.create_variable(0.5, MessageWeight::Standard),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        assert_eq!(values(&graph, &variables), vec![0.0, 1.0, 0.0]);
    }
```

Among standard-weight candidates, the largest incoming value wins.

```rust
    #[test]
    fn infinite_one_wins() {
        let mut graph = FactorGraph::default();
        let variables = vec![
            graph.create_variable(0.9, MessageWeight::Standard),
            graph.create_variable(1.0, MessageWeight::Infinite),
            graph.create_variable(0.8, MessageWeight::Standard),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        assert_eq!(values(&graph, &variables), vec![0.0, 1.0, 0.0]);
    }
```

Incoming certainty that a variable is one dominates standard messages.

```rust
    #[test]
    fn all_but_one_infinite_zero_implies_one() {
        let mut graph = FactorGraph::default();
        let variables = vec![
            graph.create_variable(0.0, MessageWeight::Infinite),
            graph.create_variable(0.2, MessageWeight::Standard),
            graph.create_variable(0.0, MessageWeight::Infinite),
        ];

        create_one_hot_factor(&mut graph, &variables);

        assert!(graph.iterate_until_converged(100));
        assert_eq!(values(&graph, &variables), vec![0.0, 1.0, 0.0]);
        assert_eq!(graph.weight(variables[1]), MessageWeight::Infinite);
    }
```

If every other variable is certainly zero, the last possible variable becomes
certainly one.

```rust
    #[test]
    fn preserves_incoming_infinite_zero_weight() {
        let mut graph = FactorGraph::default();
        let variables = vec![
            graph.create_variable(0.0, MessageWeight::Infinite),
            graph.create_variable(0.2, MessageWeight::Standard),
            graph.create_variable(0.8, MessageWeight::Standard),
        ];

        create_one_hot_factor(&mut graph, &variables);

        graph.iterate();
        assert_eq!(values(&graph, &variables), vec![0.0, 0.0, 1.0]);
        assert_eq!(graph.weight(variables[0]), MessageWeight::Infinite);
        assert_eq!(graph.weight(variables[1]), MessageWeight::Standard);
        assert_eq!(graph.weight(variables[2]), MessageWeight::Standard);
    }
```

This protects the certainty-preservation interaction between `FactorData` and
the one-hot minimizer.

```rust
    #[test]
    fn standard_ties_are_randomized() {
        let mut saw_first = false;
        let mut saw_second = false;
        for seed in 0..50 {
            let choice = tied_one_hot_choice(seed);
            saw_first = saw_first || choice == 0;
            saw_second = saw_second || choice == 1;
        }

        assert!(saw_first);
        assert!(saw_second);
    }
```

Across a range of seeds, both tied choices should appear.

```rust
    #[test]
    #[should_panic(expected = "at least one variable")]
    fn rejects_empty_variable_set() {
        let mut graph = FactorGraph::default();
        create_one_hot_factor(&mut graph, &[]);
    }
}
```

An empty one-hot factor is invalid because no variable can be selected as one.

## Important Invariants

- Exactly one outgoing value is `1.0`; every other outgoing value is `0.0`.
- Infinite-one certainty dominates all other candidates.
- All-but-one infinite-zero certainty forces the remaining variable to one.
- Standard-weight candidates are preferred over zero-weight candidates.
- Ties are broken with the graph's seeded RNG.

## Algorithm Context

The one-hot minimizer is the discrete-choice projection used by Sudoku groups:
each cell, row digit, column digit, and box digit must select exactly one
candidate.

## Extension Notes

Keep the scratch vectors captured by the closure rather than allocating new
tie sets on every factor pass. If future profiling changes this logic, preserve
the priority order before tuning data structures.
