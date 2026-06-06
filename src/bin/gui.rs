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

#[derive(Debug, Parser)]
#[command(name = "gui", about = "Interactive Three-Weight Algorithm visualizer")]
struct Options {
    /// Build a GUI app state and run a tiny deterministic solve without opening a window.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProblemDomain {
    Sudoku,
    CirclePacking,
}

impl ProblemDomain {
    fn label(self) -> &'static str {
        match self {
            Self::Sudoku => "Sudoku",
            Self::CirclePacking => "Circle Packing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SudokuBuild {
    Compact,
    Full,
}

impl SudokuBuild {
    fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Full => "Full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircleBuild {
    Fast,
    Full,
}

impl CircleBuild {
    fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Full => "Full",
        }
    }
}

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

impl SolverApp {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        apply_visuals(&creation_context.egui_ctx);
        Self::default()
    }

    fn puzzle_spec(&self) -> &PuzzleSpec {
        &self.puzzles[self.puzzle_index]
    }

    fn circle_problem(&self) -> &CircleProblem {
        &self.circle_problems[self.circle_problem_index]
    }

    fn active_learning_rate(&self) -> f64 {
        match self.domain {
            ProblemDomain::Sudoku => self.sudoku_learning_rate,
            ProblemDomain::CirclePacking => self.circle_learning_rate,
        }
    }

    fn set_active_learning_rate(&mut self, value: f64) {
        match self.domain {
            ProblemDomain::Sudoku => self.sudoku_learning_rate = value,
            ProblemDomain::CirclePacking => self.circle_learning_rate = value,
        }
    }

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

        match result {
            Ok(()) => {
                self.graph = Some(graph);
                self.running = false;
                self.settings_dirty = false;
                self.last_step_ms = 0.0;
            }
            Err(error) => {
                self.last_error = Some(error);
                self.running = false;
                self.settings_dirty = false;
            }
        }
    }

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

        self.selected_circle = self
            .selected_circle
            .min(self.initial_circles.len().saturating_sub(1));
        Ok(())
    }

    fn scaled_radii(&self) -> Vec<RadiusCount> {
        let problem = self.circle_problem();
        let mut radii = problem.radii.clone();
        for radius_count in &mut radii {
            radius_count.count = 0;
        }

        let base_count = problem.radii.iter().map(|entry| entry.count).sum::<usize>();
        if base_count == 0 || radii.is_empty() {
            return radii;
        }

        let target_count = self.circle_count.clamp(1, 1_000);
        let mut remainders = vec![0.0; radii.len()];
        let mut assigned_count = 0;
        for (index, radius_count) in radii.iter_mut().enumerate() {
            let exact_count =
                target_count as f64 * problem.radii[index].count as f64 / base_count as f64;
            radius_count.count = exact_count.floor() as usize;
            remainders[index] = exact_count - radius_count.count as f64;
            assigned_count += radius_count.count;
        }

        while assigned_count < target_count {
            let (index, _) = remainders
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| left.total_cmp(right))
                .expect("radii should not be empty");
            radii[index].count += 1;
            remainders[index] = 0.0;
            assigned_count += 1;
        }

        let area = range_span(problem.horizontal_range) * range_span(problem.vertical_range);
        if area <= 0.0 {
            return radii;
        }

        let unscaled_density = radii
            .iter()
            .map(|entry| entry.count as f64 * std::f64::consts::PI * entry.radius * entry.radius)
            .sum::<f64>()
            / area;
        if unscaled_density <= 0.0 {
            return radii;
        }

        let radius_scale = (self.circle_density_target.max(0.0) / unscaled_density).sqrt();
        for radius_count in &mut radii {
            radius_count.radius *= radius_scale;
        }
        radii
    }

    fn circle_density(&self) -> f64 {
        let problem = self.circle_problem();
        let area = range_span(problem.horizontal_range) * range_span(problem.vertical_range);
        if area <= 0.0 {
            return 0.0;
        }

        self.scaled_radii()
            .iter()
            .map(|entry| entry.count as f64 * std::f64::consts::PI * entry.radius * entry.radius)
            .sum::<f64>()
            / area
    }

    fn sudoku_state(&self) -> Vec<i32> {
        let Some(graph) = &self.graph else {
            return Vec::new();
        };
        match &self.sudoku_variables {
            Some(SudokuVariablesState::Compact(variables)) => {
                extract_compact_sudoku_state(graph, variables)
            }
            Some(SudokuVariablesState::Full(variables)) => extract_sudoku_state(graph, variables),
            None => Vec::new(),
        }
    }

    fn packed_circles(&self) -> Vec<Circle> {
        let (Some(graph), Some(variables)) = (&self.graph, &self.circle_variables) else {
            return Vec::new();
        };
        extract_circles(graph, variables)
    }

    fn circle_max_overlap(&self) -> f64 {
        let (Some(graph), Some(variables)) = (&self.graph, &self.circle_variables) else {
            return 0.0;
        };
        let problem = self.circle_problem();
        max_overlap(
            graph,
            variables,
            problem.horizontal_range,
            problem.vertical_range,
        )
    }

    fn matches_solution(&self, state: &[i32]) -> bool {
        self.puzzle_spec()
            .solution
            .as_ref()
            .is_some_and(|solution| solution.as_slice() == state)
    }

    fn enabled_intersection_factors(&self) -> usize {
        let (Some(graph), Some(variables)) = (&self.graph, &self.circle_variables) else {
            return 0;
        };

        variables
            .intersection_factors
            .iter()
            .flatten()
            .filter(|&&factor| graph.is_factor_enabled(factor))
            .count()
    }

    fn total_intersection_factors(&self) -> usize {
        self.circle_variables.as_ref().map_or(0, |variables| {
            variables.intersection_factors.iter().map(Vec::len).sum()
        })
    }

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

    fn run_to_convergence(&mut self) {
        let circle_satisfaction = if self.domain == ProblemDomain::CirclePacking {
            self.circle_variables.as_ref().map(|variables| {
                (
                    variables.clone(),
                    self.circle_problem().clone(),
                    self.convergence_delta,
                )
            })
        } else {
            None
        };
        let Some(graph) = &mut self.graph else {
            self.running = false;
            return;
        };
        if graph.converged() {
            self.running = false;
            return;
        }

        let start = Instant::now();
        match circle_satisfaction {
            Some((variables, problem, tolerance)) => {
                graph.iterate_until_satisfied(self.max_iterations.max(1), |graph| {
                    max_overlap(
                        graph,
                        &variables,
                        problem.horizontal_range,
                        problem.vertical_range,
                    ) <= tolerance
                });
            }
            None => {
                graph.iterate_until_converged(self.max_iterations.max(1));
            }
        }
        self.last_step_ms = start.elapsed().as_secs_f64() * 1_000.0;
        self.running = false;
    }

    fn apply_settings_if_dirty(&mut self) {
        if self.settings_dirty {
            self.rebuild();
        }
    }

    fn selected_candidate(&self, cell: usize, value: usize) -> Option<twa::VariableNode> {
        let variable = match &self.sudoku_variables {
            Some(SudokuVariablesState::Compact(variables)) => {
                variables.candidates.get(cell)?.get(value).copied()?
            }
            Some(SudokuVariablesState::Full(variables)) => {
                variables.get(cell)?.get(value).copied()?
            }
            None => return None,
        };
        variable.is_valid().then_some(variable)
    }

    fn variable_is_certain(&self, variable: twa::VariableNode) -> bool {
        self.graph
            .as_ref()
            .is_some_and(|graph| graph.weight(variable) == MessageWeight::Infinite)
    }

    fn selected_cell_is_certain(&self, state: &[i32], cell: usize) -> bool {
        let Some(&value) = state.get(cell) else {
            return false;
        };
        if value < 0 {
            return false;
        }
        self.selected_candidate(cell, value as usize)
            .is_some_and(|variable| self.variable_is_certain(variable))
    }

    fn intersection_factor(&self, first: usize, second: usize) -> Option<FactorNode> {
        let variables = self.circle_variables.as_ref()?;
        let (first, second) = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        variables
            .intersection_factors
            .get(first)?
            .get(second - first - 1)
            .copied()
    }

    fn circle_pair_enabled(&self, first: usize, second: usize) -> bool {
        let (Some(graph), Some(factor)) = (&self.graph, self.intersection_factor(first, second))
        else {
            return false;
        };
        graph.is_factor_enabled(factor)
    }

    fn draw_control_panel(&mut self, ctx: &Context) {
        SidePanel::left("controls")
            .resizable(false)
            .default_width(315.0)
            .show(ctx, |ui| {
                ui.heading("TWA Visualizer");
                ui.add_space(6.0);

                let mut rebuild_now = false;
                ComboBox::from_label("Problem")
                    .selected_text(self.domain.label())
                    .show_ui(ui, |ui| {
                        rebuild_now |= ui
                            .selectable_value(&mut self.domain, ProblemDomain::Sudoku, "Sudoku")
                            .changed();
                        rebuild_now |= ui
                            .selectable_value(
                                &mut self.domain,
                                ProblemDomain::CirclePacking,
                                "Circle Packing",
                            )
                            .changed();
                    });

                match self.domain {
                    ProblemDomain::Sudoku => self.draw_sudoku_controls(ui, &mut rebuild_now),
                    ProblemDomain::CirclePacking => self.draw_circle_controls(ui, &mut rebuild_now),
                }

                ui.separator();
                self.draw_solver_controls(ui, &mut rebuild_now);

                if rebuild_now {
                    self.rebuild();
                }

                ui.separator();
                self.draw_status(ui);
            });
    }

    fn draw_sudoku_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) {
        let selected_name = self.puzzle_spec().name;
        ComboBox::from_label("Puzzle")
            .selected_text(selected_name)
            .show_ui(ui, |ui| {
                for index in 0..self.puzzles.len() {
                    if ui
                        .selectable_value(&mut self.puzzle_index, index, self.puzzles[index].name)
                        .changed()
                    {
                        self.selected_cell = 0;
                        *rebuild_now = true;
                    }
                }
            });

        ComboBox::from_label("Build")
            .selected_text(self.sudoku_build.label())
            .show_ui(ui, |ui| {
                *rebuild_now |= ui
                    .selectable_value(&mut self.sudoku_build, SudokuBuild::Compact, "Compact")
                    .changed();
                *rebuild_now |= ui
                    .selectable_value(&mut self.sudoku_build, SudokuBuild::Full, "Full")
                    .changed();
            });
    }

    fn draw_circle_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) {
        ComboBox::from_label("Circle Set")
            .selected_text(self.circle_problem().name)
            .show_ui(ui, |ui| {
                for index in 0..self.circle_problems.len() {
                    if ui
                        .selectable_value(
                            &mut self.circle_problem_index,
                            index,
                            self.circle_problems[index].name,
                        )
                        .changed()
                    {
                        self.selected_circle = 0;
                        self.circle_count = base_circle_count(self.circle_problem());
                        *rebuild_now = true;
                    }
                }
            });

        ComboBox::from_label("Build")
            .selected_text(self.circle_build.label())
            .show_ui(ui, |ui| {
                *rebuild_now |= ui
                    .selectable_value(&mut self.circle_build, CircleBuild::Fast, "Fast")
                    .changed();
                *rebuild_now |= ui
                    .selectable_value(&mut self.circle_build, CircleBuild::Full, "Full")
                    .changed();
            });

        let mut settings_changed = false;
        settings_changed |= ui
            .add(Slider::new(&mut self.circle_count, 1..=1_000).text("Circle count"))
            .changed();
        settings_changed |= ui
            .add(
                DragValue::new(&mut self.circle_density_target)
                    .speed(0.005)
                    .range(0.0..=1.2)
                    .prefix("Density "),
            )
            .changed();
        if self.circle_build == CircleBuild::Fast {
            settings_changed |= ui
                .add(
                    DragValue::new(&mut self.nearby_radius_scale)
                        .speed(0.05)
                        .range(0.0..=8.0)
                        .prefix("Nearby scale "),
                )
                .changed();
        }
        if settings_changed {
            self.settings_dirty = true;
        }
    }

    fn draw_solver_controls(&mut self, ui: &mut egui::Ui, rebuild_now: &mut bool) {
        let mut settings_changed = false;

        let mut learning_rate = self.active_learning_rate();
        if ui
            .add(
                DragValue::new(&mut learning_rate)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .prefix("Learning rate "),
            )
            .changed()
        {
            self.set_active_learning_rate(learning_rate);
            settings_changed = true;
        }

        settings_changed |= ui
            .add(
                DragValue::new(&mut self.convergence_delta)
                    .speed(0.000001)
                    .range(0.0..=1.0)
                    .prefix("Delta "),
            )
            .changed();
        settings_changed |= ui
            .add(
                DragValue::new(&mut self.random_seed)
                    .speed(1)
                    .range(0..=u64::MAX)
                    .prefix("Seed "),
            )
            .changed();
        settings_changed |= ui
            .add(
                DragValue::new(&mut self.max_iterations)
                    .speed(100)
                    .range(1..=1_000_000)
                    .prefix("Max iterations "),
            )
            .changed();
        ui.add(
            DragValue::new(&mut self.iterations_per_frame)
                .speed(1)
                .range(1..=500)
                .prefix("Iterations/frame "),
        );

        if settings_changed {
            self.settings_dirty = true;
        }

        if self.settings_dirty {
            ui.colored_label(Color32::from_rgb(226, 196, 94), "Settings changed");
        }

        ui.horizontal(|ui| {
            if ui.button("Reset").clicked() {
                *rebuild_now = true;
            }
            if ui.button("Step").clicked() {
                self.apply_settings_if_dirty();
                self.step_many(1);
            }
            let run_label = if self.running { "Pause" } else { "Run" };
            if ui.button(run_label).clicked() {
                self.apply_settings_if_dirty();
                self.running = !self.running;
            }
        });

        if ui.button("Run to convergence").clicked() {
            self.apply_settings_if_dirty();
            self.run_to_convergence();
        }
    }

    fn draw_status(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.last_error {
            ui.colored_label(Color32::from_rgb(210, 72, 76), error);
            return;
        }

        let Some(graph) = &self.graph else {
            ui.label("No active graph");
            return;
        };

        ui.label(format!("Iterations: {}", graph.iterations()));
        ui.label(format!(
            "Converged: {}",
            if graph.converged() { "yes" } else { "no" }
        ));

        match self.domain {
            ProblemDomain::Sudoku => {
                if self.puzzle_spec().solution.is_some() {
                    let state = self.sudoku_state();
                    ui.label(format!(
                        "Matches solution: {}",
                        if self.matches_solution(&state) {
                            "yes"
                        } else {
                            "no"
                        }
                    ));
                }
            }
            ProblemDomain::CirclePacking => {
                ui.label(format!("Max overlap: {:.8}", self.circle_max_overlap()));
                ui.label(format!("Circles: {}", self.initial_circles.len()));
                ui.label(format!("Density: {:.3}", self.circle_density()));
                ui.label(format!(
                    "Intersections: {} / {} enabled",
                    self.enabled_intersection_factors(),
                    self.total_intersection_factors()
                ));
            }
        }

        ui.label(format!("Variables: {}", graph.num_variables()));
        ui.label(format!(
            "Factors: {} / {} enabled",
            graph.num_enabled_factors(),
            graph.num_factors()
        ));
        ui.label(format!(
            "Edges: {} / {} enabled",
            graph.num_enabled_edges(),
            graph.num_edges()
        ));
        ui.label(format!("Last step: {:.3} ms", self.last_step_ms));
    }

    fn draw_sudoku(&mut self, ctx: &Context) {
        SidePanel::right("selected_cell")
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                self.draw_candidate_panel(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Sudoku");
            ui.add_space(8.0);
            let state = self.sudoku_state();
            self.draw_sudoku_grid(ui, &state);
        });
    }

    fn draw_sudoku_grid(&mut self, ui: &mut egui::Ui, state: &[i32]) {
        let Some(puzzle) = &self.puzzle else {
            ui.label("No Sudoku puzzle loaded");
            return;
        };

        let side = puzzle.outer_side;
        let available = ui.available_size();
        let max_width = available.x.clamp(280.0, 760.0);
        let max_height = available.y.clamp(280.0, 760.0);
        let board_size = max_width.min(max_height);
        let cell_size = board_size / side as f32;
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(board_size), Sense::click());
        handle_board_click(&response, rect, side, &mut self.selected_cell);

        let painter = ui.painter_at(rect);
        let solved = self.matches_solution(state);
        for row in 0..side {
            for column in 0..side {
                let cell = row * side + column;
                let cell_rect = Rect::from_min_size(
                    Pos2::new(
                        rect.left() + column as f32 * cell_size,
                        rect.top() + row as f32 * cell_size,
                    ),
                    Vec2::splat(cell_size),
                );
                let selected = self.selected_cell == cell;
                let certain = self.selected_cell_is_certain(state, cell);
                let given = puzzle.givens.contains_key(&cell);
                painter.rect_filled(
                    cell_rect,
                    0.0,
                    sudoku_cell_fill(selected, given, certain, solved),
                );
                painter.rect_stroke(
                    cell_rect,
                    0.0,
                    Stroke::new(1.0, Color32::from_rgb(80, 86, 96)),
                    StrokeKind::Inside,
                );
                if certain {
                    let color = if given {
                        Color32::from_rgb(198, 204, 214)
                    } else {
                        Color32::from_rgb(226, 196, 94)
                    };
                    painter.rect_stroke(
                        cell_rect.shrink(2.0),
                        0.0,
                        Stroke::new(2.0, color),
                        StrokeKind::Inside,
                    );
                }

                if let Some(&value) = state.get(cell)
                    && value >= 0
                {
                    painter.text(
                        cell_rect.center(),
                        Align2::CENTER_CENTER,
                        value_label(value),
                        FontId::proportional((cell_size * 0.44).clamp(12.0, 24.0)),
                        Color32::from_rgb(236, 239, 244),
                    );
                }
            }
        }

        for index in 0..=side {
            let major = index % puzzle.inner_side == 0;
            let stroke = if major {
                Stroke::new(2.0, Color32::from_rgb(210, 216, 226))
            } else {
                Stroke::new(1.0, Color32::from_rgb(92, 98, 108))
            };
            let offset = index as f32 * cell_size;
            painter.line_segment(
                [
                    Pos2::new(rect.left() + offset, rect.top()),
                    Pos2::new(rect.left() + offset, rect.bottom()),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.left(), rect.top() + offset),
                    Pos2::new(rect.right(), rect.top() + offset),
                ],
                stroke,
            );
        }
    }

    fn draw_candidate_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected Cell");
        let Some(puzzle) = &self.puzzle else {
            ui.label("No cell selected");
            return;
        };

        let side = puzzle.outer_side;
        let state = self.sudoku_state();
        let cell_count = side * side;
        self.selected_cell = self.selected_cell.min(cell_count.saturating_sub(1));
        let row = self.selected_cell / side;
        let column = self.selected_cell % side;

        ui.label(format!("Row {}, column {}", row + 1, column + 1));
        if let Some(&value) = state.get(self.selected_cell) {
            ui.label(format!("Extracted value: {}", value_label(value)));
        }
        if let Some(&given) = puzzle.givens.get(&self.selected_cell) {
            ui.label(format!("Given: {}", value_label(given as i32)));
        }
        if let Some(solution) = &self.puzzle_spec().solution
            && let Some(&value) = solution.get(self.selected_cell)
        {
            ui.label(format!("Solution: {}", value_label(value)));
        }

        ui.separator();
        let mut has_candidates = false;
        for value in 0..side {
            let Some(variable) = self.selected_candidate(self.selected_cell, value) else {
                continue;
            };
            has_candidates = true;
            let raw_confidence = self
                .graph
                .as_ref()
                .map_or(0.0, |graph| graph.value(variable));
            let confidence = raw_confidence.clamp(0.0, 1.0) as f32;
            let certain = self.variable_is_certain(variable);
            let text = if certain {
                format!(
                    "{}  {:.3}  certain",
                    value_label(value as i32),
                    raw_confidence
                )
            } else {
                format!("{}  {:.3}", value_label(value as i32), raw_confidence)
            };
            ui.add(ProgressBar::new(confidence).text(text));
        }

        if !has_candidates {
            ui.label("No candidate variables in compact build.");
        }
    }

    fn draw_circle_packing(&mut self, ctx: &Context) {
        SidePanel::right("selected_circle")
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                self.draw_selected_circle_panel(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Circle Packing");
            ui.horizontal(|ui| {
                ui.label(format!("Circles: {}", self.initial_circles.len()));
                ui.separator();
                ui.label(format!("Density: {:.3}", self.circle_density()));
                ui.separator();
                ui.label(format!("Max overlap: {:.8}", self.circle_max_overlap()));
            });
            ui.add_space(8.0);

            let circles = self.packed_circles();
            self.draw_circle_canvas(ui, &circles);
        });
    }

    fn draw_circle_canvas(&mut self, ui: &mut egui::Ui, circles: &[Circle]) {
        let problem = self.circle_problem().clone();
        let horizontal_span = range_span(problem.horizontal_range);
        let vertical_span = range_span(problem.vertical_range);
        if horizontal_span <= 0.0 || vertical_span <= 0.0 {
            ui.label("Invalid coordinate range");
            return;
        }

        let available = ui.available_size();
        let width = available.x.clamp(320.0, 880.0);
        let height = (width * (vertical_span / horizontal_span) as f32)
            .min(available.y.max(320.0))
            .max(280.0);
        let scale = width / horizontal_span as f32;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        self.handle_circle_click(&response, rect, scale, height, &problem, circles);

        let has_overlap = circles
            .iter()
            .enumerate()
            .map(|(index, circle)| {
                visually_overlaps(boundary_overlap(*circle, &problem))
                    || circles.iter().enumerate().any(|(other, other_circle)| {
                        other != index && visually_overlaps(pair_overlap(*circle, *other_circle))
                    })
            })
            .collect::<Vec<_>>();

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(210, 216, 226)),
            StrokeKind::Inside,
        );

        if self.circle_build == CircleBuild::Fast {
            self.draw_enabled_intersections(&painter, rect, scale, height, &problem, circles);
        }

        for (index, &circle) in circles.iter().enumerate() {
            let center = world_to_screen(&problem, circle, rect, scale, height);
            let radius = (circle.radius as f32 * scale).max(1.0);
            let selected = self.selected_circle == index;
            let fill = if selected {
                Color32::from_rgba_unmultiplied(62, 102, 142, 230)
            } else {
                Color32::from_rgba_unmultiplied(48, 92, 118, 215)
            };
            let outline = if has_overlap[index] {
                Color32::from_rgb(210, 72, 76)
            } else {
                Color32::from_rgb(166, 204, 214)
            };
            painter.circle_filled(center, radius, fill);
            painter.circle_stroke(
                center,
                radius,
                Stroke::new(if has_overlap[index] { 3.0 } else { 1.5 }, outline),
            );
            if selected {
                painter.circle_stroke(
                    center,
                    radius + 4.0,
                    Stroke::new(2.0, Color32::from_rgb(226, 196, 94)),
                );
            }
        }
    }

    fn draw_enabled_intersections(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        scale: f32,
        height: f32,
        problem: &CircleProblem,
        circles: &[Circle],
    ) {
        let mut drawn = 0;
        for first in 0..circles.len().saturating_sub(1) {
            for second in first + 1..circles.len() {
                if drawn >= MAX_DRAWN_CONNECTION_LINES {
                    return;
                }
                if !self.circle_pair_enabled(first, second) {
                    continue;
                }

                painter.line_segment(
                    [
                        world_to_screen(problem, circles[first], rect, scale, height),
                        world_to_screen(problem, circles[second], rect, scale, height),
                    ],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(226, 196, 94, 145)),
                );
                drawn += 1;
            }
        }
    }

    fn handle_circle_click(
        &mut self,
        response: &Response,
        rect: Rect,
        scale: f32,
        height: f32,
        problem: &CircleProblem,
        circles: &[Circle],
    ) {
        if !response.clicked() || circles.is_empty() {
            return;
        }
        let Some(position) = response.interact_pointer_pos() else {
            return;
        };

        let mut nearest = 0;
        let mut nearest_distance = f32::MAX;
        for (index, &circle) in circles.iter().enumerate() {
            let center = world_to_screen(problem, circle, rect, scale, height);
            let distance = center.distance_sq(position);
            if distance < nearest_distance {
                nearest = index;
                nearest_distance = distance;
            }
        }
        self.selected_circle = nearest;
    }

    fn draw_selected_circle_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected Circle");
        let circles = self.packed_circles();
        if circles.is_empty() {
            ui.label("No circles");
            return;
        }

        self.selected_circle = self.selected_circle.min(circles.len() - 1);
        let selected = self.selected_circle;
        let circle = circles[selected];
        let problem = self.circle_problem();

        let enabled_neighbors = (0..circles.len())
            .filter(|&other| other != selected && self.circle_pair_enabled(selected, other))
            .count();
        let max_pair_overlap = circles
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != selected)
            .map(|(_, &other_circle)| pair_overlap(circle, other_circle))
            .fold(0.0, f64::max);

        ui.label(format!("Circle {} of {}", selected + 1, circles.len()));
        ui.label(format!("x: {:.6}", circle.x));
        ui.label(format!("y: {:.6}", circle.y));
        ui.label(format!("radius: {:.6}", circle.radius));
        ui.separator();
        ui.label(format!(
            "Boundary overlap: {:.8}",
            boundary_overlap(circle, problem).max(0.0)
        ));
        ui.label(format!("Pair overlap: {:.8}", max_pair_overlap.max(0.0)));
        ui.label(format!("Enabled neighbors: {enabled_neighbors}"));
    }
}

