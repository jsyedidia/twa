# src/problems/compact_sudoku.rs

## Role In The System

This file implements the compact Sudoku builder. It constructs the same kind
of one-hot Sudoku graph as the direct builder but skips given cells and
candidate values already ruled out by givens.

Related concept note:

- [notes/concepts/sudoku.md](../../concepts/sudoku.md)

## Code Walkthrough

### Imports

```rust
use crate::{FactorGraph, VariableNode, create_one_hot_factor};

use super::sudoku::{SudokuGivens, cell_index, outer_side_for, validate_givens};
```

The compact builder reuses the direct builder's givens and indexing helpers.

### Variables Struct

```rust
#[derive(Debug, Clone)]
pub struct CompactSudokuVariables {
    pub inner_side: usize,
    pub outer_side: usize,
    pub givens: SudokuGivens,
    pub candidates: Vec<Vec<VariableNode>>,
}
```

`candidates[cell][value]` is either a valid graph variable or the default
invalid `VariableNode` sentinel when that candidate was pruned.

### Compact Builder

```rust
pub fn add_compact_sudoku_to_factor_graph(
    graph: &mut FactorGraph,
    inner_side: usize,
    givens: &SudokuGivens,
) -> CompactSudokuVariables {
```

The builder validates size and givens, then initializes every candidate slot to
an invalid handle.

```rust
            if givens.contains_key(&cell) {
                continue;
            }
```

Given cells are omitted entirely. Their values are recorded in the returned
metadata and restored during extraction.

```rust
                if value_found(givens, inner_side, outer_side, row, column, value) {
                    continue;
                }
```

Candidates already present in the same row, column, or box are pruned before a
graph variable is created.

```rust
            create_one_hot_factor(graph, &cell_candidates);
```

Each non-given cell receives a one-hot factor over its surviving candidates.

```rust
            add_one_hot_if_nonempty(graph, &row_value_variables);
            add_one_hot_if_nonempty(graph, &column_value_variables);
            add_one_hot_if_nonempty(graph, &square_value_variables);
```

Row, column, and box constraints are added only when at least one surviving
candidate remains for that value.

### State Extraction

```rust
pub fn extract_compact_sudoku_state(
    graph: &FactorGraph,
    variables: &CompactSudokuVariables,
) -> Vec<i32> {
```

Givens are copied directly into the extracted state. Other cells scan only
valid surviving candidates and use the same `> 0.99` threshold as the direct
builder.

### Pruning Helpers

```rust
fn square_index(row: usize, column: usize, inner_side: usize) -> usize {
```

The square helpers convert a row and column into a box index and box origin.

```rust
fn has_given_value(givens: &SudokuGivens, cell: usize, value: usize) -> bool {
    givens.get(&cell).is_some_and(|&given| given == value)
}
```

`has_given_value` is the primitive used by pruning.

```rust
fn value_found(
```

`value_found` checks whether a value already appears in the candidate's row,
column, or box.

```rust
fn add_one_hot_if_nonempty(graph: &mut FactorGraph, candidates: &[VariableNode]) {
```

Some compact row/column/box value groups are already satisfied by givens and
therefore have no surviving candidates. Empty groups do not get factors.

### Tests

The tests port the compact graph count checks for all tracked fixtures,
confirm that compact 9x9 construction is smaller than direct construction,
verify extraction includes givens, cover invalid inputs, and solve the 2x2 and
9x9 fixtures.

## Important Invariants

- Invalid candidate handles mean "candidate pruned"; they are not graph nodes.
- Given cells are omitted from graph construction and restored during
  extraction.
- Empty row/column/box value groups do not create one-hot factors.

## Extension Notes

When changing pruning, keep the graph-count fixture tests updated. They are a
compact way to catch accidental changes in construction behavior.
