# Factor Graphs

This note explains what a factor graph is and how this crate uses one to
represent optimization and constraint-satisfaction problems.

## What Is A Factor Graph?

A factor graph is a bipartite graph with two kinds of nodes:

- **Variable nodes**, representing unknown quantities to be solved for.
- **Factor nodes**, representing constraints or local objective terms that
  involve one or more variables.

Edges connect factors to variables. An edge always links exactly one factor to
exactly one variable. A factor with three incident edges touches three
variables; those three edges carry messages back and forth between that factor
and its three variables.

The bipartite structure matters: there are never direct edges between two
variables or between two factors. All communication happens through the
factor–variable edges.

## Why Factor Graphs?

Many optimization and constraint-satisfaction problems decompose naturally into
local pieces. Sudoku, for example, has row constraints, column constraints, and
box constraints, each involving a subset of the cells. Circle packing has
pairwise non-overlap constraints between nearby circles plus boundary
constraints for each circle.

A factor graph makes this decomposition explicit:

- Each local constraint or cost becomes a factor node.
- Each unknown becomes a variable node.
- The edges show which unknowns participate in which constraints.

The Three-Weight Algorithm (TWA) exploits this structure by running
message-passing on the factor graph until the variables reach consensus.

## Factor Graphs In This Crate

The public type is `FactorGraph`. Users build a graph by creating variables,
edges, and factors, then iterate until convergence.

### Handles

The three node and edge types exposed to users are lightweight handles:

- `VariableNode` — identifies a variable by index.
- `GraphEdge` — identifies an edge by index.
- `FactorNode` — identifies a factor by index.

Each handle is a newtype around `usize` with an `is_valid()` check. They are
cheap to copy and compare.

### Building A Graph

A typical construction sequence is:

```rust
use twa::{FactorGraph, MessageWeight, WeightedValue};

let mut graph = FactorGraph::default();

// Create variables.
let a = graph.create_variable(0.0, MessageWeight::Standard);
let b = graph.create_variable(0.0, MessageWeight::Standard);

// Create edges for a factor that touches both variables.
let ea = graph.create_edge(a);
let eb = graph.create_edge(b);

// Create the factor with a minimization function.
graph.create_factor(
    &[ea, eb],
    Box::new(|exchanges, _rng| {
        let average = (exchanges[0].get().value + exchanges[1].get().value) / 2.0;
        exchanges[0].set(WeightedValue::new(average, MessageWeight::Standard));
        exchanges[1].set(WeightedValue::new(average, MessageWeight::Standard));
    }),
);
```

Each `create_edge` call links the new edge to one variable. Each
`create_factor` call groups edges and attaches a minimization function. The
minimizer only reads and writes `WeightedValueExchange` entries — it does not
need to know about graph internals.

### Iterating

After building, call `iterate_until_converged()`:

```rust
let converged = graph.iterate_until_converged(1000);
```

This runs up to 1000 iterations. The learning rate, convergence threshold, and
random seed live on the graph; `FactorGraph::default()` uses the standard
settings, and `FactorGraph::new(learning_rate, convergence_delta, random_seed)`
lets callers choose them explicitly.

The default convergence check is belief-based: after an iteration, every
variable's new value must be within `convergence_delta` of its previous value.
If a problem also has a natural satisfaction check, use
`iterate_until_satisfied()`. Circle packing uses this to combine stable
beliefs with a `max_overlap` threshold.

### Reading Results

After convergence, read variable values:

```rust
let result = graph.value(a);
```

The current certainty can also be queried:

```rust
let weight = graph.weight(a);
```

If the weight is `MessageWeight::Infinite`, the solver is certain of that
variable's value.

## Internal Storage

The public graph owns three internal vectors:

- `VariableData` stores per-variable state: the initial value, current
  consensus, current weight, connected edges, and a lazy cache of enabled
  edges.
- `EdgeData` stores per-edge message state: factor-side `x`, variable-side
  `z`, disagreement `u`, directional weights, enabled state, and
  message-difference diagnostics.
- `FactorData` stores per-factor state: the minimization function, exchange
  buffer, certainty-preservation scratch space, and enabled flag.

The handle indexes are positions in these vectors. This is why handles are
cheap: `VariableNode`, `FactorNode`, and `GraphEdge` are typed indexes, not
owned graph objects.

See [src/variable_data.rs.md](../src/variable_data.rs.md),
[src/edge_data.rs.md](../src/edge_data.rs.md), and
[src/factor_data.rs.md](../src/factor_data.rs.md) for the storage details.

## Dynamic Factors

Factors can be disabled and reenabled:

```rust
graph.set_factor_enabled(factor, false);
graph.set_factor_enabled(factor, true);
```

Disabling a factor disables its incident edges. Variable enabled-edge caches
are marked stale so they are rebuilt on demand. Reenabling a factor resets its
edges from the current variable beliefs with standard weight.

This mechanism matters for fast circle packing. Far-away pairwise
intersection factors would send zero-weight messages, so the fast builder can
disable most of them and reenable only nearby pairs after each iteration.

## Relationship To The Paper

The paper describes:

- function cost nodes, which correspond to factor nodes;
- equality nodes, which correspond to variable nodes;
- edges carrying messages between them.

The implementation maps that description to `FactorGraph::iterate()`:

1. factor pass: factors compute local `x` values;
2. variable pass: variables compute consensus `z` values;
3. edge update: edges update disagreement `u`;
4. convergence check: variable beliefs report whether values have stabilized.

For the step-by-step implementation, read
[src/factor_graph.rs.md](../src/factor_graph.rs.md). For the algorithmic
message flow, read [message_passing.md](message_passing.md).

## Further Reading

- [message_passing.md](message_passing.md) explains one iteration in detail.
- [weights.md](weights.md) explains zero, standard, and infinite weights.
- [src/factor_graph.rs.md](../src/factor_graph.rs.md) walks through graph
  construction, iteration, convergence, callbacks, and factor enablement.
