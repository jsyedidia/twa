# src/lib.rs

## Role In The System

`src/lib.rs` is the crate root. It sets crate-wide safety policy, attaches the
README as crate-level documentation, declares modules, and re-exports the
public API that users are expected to import from `twa`.

## Code Walkthrough

### Crate Attributes

```rust
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
```

The crate forbids unsafe Rust everywhere unless this policy is explicitly
changed later. The README is included as the top-level Rustdoc page.

### Public Modules

```rust
pub mod factor_graph;
pub mod factor_node;
pub mod graph_edge;
pub mod minimizers;
pub mod problems;
pub mod variable_node;
pub mod weighted_value;
```

These modules contain the public graph type, typed handles, and weighted-value
types. `minimizers` contains the built-in factor constructors. `problems`
contains reusable problem builders.

### Internal Modules

```rust
pub(crate) mod edge_data;
pub(crate) mod factor_data;
pub(crate) mod variable_data;
```

These modules hold implementation storage. They are visible throughout the
crate but not exported to downstream users.

### Re-exports

```rust
pub use factor_graph::FactorGraph;
pub use factor_node::FactorNode;
pub use graph_edge::GraphEdge;
pub use minimizers::{
    create_in_range_factor, create_known_value_factor, create_one_hot_factor, create_spy_factor,
};
pub use problems::{
    Circle, CirclePackingVariables, CompactSudokuVariables, CoordinateRange, KissingCircle,
    RadiusCount, SudokuGivens, SudokuPuzzle, SudokuVariables, add_circle_packing_to_factor_graph,
    add_circle_packing_to_factor_graph_fast, add_compact_sudoku_to_factor_graph,
    add_sudoku_to_factor_graph, create_intersection_factor, create_kiss_factor, extract_circles,
    extract_compact_sudoku_state, extract_sudoku_state, generate_circles, max_overlap,
    read_sudoku_puzzle,
};
pub use variable_node::VariableNode;
pub use weighted_value::{MessageWeight, MinimizationFn, WeightedValue, WeightedValueExchange};
```

The re-exports make the main API available from the crate root, so users can
write `twa::FactorGraph`, `twa::create_one_hot_factor`, or
`twa::add_circle_packing_to_factor_graph` instead of importing from nested
modules.

## Important Invariants

- Internal storage modules remain crate-private.
- The crate stays `unsafe`-free unless a future measured performance need comes
  with a documented safety argument.

## Algorithm Context

This file does not implement TWA behavior directly. It defines the public and
internal module boundary for the implementation.

## Extension Notes

When adding minimizers, problem builders, or binaries, declare library modules
here only when they are part of the reusable crate API. Keep binary-only code
inside `src/bin/`.
