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
3. Did the variable beliefs stop changing?

When every variable belief value changes by at most the convergence threshold,
the graph is considered converged. Callers can also add domain-specific
satisfaction checks for problems where stable beliefs are necessary but not
quite enough.

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
- Update each edge's `x` and outgoing weight from the minimizer's output. If
  the incoming weight on an edge was infinite, `FactorData` forces that edge's
  outgoing weight back to infinite after the minimizer runs.

### 2. Variable Pass

For each variable:

- Gather the incoming messages from all connected edges: `m = x + u` with
  weights.
- Compute the weighted consensus value `z`:
  - If any message has infinite weight, `z` takes the first such value
    encountered (certainty wins).
  - Otherwise, if any message has standard weight, `z` is the arithmetic mean
    of the standard-weight messages.
  - Zero-weight messages are ignored when standard-weight messages exist.
- If every message has zero weight, average all their scalar values and give
  the consensus zero weight.
- Write the consensus `z` and its weight to every enabled edge.
- Update disagreement on an ordinary standard/standard edge:
  `u += learning_rate * (x - z)`.

The implementation resets `u` instead of accumulating it when either
direction has infinite weight, when the factor-to-variable direction has zero
weight, or when the variable has only one standard-weight incoming message.
Those resets discard disagreement that is no longer useful under the
three-weight rules.

### 3. Convergence Check

For each variable, compare the new consensus value against the previous belief
value. If every variable changes by at most `convergence_delta`, the graph has
converged and iteration stops early.

`FactorGraph::max_message_difference()` also exposes the largest enabled-edge
message change as a diagnostic. That quantity tracks dual-state motion; it is
not the default stopping criterion because messages can continue in a limit
cycle after the variable beliefs have settled.

## Dual-Update Learning Rate

The graph calls its step-size parameter `learning_rate`. It does not blend the
old and new consensus values. The variable adopts the newly computed `z`
immediately, while the parameter scales the disagreement update:

```text
u_new = u_old + learning_rate * (x - z)
```

This is the implementation's counterpart to the dual-variable step size
`alpha` in the paper (with the standard message weight normalized to `1.0`).
Smaller values make disagreement accumulate more slowly; they do not damp
`z` directly.

## Convergence

The graph reports convergence when all variable belief values change by at most
`convergence_delta`. For ordinary `iterate()` and
`iterate_until_converged()`, belief stability is the whole stopping rule.

Some domains need an additional problem-level check. Circle packing, for
example, can ask for both stable beliefs and `max_overlap <= tolerance` by
using `FactorGraph::iterate_until_satisfied()`.

In practice, some problems may not converge. The caller should set a maximum
iteration count as a safeguard.

## Where The Pieces Live

The message-passing loop is split across a small set of source files:

- [src/factor_graph.rs.md](../src/factor_graph.rs.md) explains
  `FactorGraph::iterate()`, `iterate_until_converged()`,
  `iterate_until_satisfied()`, `max_message_difference()`,
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
