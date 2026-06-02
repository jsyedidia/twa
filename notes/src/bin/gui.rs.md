# src/bin/gui.rs

## Role In The System

This binary is the native interactive visualizer for `twa`. It uses
`egui`/`eframe` behind the `gui` cargo feature and lets a user step, run,
rebuild, and inspect factor graphs for Sudoku and circle packing.

The user-facing guide is [docs/gui_help.md](../../../docs/gui_help.md).

## State At A Glance

`SolverApp` owns all GUI state:

- lists of built-in Sudoku and circle-packing problems,
- current problem selection and graph parameters,
- the active `FactorGraph`,
- problem-specific variable handles,
- selected Sudoku cell or selected circle,
- running/dirty/timing status.

The app rebuilds the graph whenever the user changes problem settings and then
asks for reset, step, run, or run-to-convergence.

## Code Walkthrough

### Imports And Constants

```rust
use std::time::Instant;

use clap::Parser;
use eframe::egui::{
    self, Align2, CentralPanel, Color32, ComboBox, Context, DragValue, FontId, Pos2, ProgressBar,
    Rect, Response, Sense, SidePanel, Slider, Stroke, StrokeKind, Vec2,
};
use twa::{
    Circle, CirclePackingVariables, CompactSudokuVariables, CoordinateRange, FactorGraph,
    FactorNode, MessageWeight, RadiusCount, SudokuPuzzle, SudokuVariables,
    add_circle_packing_to_factor_graph, add_circle_packing_to_factor_graph_fast,
    add_compact_sudoku_to_factor_graph, add_sudoku_to_factor_graph, extract_circles,
    extract_compact_sudoku_state, extract_sudoku_state, generate_circles, max_overlap,
    read_sudoku_puzzle,
};

const MAX_DRAWN_CONNECTION_LINES: usize = 5_000;
const VISUAL_OVERLAP_EPSILON: f64 = 1e-8;
```

The GUI stays thin over the library API. It imports graph builders, extraction
helpers, and typed handles from `twa`, while drawing and layout come from
`egui`.

### CLI Entry Point

```rust
#[derive(Debug, Parser)]
#[command(name = "gui", about = "Interactive Three-Weight Algorithm visualizer")]
struct Options {
    #[arg(long)]
    smoke_test: bool,
}

fn main() -> eframe::Result<()> {
    let options = Options::parse();
    if options.smoke_test {
        smoke_test();
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 860.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Three-Weight Algorithm",
        native_options,
        Box::new(|creation_context| Ok(Box::new(SolverApp::new(creation_context)))),
    )
}
```

`--smoke-test` builds and solves a tiny graph without opening a window. Normal
execution starts the native `eframe` event loop with an initial window size but
no configured minimum native window size.

### Smoke Test

```rust
fn smoke_test() {
    let mut app = SolverApp {
        domain: ProblemDomain::Sudoku,
        puzzle_index: 0,
        sudoku_build: SudokuBuild::Compact,
        ..Default::default()
    };
    app.rebuild();
    app.run_to_convergence();

    let state = app.sudoku_state();
    assert_eq!(state, app.puzzle_spec().solution.as_deref().unwrap());
    println!(
        "gui smoke test ok: {} iterations, {} variables, {} factors",
        app.graph.as_ref().map_or(0, FactorGraph::iterations),
        app.graph.as_ref().map_or(0, FactorGraph::num_variables),
        app.graph.as_ref().map_or(0, FactorGraph::num_factors)
    );
}
```

The smoke path exercises the same rebuild and iteration code used by the GUI,
but avoids native-window requirements.