impl Default for SolverApp {
    fn default() -> Self {
        let mut app = Self {
            puzzles: make_puzzles(),
            circle_problems: make_circle_problems(),
            domain: ProblemDomain::CirclePacking,
            puzzle_index: 2,
            sudoku_build: SudokuBuild::Compact,
            circle_problem_index: 1,
            circle_build: CircleBuild::Fast,
            random_seed: 42,
            sudoku_learning_rate: 1.0,
            circle_learning_rate: 0.3,
            convergence_delta: 1e-7,
            max_iterations: 100_000,
            iterations_per_frame: 1,
            circle_count: 200,
            circle_density_target: 0.82,
            nearby_radius_scale: 1.4,
            selected_cell: 0,
            selected_circle: 0,
            running: false,
            settings_dirty: false,
            last_step_ms: 0.0,
            graph: None,
            puzzle: None,
            sudoku_variables: None,
            initial_circles: Vec::new(),
            circle_variables: None,
            last_error: None,
        };
        app.rebuild();
        app
    }
}

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

fn apply_visuals(ctx: &Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(24, 27, 31);
    visuals.window_fill = Color32::from_rgb(28, 31, 36);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 43, 49);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 62, 70);
    visuals.widgets.active.bg_fill = Color32::from_rgb(64, 86, 96);
    visuals.selection.bg_fill = Color32::from_rgb(62, 102, 142);
    ctx.set_visuals(visuals);
}

