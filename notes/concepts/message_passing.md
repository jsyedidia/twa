# Message Passing

This note explains the message-passing model implemented by this crate. It
connects the mathematical picture from the paper to the code-level objects:
variables, factors, edges, weighted values, minimizers, and graph iterations.

## The Big Picture

The solver represents an optimization problem as a factor graph.

There are two kinds of nodes:

- Variable nodes, which hold the current shared belief for a quantity.
- Factor nodes, which represent local constraints or objective terms.

An edge connects one factor to one variable. The same quantity, but possibly
with temporarily different estimates, is represented in two local places:

- The factor's local copy, called `x` in the code.
- The variable's shared consensus value, called `z` in the code.

The algorithm repeatedly asks:

1. Given the current messages from the variables, what does each factor want?
2. Given the current messages from the factors, what consensus should each
   variable enforce?
3. How much did the messages change?

When the enabled edge messages stop changing beyond the convergence threshold,
the graph is considered converged.

## What An Edge Stores

Each edge stores the state needed to pass messages in both directions. The core
scalar values are:

```text
x: the factor-side local value
z: the variable-side consensus value
u: the accumulated disagreement (dual variable)
```

The edge does not store independent message values. It computes them from `x`,
`z`, and `u`.

## The Two Message Directions

The message sent from a variable to a factor is:

```text
n = z - u
```

The message sent from a factor to a variable is:

```text
m = x + u
```

These formulas come from the ADMM (Alternating Direction Method of Multipliers)
framework. The `u` variable accumulates disagreement between the factor's local
preference (`x`) and the variable's consensus (`z`).

## The Iteration Loop

Each call to `iterate()` performs these steps:

### 1. Factor Pass

For each enabled factor:

- Gather the incoming messages from all connected edges: `n = z - u` for each
  edge.
- Package these as `WeightedValueExchange` entries with their weights.
- Call the factor's minimization function.
- The minimizer writes its preferred values back into the exchange entries.
- Update each edge's `x` and outgoing weight from the minimizer's output.

### 2. Variable Pass

For each variable:

- Gather the incoming messages from all connected edges: `m = x + u` with
  weights.
- Compute the weighted consensus value `z`:
  - If any message has infinite weight, `z` takes that value (certainty wins).
  - Otherwise, `z` is the weighted average of standard-weight messages.
  - Zero-weight messages are ignored when standard-weight messages exist.
- Update `z` on all connected edges (blended with learning rate).
- Update `u` on each edge: `u += x - z`.

### 3. Convergence Check

For each enabled edge, check whether the message change is below
`convergence_delta`. If all enabled edges satisfy this, the graph has converged
and iteration stops early.

## Learning Rate

The variable update blends the old `z` with the new consensus:

```text
z_new = z_old + learning_rate * (consensus - z_old)
```

A learning rate of 1.0 means immediate adoption of the new consensus. Smaller
values provide damping, which can help convergence on difficult problems.

## Convergence

The graph reports convergence when all enabled edges have
`|message_change| < convergence_delta`. This means every factor and every
variable agree (within tolerance) on the value passing through each edge.

In practice, some problems may not converge. The caller should set a maximum
iteration count as a safeguard.

## Where The Pieces Live

The message-passing loop is split across a small set of source files:

- [src/factor_graph.rs.md](../src/factor_graph.rs.md) explains
  `FactorGraph::iterate()`, `iterate_until_converged()`,
  `enforce_variable_equality()`, convergence checking, and dynamic factor
  callbacks.
- [src/edge_data.rs.md](../src/edge_data.rs.md) explains `x`, `z`, `u`,
  `message_to_factor()`, `message_to_variable()`,
  `set_result_from_factor()`, and `set_result_from_variable()`.
- [src/factor_data.rs.md](../src/factor_data.rs.md) explains how a factor
  fills `WeightedValueExchange` entries, calls its minimizer, and preserves
  incoming infinite weights.
- [src/variable_data.rs.md](../src/variable_data.rs.md) explains the
  variable-side current value, current weight, connected-edge list, and lazy
  enabled-edge cache.
- [src/weighted_value.rs.md](../src/weighted_value.rs.md) explains
  `MessageWeight`, `WeightedValue`, `WeightedValueExchange`, and the
  minimizer function type.

The built-in minimizers show how specific factors participate in the same
message protocol:

- [src/minimizers/known_value.rs.md](../src/minimizers/known_value.rs.md)
  always emits infinite-weight certainty.
- [src/minimizers/in_range.rs.md](../src/minimizers/in_range.rs.md) emits
  zero weight when a scalar is already inside a range and standard weight when
  it must clamp.
- [src/minimizers/one_hot.rs.md](../src/minimizers/one_hot.rs.md) chooses one
  connected variable to be `1.0` and the others to be `0.0`.
- [src/minimizers/spy.rs.md](../src/minimizers/spy.rs.md) lets callers observe
  or override messages through a callback.

For a concrete graph builder that creates many factors at once, read
[sudoku.md](sudoku.md) with
[src/problems/sudoku.rs.md](../src/problems/sudoku.rs.md), or
[circle_packing.md](circle_packing.md) with
[src/problems/circle_packing.rs.md](../src/problems/circle_packing.rs.md).
