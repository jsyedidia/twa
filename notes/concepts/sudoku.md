# Sudoku

## Role In The System

Sudoku is the first complete problem builder in this Rust crate. It turns a
familiar discrete constraint problem into a factor graph made almost entirely
from one-hot factors.

The crate has two Sudoku builders:

- the direct builder in [src/problems/sudoku.rs.md](../src/problems/sudoku.rs.md),
  which creates every `(cell, value)` candidate;
- the compact builder in
  [src/problems/compact_sudoku.rs.md](../src/problems/compact_sudoku.rs.md),
  which uses givens to avoid impossible candidates.

## Grid Sizes

The builders use two side lengths:

```text
outer_side = inner_side * inner_side
```

`inner_side` is the side length of one box. `outer_side` is the side length of
the whole grid. For example, `inner_side = 3` means a 9-by-9 puzzle with 3-by-3
boxes.

Cells are row-major:

```text
cell = row * outer_side + column
```

Puzzle files use one-based clue values. The library stores values zero-based.

## Candidate Variables

A Sudoku variable represents one candidate:

```text
cell c contains value v
```

The variable should converge to `1.0` when selected and `0.0` when rejected.

The direct builder creates every `(cell, value)` candidate. For a 9-by-9
puzzle, that is `81 * 9 = 729` variables.

The direct builder stores these handles in `SudokuVariables`, described in
[src/problems/sudoku.rs.md](../src/problems/sudoku.rs.md). The compact builder
stores a pruned candidate grid in `CompactSudokuVariables`, described in
[src/problems/compact_sudoku.rs.md](../src/problems/compact_sudoku.rs.md).

## One-Hot Constraints

Sudoku rules become one-hot constraints:

- Each cell selects exactly one value.
- Each row contains each value exactly once.
- Each column contains each value exactly once.
- Each box contains each value exactly once.

The one-hot minimizer drives exactly one connected candidate to `1.0` and all
others to `0.0`.

The one-hot behavior itself is implemented in
[src/minimizers/one_hot.rs.md](../src/minimizers/one_hot.rs.md). The builders
call it while assembling cell, row-value, column-value, and box-value factors.

## Givens

Givens are stored as `SudokuGivens`, a map from flat cell index to zero-based
value. The direct builder encodes a given by initializing every candidate in
that cell with infinite weight: the given candidate starts at `1.0`, and the
others start at `0.0`.

The compact builder prunes more aggressively. It omits given cells entirely and
does not create candidates ruled out by givens in the same row, column, or box.

The direct builder's given handling is easiest to see in
[src/problems/sudoku.rs.md](../src/problems/sudoku.rs.md). The compact
builder's candidate pruning is explained in
[src/problems/compact_sudoku.rs.md](../src/problems/compact_sudoku.rs.md).

## Extracting State

Both builders return a variables object that records which graph variable
belongs to which candidate. After iteration, extraction returns a row-major
`Vec<i32>`. A candidate is selected only when its graph value is greater than
`0.99`; extraction reports its zero-based value, or `-1` when no candidate
passes that threshold. Compact extraction copies givens directly into the
result.

The command-line frontend in [src/bin/sudoku.rs.md](../src/bin/sudoku.rs.md)
uses the parser, chooses either the compact or direct builder, runs
`iterate_until_converged()`, and prints the extracted solution when requested.

The GUI frontend in [src/bin/gui.rs.md](../src/bin/gui.rs.md) uses the same
builders and extraction functions, but displays the current grid and candidate
values after each step.

## Extension Notes

Keep parsing separate from graph construction. Parsing handles text-file
shape and one-based clue values; builders consume already-validated
zero-based givens.

## Further Reading

- [src/problems/sudoku.rs.md](../src/problems/sudoku.rs.md) explains puzzle
  parsing, direct graph construction, and direct state extraction.
- [src/problems/compact_sudoku.rs.md](../src/problems/compact_sudoku.rs.md)
  explains compact graph construction and compact state extraction.
- [src/minimizers/one_hot.rs.md](../src/minimizers/one_hot.rs.md) explains
  the main minimizer used by both Sudoku builders.
- [weights.md](weights.md) explains why givens use infinite weight.
- [message_passing.md](message_passing.md) explains how candidate values move
  during iteration.
