use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use twa::{
    FactorGraph, add_compact_sudoku_to_factor_graph, add_sudoku_to_factor_graph,
    extract_compact_sudoku_state, extract_sudoku_state, read_sudoku_puzzle,
};

const DEFAULT_PROBLEM_PATH: &str = "data/sudoku/example_4x4_medium.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Builder {
    Compact,
    Full,
}

#[derive(Debug, Parser)]
#[command(
    name = "sudoku",
    about = "Solve a Sudoku text file using the Three-Weight Algorithm"
)]
struct Options {
    /// Path to a Sudoku text file using `.` for empty cells.
    puzzle_file: Option<PathBuf>,

    /// Path to a Sudoku text file using `.` for empty cells.
    #[arg(long = "problem_path", value_name = "PATH")]
    problem_path: Option<PathBuf>,

    /// Graph builder to use.
    #[arg(long, value_enum, default_value_t = Builder::Compact)]
    builder: Builder,

    /// Dual update learning rate / alpha.
    #[arg(long = "learning_rate", alias = "learning-rate", default_value_t = 1.0)]
    learning_rate: f64,

    /// Per-edge convergence threshold.
    #[arg(
        long = "convergence_delta",
        alias = "convergence-delta",
        default_value_t = 1e-5
    )]
    convergence_delta: f64,

    /// Maximum solver iterations.
    #[arg(
        long = "max_iterations",
        alias = "max-iterations",
        default_value_t = 100_000
    )]
    max_iterations: usize,

    /// RNG seed for reproducible one-hot tie-breaking.
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Print the solved grid after solving.
    #[arg(long = "print_solution", alias = "print-solution")]
    print_solution: bool,
}

fn main() {
    let options = Options::parse();
    let exit_code = match run(&options) {
        Ok(converged) => {
            if converged {
                0
            } else {
                2
            }
        }
        Err(error) => {
            eprintln!("sudoku failed: {error}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(options: &Options) -> Result<bool, Box<dyn std::error::Error>> {
    println!("Solving:");
    print!(" generating problem...");

    let generation_start = Instant::now();
    let problem_path = options
        .problem_path
        .as_deref()
        .or(options.puzzle_file.as_deref())
        .unwrap_or_else(|| DEFAULT_PROBLEM_PATH.as_ref());
    let puzzle = read_sudoku_puzzle(problem_path)?;
    let mut graph = FactorGraph::new(
        options.learning_rate,
        options.convergence_delta,
        options.seed,
    );

    let mut full_variables = None;
    let mut compact_variables = None;
    match options.builder {
        Builder::Compact => {
            compact_variables = Some(add_compact_sudoku_to_factor_graph(
                &mut graph,
                puzzle.inner_side,
                &puzzle.givens,
            ));
        }
        Builder::Full => {
            full_variables = Some(add_sudoku_to_factor_graph(
                &mut graph,
                puzzle.inner_side,
                &puzzle.givens,
            ));
        }
    }

    let generation_milliseconds = generation_start.elapsed().as_secs_f64() * 1000.0;
    println!(
        " done ({} factors, {} variables, {} edges, {} msecs)",
        graph.num_factors(),
        graph.num_variables(),
        graph.num_edges(),
        generation_milliseconds
    );
    println!(
        " configuration: builder={}, learning_rate={}, convergence_delta={}, max_iterations={}, seed={}",
        builder_name(options.builder),
        options.learning_rate,
        options.convergence_delta,
        options.max_iterations,
        options.seed
    );

    print!(" solving problem...");
    let solve_start = Instant::now();
    let converged = graph.iterate_until_converged(options.max_iterations);
    let milliseconds = solve_start.elapsed().as_secs_f64() * 1000.0;
    let milliseconds_per_iteration = if graph.iterations() == 0 {
        0.0
    } else {
        milliseconds / graph.iterations() as f64
    };

    println!(
        " done ({} in {} iterations, {} msecs, {} msec/iteration)",
        if converged {
            "did converge"
        } else {
            "did not converge"
        },
        graph.iterations(),
        milliseconds,
        milliseconds_per_iteration
    );

    if options.print_solution {
        let solution = match options.builder {
            Builder::Compact => extract_compact_sudoku_state(
                &graph,
                compact_variables
                    .as_ref()
                    .expect("compact variables should exist for compact builder"),
            ),
            Builder::Full => extract_sudoku_state(
                &graph,
                full_variables
                    .as_ref()
                    .expect("full variables should exist for full builder"),
            ),
        };
        print_solution(&solution, puzzle.outer_side);
    }

    Ok(converged)
}

fn builder_name(builder: Builder) -> &'static str {
    match builder {
        Builder::Compact => "compact",
        Builder::Full => "full",
    }
}

fn print_solution(solution: &[i32], outer_side: usize) {
    print!("{}", format_solution(solution, outer_side));
}

fn format_solution(solution: &[i32], outer_side: usize) -> String {
    let mut output = String::from("\n== Solution ==\n\n");
    for row in 0..outer_side {
        for column in 0..outer_side {
            if column > 0 {
                output.push('\t');
            }

            let value = solution[row * outer_side + column];
            if value < 0 {
                output.push('.');
            } else {
                output.push_str(&(value + 1).to_string());
            }
        }
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_names_match_cli_values() {
        assert_eq!(builder_name(Builder::Compact), "compact");
        assert_eq!(builder_name(Builder::Full), "full");
    }

    #[test]
    fn formats_solution_grid() {
        assert_eq!(
            format_solution(&[0, -1, 2, 3], 2),
            "\n== Solution ==\n\n1\t.\n3\t4\n"
        );
    }
}