### Selection Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProblemDomain {
    Sudoku,
    CirclePacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SudokuBuild {
    Compact,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircleBuild {
    Fast,
    Full,
}
```

Each enum has a `label` method used by combo boxes. Keeping these as enums
avoids string comparisons in app logic.

### Problem Specs

```rust
#[derive(Debug, Clone)]
struct PuzzleSpec {
    name: &'static str,
    path: &'static str,
    solution: Option<Vec<i32>>,
}

#[derive(Debug, Clone)]
struct CircleProblem {
    name: &'static str,
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    radii: Vec<RadiusCount>,
}

enum SudokuVariablesState {
    Compact(CompactSudokuVariables),
    Full(SudokuVariables),
}
```

`PuzzleSpec` stores tracked fixture paths and optional known solutions.
`CircleProblem` stores radius mixtures and ranges. `SudokuVariablesState`
keeps the compact and full Sudoku builders behind one field.

### `SolverApp`

```rust
struct SolverApp {
    puzzles: Vec<PuzzleSpec>,
    circle_problems: Vec<CircleProblem>,
    domain: ProblemDomain,
    puzzle_index: usize,
    sudoku_build: SudokuBuild,
    circle_problem_index: usize,
    circle_build: CircleBuild,
    random_seed: u64,
    sudoku_learning_rate: f64,
    circle_learning_rate: f64,
    convergence_delta: f64,
    max_iterations: usize,
    iterations_per_frame: usize,
    circle_count: usize,
    circle_density_target: f64,
    nearby_radius_scale: f64,
    selected_cell: usize,
    selected_circle: usize,
    running: bool,
    settings_dirty: bool,
    last_step_ms: f64,
    graph: Option<FactorGraph>,
    puzzle: Option<SudokuPuzzle>,
    sudoku_variables: Option<SudokuVariablesState>,
    initial_circles: Vec<Circle>,
    circle_variables: Option<CirclePackingVariables>,
    last_error: Option<String>,
}
```

The graph and variable handles are optional because rebuild can fail if a
fixture cannot be read. The UI reports `last_error` instead of panicking.

### Rebuild Logic

```rust
fn rebuild(&mut self) {
    self.graph = None;
    self.puzzle = None;
    self.sudoku_variables = None;
    self.initial_circles.clear();
    self.circle_variables = None;
    self.last_error = None;

    let mut graph = FactorGraph::new(
        self.active_learning_rate(),
        self.convergence_delta,
        self.random_seed,
    );

    let result = match self.domain {
        ProblemDomain::Sudoku => self.rebuild_sudoku(&mut graph),
        ProblemDomain::CirclePacking => self.rebuild_circle_packing(&mut graph),
    };
```

Rebuild starts from a fresh `FactorGraph` so all variables, factors, callbacks,
and dynamic factor state are regenerated from the current settings.

```rust
fn rebuild_sudoku(&mut self, graph: &mut FactorGraph) -> Result<(), String> {
    let puzzle = read_sudoku_puzzle(self.puzzle_spec().path)
        .map_err(|error| format!("could not read {}: {error}", self.puzzle_spec().path))?;
    let cell_count = puzzle.outer_side * puzzle.outer_side;
    self.selected_cell = self.selected_cell.min(cell_count.saturating_sub(1));

    self.sudoku_variables = Some(match self.sudoku_build {
        SudokuBuild::Compact => SudokuVariablesState::Compact(
            add_compact_sudoku_to_factor_graph(graph, puzzle.inner_side, &puzzle.givens),
        ),
        SudokuBuild::Full => SudokuVariablesState::Full(add_sudoku_to_factor_graph(
            graph,
            puzzle.inner_side,
            &puzzle.givens,
        )),
    });
    self.puzzle = Some(puzzle);
    Ok(())
}
```

Sudoku rebuild reads a tracked fixture and dispatches to the compact or full
builder.

```rust
fn rebuild_circle_packing(&mut self, graph: &mut FactorGraph) -> Result<(), String> {
    let problem = self.circle_problem().clone();
    let radii = self.scaled_radii();
    self.initial_circles = generate_circles(
        self.random_seed,
        &radii,
        problem.horizontal_range,
        problem.vertical_range,
    );

    self.circle_variables = Some(match self.circle_build {
        CircleBuild::Fast => add_circle_packing_to_factor_graph_fast(
            graph,
            &self.initial_circles,
            problem.horizontal_range,
            problem.vertical_range,
            self.nearby_radius_scale,
        ),
        CircleBuild::Full => add_circle_packing_to_factor_graph(
            graph,
            &self.initial_circles,
            problem.horizontal_range,
            problem.vertical_range,
            None,
        ),
    });
    Ok(())
}
```

Circle-packing rebuild generates deterministic initial circles, then attaches
either the fast dynamic-intersection builder or the full pairwise builder.

### Circle Scaling

```rust
fn scaled_radii(&self) -> Vec<RadiusCount> { /* ... */ }

fn circle_density(&self) -> f64 { /* ... */ }
```

`scaled_radii` preserves the selected problem's radius mixture while matching
the requested circle count, then scales radii to the target density.
`circle_density` recomputes the resulting area fraction for display.

### State Extraction

```rust
fn sudoku_state(&self) -> Vec<i32> { /* ... */ }

fn packed_circles(&self) -> Vec<Circle> { /* ... */ }

fn circle_max_overlap(&self) -> f64 { /* ... */ }
```

These methods read current graph consensus values through the problem-builder
extraction helpers.

### Iteration

```rust
fn step_many(&mut self, count: usize) {
    let Some(graph) = &mut self.graph else {
        self.running = false;
        return;
    };
    if graph.converged() {
        self.running = false;
        return;
    }

    let start = Instant::now();
    for _ in 0..count.max(1) {
        if graph.converged() {
            break;
        }
        graph.iterate();
    }
    self.last_step_ms = start.elapsed().as_secs_f64() * 1_000.0;

    if graph.converged() {
        self.running = false;
    }
}

fn run_to_convergence(&mut self) { /* ... */ }
```

The app can step one or many iterations per frame. `run_to_convergence`
delegates to `FactorGraph::iterate_until_converged`.

### Controls And Status

```rust
fn draw_control_panel(&mut self, ctx: &Context) { /* ... */ }

fn draw_sudoku_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) { /* ... */ }

fn draw_circle_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) { /* ... */ }