fn make_puzzles() -> Vec<PuzzleSpec> {
    vec![
        PuzzleSpec {
            name: "2x2",
            path: "data/sudoku/example_2x2.txt",
            solution: Some(vec![0, 1, 2, 3, 3, 2, 1, 0, 1, 0, 3, 2, 2, 3, 0, 1]),
        },
        PuzzleSpec {
            name: "3x3",
            path: "data/sudoku/example_3x3.txt",
            solution: Some(vec![
                4, 2, 3, 5, 6, 7, 8, 0, 1, 5, 6, 1, 0, 8, 4, 2, 3, 7, 0, 8, 7, 2, 3, 1, 4, 5, 6, 7,
                4, 8, 6, 5, 0, 3, 1, 2, 3, 1, 5, 7, 4, 2, 6, 8, 0, 6, 0, 2, 8, 1, 3, 7, 4, 5, 8, 5,
                0, 4, 2, 6, 1, 7, 3, 1, 7, 6, 3, 0, 8, 5, 2, 4, 2, 3, 4, 1, 7, 5, 0, 6, 8,
            ]),
        },
        PuzzleSpec {
            name: "4x4 medium",
            path: "data/sudoku/example_4x4_medium.txt",
            solution: None,
        },
        PuzzleSpec {
            name: "4x4 hard",
            path: "data/sudoku/example_4x4_hard.txt",
            solution: None,
        },
        PuzzleSpec {
            name: "5x5",
            path: "data/sudoku/example_5x5.txt",
            solution: None,
        },
    ]
}

