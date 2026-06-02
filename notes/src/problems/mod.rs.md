# src/problems/mod.rs

## Role In The System

This module groups reusable problem builders built on top of the factor graph
API.

## Code Walkthrough

### Module Documentation

```rust
//! Problem builders built on top of the factor graph API.
```

The module-level docs identify this as the domain-builder layer.

### Child Modules

```rust
pub mod circle_packing;
pub mod compact_sudoku;
pub mod sudoku;
```

`circle_packing` contains the geometric packing builder. `sudoku` contains
parsing and the direct builder. `compact_sudoku` contains the pruned builder.

### Re-exports

```rust
pub use circle_packing::{
    Circle, CirclePackingVariables, CoordinateRange, KissingCircle, RadiusCount,
    add_circle_packing_to_factor_graph, add_circle_packing_to_factor_graph_fast,
    create_intersection_factor, create_kiss_factor, extract_circles, generate_circles, max_overlap,
};
pub use compact_sudoku::{
    CompactSudokuVariables, add_compact_sudoku_to_factor_graph, extract_compact_sudoku_state,
};
pub use sudoku::{
    SudokuGivens, SudokuPuzzle, SudokuVariables, add_sudoku_to_factor_graph, extract_sudoku_state,
    read_sudoku_puzzle,
};
```

The re-exports let callers import problem builders from `twa::problems`.

## Important Invariants

- Problem construction code belongs in child modules.
- Re-exported items should be reusable outside binaries.