fn draw_solver_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) { /* ... */ }

fn draw_status(&self, ui: &mut egui::Ui) { /* ... */ }
```

The left side panel owns problem selection, solver parameters, step/run
buttons, graph statistics, and convergence or overlap status.

### Sudoku Drawing

```rust
fn draw_sudoku(&mut self, ctx: &Context) { /* ... */ }

fn draw_sudoku_grid(&mut self, ui: &mut egui::Ui, state: &[i32]) { /* ... */ }

fn draw_candidate_panel(&mut self, ui: &mut egui::Ui) { /* ... */ }
```

The central panel draws the Sudoku board with colored cells and major square
lines. The right panel shows candidate value bars for the selected cell.

### Circle Drawing

```rust
fn draw_circle_packing(&mut self, ctx: &Context) { /* ... */ }

fn draw_circle_canvas(&mut self, ui: &mut egui::Ui, circles: &[Circle]) { /* ... */ }

fn draw_enabled_intersections(
    &self,
    painter: &egui::Painter,
    rect: Rect,
    scale: f32,
    height: f32,
    problem: &CircleProblem,
    circles: &[Circle],
) { /* ... */ }

fn draw_selected_circle_panel(&mut self, ui: &mut egui::Ui) { /* ... */ }
```

The circle canvas maps world coordinates to screen coordinates, colors
overlapping circles with red outlines, and draws enabled fast-builder
intersection factors as amber connection lines.

### App Trait

```rust
impl eframe::App for SolverApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.running {
            self.step_many(self.iterations_per_frame);
            ctx.request_repaint();
        }

        self.draw_control_panel(ctx);
        match self.domain {
            ProblemDomain::Sudoku => self.draw_sudoku(ctx),
            ProblemDomain::CirclePacking => self.draw_circle_packing(ctx),
        }
    }
}
```

`eframe` calls `update` every frame. Continuous running advances the graph and
requests another repaint.

### Helpers And Tests

```rust
fn apply_visuals(ctx: &Context) { /* ... */ }
fn make_puzzles() -> Vec<PuzzleSpec> { /* ... */ }
fn make_circle_problems() -> Vec<CircleProblem> { /* ... */ }
fn boundary_overlap(circle: Circle, problem: &CircleProblem) -> f64 { /* ... */ }
fn pair_overlap(first: Circle, second: Circle) -> f64 { /* ... */ }
fn world_to_screen(...) -> Pos2 { /* ... */ }
```

The remaining helpers provide fixture lists, colors, labels, overlap checks,
and coordinate conversion.

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn scaled_radii_preserve_target_count_and_density() { /* ... */ }

    #[test]
    fn gui_sudoku_path_matches_cli_solution_for_2x2() { /* ... */ }
}
```

The binary tests cover radius scaling and confirm that the GUI rebuild/iterate
path solves the 2x2 Sudoku fixture to the same known state used by the CLI.

## Important Invariants

- The GUI never mutates graph internals directly; it uses public graph and
  problem-builder APIs.
- Dirty settings are applied by rebuilding a fresh graph.
- Continuous running must request repaint so the event loop keeps advancing.
- The fast circle-packing view only draws a capped number of connection lines.

## Extension Notes

When adding new problem domains, follow the same pattern: keep domain data in a
small spec type, rebuild into a fresh `FactorGraph`, store returned variable
handles, and draw current state through extraction helpers.