fn make_circle_problems() -> Vec<CircleProblem> {
    vec![
        CircleProblem {
            name: "equal radii",
            horizontal_range: unit_range(),
            vertical_range: unit_range(),
            radii: vec![RadiusCount {
                radius: 0.040,
                count: 200,
            }],
        },
        CircleProblem {
            name: "mixed radii",
            horizontal_range: unit_range(),
            vertical_range: unit_range(),
            radii: vec![
                RadiusCount {
                    radius: 0.035,
                    count: 118,
                },
                RadiusCount {
                    radius: 0.060,
                    count: 60,
                },
                RadiusCount {
                    radius: 0.085,
                    count: 22,
                },
            ],
        },
    ]
}

fn unit_range() -> CoordinateRange {
    CoordinateRange {
        lower: 0.0,
        upper: 1.0,
    }
}

fn base_circle_count(problem: &CircleProblem) -> usize {
    problem.radii.iter().map(|entry| entry.count).sum()
}

fn range_span(range: CoordinateRange) -> f64 {
    range.upper - range.lower
}

fn value_label(value: i32) -> String {
    if value < 0 {
        ".".to_owned()
    } else {
        (value + 1).to_string()
    }
}

fn sudoku_cell_fill(selected: bool, given: bool, certain: bool, solved: bool) -> Color32 {
    if selected {
        Color32::from_rgb(72, 110, 160)
    } else if given {
        if certain {
            Color32::from_rgb(62, 66, 72)
        } else {
            Color32::from_rgb(44, 54, 66)
        }
    } else if certain {
        Color32::from_rgb(58, 70, 42)
    } else if solved {
        Color32::from_rgb(36, 74, 54)
    } else {
        Color32::from_rgb(28, 30, 34)
    }
}

