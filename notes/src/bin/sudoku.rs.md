# src/bin/sudoku.rs

## Role In The System

This binary is the command-line Sudoku solver. It parses CLI options, reads a
Sudoku puzzle file, builds either the compact or direct graph, iterates to
convergence, and optionally prints the solved grid.

## Code Walkthrough

### Imports And Defaults

```rust
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use twa::{
    FactorGraph, add_compact_sudoku_to_factor_graph, add_sudoku_to_factor_graph,
    extract_compact_sudoku_state, extract_sudoku_state, read_sudoku_puzzle,
};

const DEFAULT_PROBLEM_PATH: &str = "data/sudoku/example_4x4_medium.txt";
```

The binary keeps timing and argument parsing local. Graph construction and
puzzle parsing come from the library.

### Builder Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Builder {
    Compact,
    Full,
}
```

`ValueEnum` lets clap parse `--builder compact` and `--builder full`.

### Options

```rust
#[derive(Debug, Parser)]
#[command(
    name = "sudoku",
    about = "Solve a Sudoku text file using the Three-Weight Algorithm"
)]
struct Options {
```

The options support a positional puzzle path and a reference-compatible
`--problem_path` flag. Long options use underscores as their primary spelling,
with hyphenated aliases for Rust CLI convention.

### `main`

```rust
fn main() {
```

`main` parses options, runs the solver, and maps results to exit codes:
`0` for convergence, `2` for non-convergence, and `1` for errors.

### `run`

```rust
fn run(options: &Options) -> Result<bool, Box<dyn std::error::Error>> {
```

`run` selects the puzzle path, reads the puzzle, creates the configured
`FactorGraph`, and dispatches to the compact or full builder.

```rust
    let converged = graph.iterate_until_converged(options.max_iterations);
```

The graph iteration result drives the process exit code. Timing output is
diagnostic only.

```rust
    if options.print_solution {
```

Solution extraction uses the matching variables object for the selected
builder.

### Output Helpers

```rust
fn builder_name(builder: Builder) -> &'static str {
```

The builder name is printed in the configuration summary.

```rust
fn print_solution(solution: &[i32], outer_side: usize) {
    println!("{}", format_solution(solution, outer_side));
}

fn format_solution(solution: &[i32], outer_side: usize) -> String {
```

The formatter converts zero-based internal values back to one-based display
values and writes `.` for unknown cells.

### Tests

```rust
#[cfg(test)]
mod tests {
```

The binary tests cover builder names and solution-grid formatting. Solver
behavior is covered in the library tests and command verification.

## Important Invariants

- Library modules own parsing and graph construction.
- CLI output displays Sudoku values one-based.
- The default builder is compact.

## Extension Notes

Keep new CLI options in `Options`; avoid moving CLI-only concerns into
`src/problems/`.
