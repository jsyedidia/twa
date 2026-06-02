# twa — Three-Weight Algorithm

A Rust implementation of the Three-Weight Algorithm (TWA) for message-passing
on factor graphs. This is a port of a C++ implementation 
(<https://github.com/jsyedidia/three_weight>) which was itself
derived from Nate Derbinsky's MIT-licensed SwiftADMM library
(<https://github.com/natederbinsky/swiftADMM>).

The TWA solves constraint-satisfaction and optimization problems by iterating
messages on a factor graph until convergence. It uses three message weights
(zero, standard, infinite) to express "no opinion", "ordinary opinion", and
"certainty". The algorithm is described in
[arXiv:1305.1961](https://arxiv.org/abs/1305.1961).

## Layout

- `src/` — library implementation.
- `src/bin/` — command-line executables (`sudoku`, `gui`).
- `tests/` — integration tests.
- `data/sudoku/` — example puzzle files.
- `docs/` — user-facing documentation.
- `notes/` — detailed Markdown explanations of source files and concepts.

## Build

```sh
cargo test
cargo build --release
```

## Binaries

### Sudoku solver

```sh
cargo sudoku --help
cargo sudoku --print-solution data/sudoku/example_3x3.txt
```

The solver reads whitespace-separated Sudoku grids using `.` for empty cells.
It infers the puzzle size from the square grid dimensions.

### GUI visualizer (requires `gui` feature)

```sh
cargo gui
```

See [docs/gui_help.md](docs/gui_help.md) for controls and verification
commands.

## Library Example

```rust
use twa::{create_known_value_factor, FactorGraph, MessageWeight};

let mut graph = FactorGraph::default();
let variable = graph.create_variable(0.0, MessageWeight::Standard);

create_known_value_factor(&mut graph, variable, 3.5);

assert!(graph.iterate_until_converged(100));
assert_eq!(graph.value(variable), 3.5);
assert_eq!(graph.weight(variable), MessageWeight::Infinite);
```

The main public type is `FactorGraph`. Built-in minimizer factories cover
known values, scalar ranges, one-hot constraints, and spy callbacks. The
`problems` module contains builders for Sudoku and circle packing.

## Performance

The compact 25x25 Sudoku fixture is the current release-mode benchmark for the
iteration loop:

```sh
cargo build --release --bin sudoku
./target/release/sudoku --builder compact --max-iterations 100000 data/sudoku/example_5x5.txt
```

## Documentation

The `notes/` directory contains detailed prose explanations of the algorithm
and codebase, including concept guides and file-by-file walkthroughs. 
See [notes/README.md](notes/README.md) for a suggested reading order and orientation.