fn handle_board_click(response: &Response, rect: Rect, side: usize, selected_cell: &mut usize) {
    if !response.clicked() {
        return;
    }
    let Some(position) = response.interact_pointer_pos() else {
        return;
    };
    if !rect.contains(position) {
        return;
    }

    let cell_size = rect.width() / side as f32;
    let column = ((position.x - rect.left()) / cell_size).floor() as usize;
    let row = ((position.y - rect.top()) / cell_size).floor() as usize;
    if row < side && column < side {
        *selected_cell = row * side + column;
    }
}

fn boundary_overlap(circle: Circle, problem: &CircleProblem) -> f64 {
    let mut overlap: f64 = 0.0;
    overlap = overlap.max(problem.horizontal_range.lower - (circle.x - circle.radius));
    overlap = overlap.max((circle.x + circle.radius) - problem.horizontal_range.upper);
    overlap = overlap.max(problem.vertical_range.lower - (circle.y - circle.radius));
    overlap = overlap.max((circle.y + circle.radius) - problem.vertical_range.upper);
    overlap
}

fn pair_overlap(first: Circle, second: Circle) -> f64 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    first.radius + second.radius - dx.hypot(dy)
}

fn visually_overlaps(overlap: f64) -> bool {
    overlap > VISUAL_OVERLAP_EPSILON
}

