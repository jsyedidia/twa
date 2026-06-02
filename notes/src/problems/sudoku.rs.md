# src/problems/sudoku.rs

## Role In The System

This file owns Sudoku puzzle parsing, the direct Sudoku graph builder, and
state extraction for direct Sudoku variables.

Related concept note:

- [notes/concepts/sudoku.md](../../concepts/sudoku.md)

## Code Walkthrough

### Imports

```rust
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::num::ParseIntError;
use std::path::Path;

use crate::{FactorGraph, MessageWeight, VariableNode, create_one_hot_factor};
```

The parser uses standard library file, path, and error types. The builder uses
`FactorGraph`, candidate handles, message weights for givens, and the one-hot
minimizer factory.

### Public Types

```rust
/// Mapping from flat cell index to zero-based given value.
pub type SudokuGivens = HashMap<usize, usize>;

/// Direct Sudoku candidate variables, indexed as `[cell][value]`.
pub type SudokuVariables = Vec<Vec<VariableNode>>;

/// A parsed Sudoku puzzle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SudokuPuzzle {
    pub inner_side: usize,
    pub outer_side: usize,
    pub givens: SudokuGivens,
}
```

`SudokuGivens` is zero-based internally. `SudokuVariables` preserves the
mapping from `(cell, value)` to graph variable.

### Parse Errors

```rust
/// Error returned when reading or parsing a Sudoku puzzle file fails.
#[derive(Debug)]
pub enum SudokuParseError {
    Io(io::Error),
    Empty,
    RowCountNotSquare {
        rows: usize,
    },
    NotSquare {
        row: usize,
        columns: usize,
        expected: usize,
    },
    InvalidToken {
        row: usize,
        column: usize,
        token: String,
        source: ParseIntError,
    },
    ClueOutOfRange {
        row: usize,
        column: usize,
        token: String,
        outer_side: usize,
    },
}
```

The parser distinguishes I/O errors, shape errors, token errors, and clues
outside the allowed one-based range.

```rust
impl fmt::Display for SudokuParseError {
```

The display implementation turns each variant into a user-facing message for
the CLI.

```rust
impl std::error::Error for SudokuParseError {
```

The error implementation exposes underlying I/O and integer parse errors.

```rust
impl From<io::Error> for SudokuParseError {
```

This conversion lets `read_sudoku_puzzle` use `?` on `fs::read_to_string`.

### Puzzle Parsing

```rust
pub fn read_sudoku_puzzle(path: impl AsRef<Path>) -> Result<SudokuPuzzle, SudokuParseError> {
    parse_sudoku_puzzle(&fs::read_to_string(path)?)
}
```

File reading is a small wrapper around string parsing.

```rust
pub fn parse_sudoku_puzzle(input: &str) -> Result<SudokuPuzzle, SudokuParseError> {
```

`parse_sudoku_puzzle` splits non-empty whitespace rows, checks that the row
count is a perfect square, checks that the grid is square, then converts clues
from one-based text values to zero-based givens.

### Direct Builder

```rust
pub fn add_sudoku_to_factor_graph(
    graph: &mut FactorGraph,
    inner_side: usize,
    givens: &SudokuGivens,
) -> SudokuVariables {
```

The direct builder creates every candidate variable. Givens are encoded as
infinite-weight initial values: the given candidate starts at `1.0`; other
candidates in the same cell start at `0.0`.

```rust
        create_one_hot_factor(graph, &options);
```

Each cell immediately gets a one-hot factor over its candidate values.

```rust
            create_one_hot_factor(graph, &row_value_variables);
            create_one_hot_factor(graph, &column_value_variables);
            create_one_hot_factor(graph, &square_value_variables);
```

For every value and every row/column/box index, the builder adds one-hot
constraints enforcing uniqueness.

### State Extraction

```rust
pub fn extract_sudoku_state(graph: &FactorGraph, variables: &SudokuVariables) -> Vec<i32> {
```

Extraction scans each cell's candidates and reports the last candidate with
graph value greater than `0.99`. If no candidate crosses the threshold, the
cell is `-1`.

### Shared Helpers

```rust
pub(crate) fn outer_side_for(inner_side: usize, message: &str) -> usize {
    assert!(inner_side > 0, "{message}");
    inner_side * inner_side
}

pub(crate) fn cell_index(row: usize, column: usize, outer_side: usize) -> usize {
    row * outer_side + column
}

pub(crate) fn square_root(value: usize) -> Option<usize> {
```

These helpers are shared with the compact builder. `square_root` accepts only
perfect-square side lengths.

```rust
pub(crate) fn validate_givens(givens: &SudokuGivens, outer_side: usize, name: &str) {
```

`validate_givens` panics on out-of-range cells or values. Builder inputs are
programmer errors; parser errors are returned as `Result`.

### Tests

The tests cover direct-builder graph sizes for 4x4 and 9x9 puzzles,
thresholded state extraction, invalid builder inputs, parser validation, and a
known 4x4 solve.

## Important Invariants

- Puzzle files are one-based; `SudokuGivens` are zero-based.
- Direct builder graph sizes are `outer_side^3` variables,
  `4 * outer_side^2` factors, and `4 * outer_side^3` edges.
- Givens are encoded with infinite initial weight, not separate known-value
  factors.

## Extension Notes

Keep shared indexing helpers crate-private so compact Sudoku can reuse them
without making them part of the stable public API.
