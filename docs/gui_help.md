# GUI Help

The GUI is an interactive `egui`/`eframe` visualizer for the Three-Weight
Algorithm. It can show Sudoku solving and circle-packing convergence as the
factor graph iterates.

## Running

Run the GUI with:

```text
cargo gui
```

For automated checks that do not open a window:

```text
cargo run --features gui --bin gui -- --smoke-test
```

## Controls

The left panel controls the active problem, graph parameters, and iteration
mode.

- `Problem`: switch between Sudoku and circle packing.
- `Reset`: rebuild the graph using the current settings.
- `Step`: run one iteration.
- `Run`: continuously run `Iterations/frame` iterations every frame.
- `Pause`: stop continuous iteration.
- `Run to convergence`: iterate until convergence or `Max iterations`.

Changing the seed, learning rate, convergence delta, or problem-specific
settings marks the graph dirty. The next reset, step, run, or run-to-convergence
action rebuilds the graph with the new settings.

## Sudoku View

The Sudoku view draws the current extracted grid. Click a cell to inspect its
candidate variables in the right panel.

Cell colors distinguish selected cells, givens, certain values, and solved
cells. Candidate bars show the current graph value for each candidate variable.
Compact Sudoku omits candidates ruled out by givens, so some candidate rows may
be absent.

The built-in 2x2 and 3x3 puzzles include known solutions. The status panel
reports whether the current extracted state matches those solutions.

## Circle Packing View

The circle-packing view draws the current circle centers and radii inside the
packing boundary. Click a circle to inspect its center, radius, overlap, and
enabled dynamic neighbors in the right panel.

Red outlines mark circles with a boundary or pairwise overlap. In fast mode,
amber connection lines show enabled intersection factors, capped at 5,000
drawn lines to keep the canvas responsive.

Circle settings include:

- `Circle count`: rescale the selected radius mixture to the requested number
  of circles.
- `Density`: scale radii to target a total circle area fraction.
- `Nearby scale`: fast-builder buffer used by the dynamic intersection manager.

## Verification

Useful development checks are:

```text
cargo build --features gui
cargo test --features gui
cargo clippy --features gui
cargo run --features gui --bin gui -- --smoke-test
```