fn world_to_screen(
    problem: &CircleProblem,
    circle: Circle,
    rect: Rect,
    scale: f32,
    height: f32,
) -> Pos2 {
    Pos2::new(
        rect.left() + ((circle.x - problem.horizontal_range.lower) as f32 * scale),
        rect.top() + height - ((circle.y - problem.vertical_range.lower) as f32 * scale),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_radii_preserve_target_count_and_density() {
        let mut app = SolverApp::default();
        app.domain = ProblemDomain::CirclePacking;
        app.circle_problem_index = 1;
        app.circle_count = 37;
        app.circle_density_target = 0.5;

        let radii = app.scaled_radii();
        let count = radii.iter().map(|entry| entry.count).sum::<usize>();
        let area = range_span(app.circle_problem().horizontal_range)
            * range_span(app.circle_problem().vertical_range);
        let density = radii
            .iter()
            .map(|entry| entry.count as f64 * std::f64::consts::PI * entry.radius * entry.radius)
            .sum::<f64>()
            / area;

        assert_eq!(count, 37);
        assert!((density - 0.5).abs() < 1e-12);
    }

    #[test]
    fn gui_sudoku_path_matches_cli_solution_for_2x2() {
        let mut app = SolverApp {
            domain: ProblemDomain::Sudoku,
            puzzle_index: 0,
            sudoku_build: SudokuBuild::Compact,
            ..Default::default()
        };
        app.rebuild();
        app.run_to_convergence();

        assert!(app.graph.as_ref().is_some_and(FactorGraph::converged));
        assert_eq!(
            app.sudoku_state(),
            app.puzzle_spec().solution.as_deref().unwrap()
        );
    }
}
