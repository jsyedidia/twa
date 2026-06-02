use std::fs;
use std::process::Command;

fn run_sudoku(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_sudoku"))
        .args(args)
        .output()
        .expect("failed to run sudoku binary")
}

fn read_grid(path: &str) -> Vec<Vec<String>> {
    fs::read_to_string(path)
        .expect("failed to read solution fixture")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn printed_solution(stdout: &str) -> Vec<Vec<String>> {
    let (_, solution) = stdout
        .split_once("== Solution ==")
        .expect("stdout should contain a solution section");

    solution
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn compact_solver_prints_known_2x2_solution() {
    let output = run_sudoku(&[
        "--builder",
        "compact",
        "--max-iterations",
        "1000",
        "--print-solution",
        "data/sudoku/example_2x2.txt",
    ]);

    assert!(
        output.status.success(),
        "sudoku failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("configuration: builder=compact"));
    assert!(stdout.contains("did converge"));
    assert_eq!(
        printed_solution(&stdout),
        read_grid("data/sudoku/solutions/example_2x2_solution.txt")
    );
}

#[test]
fn full_solver_prints_known_2x2_solution() {
    let output = run_sudoku(&[
        "--builder",
        "full",
        "--max-iterations",
        "1000",
        "--print-solution",
        "data/sudoku/example_2x2.txt",
    ]);

    assert!(
        output.status.success(),
        "sudoku failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("configuration: builder=full"));
    assert!(stdout.contains("did converge"));
    assert_eq!(
        printed_solution(&stdout),
        read_grid("data/sudoku/solutions/example_2x2_solution.txt")
    );
}

#[test]
fn compact_solver_prints_known_3x3_solution() {
    let output = run_sudoku(&[
        "--builder",
        "compact",
        "--max-iterations",
        "2000",
        "--print-solution",
        "data/sudoku/example_3x3.txt",
    ]);

    assert!(
        output.status.success(),
        "sudoku failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("configuration: builder=compact"));
    assert!(stdout.contains("did converge"));
    assert_eq!(
        printed_solution(&stdout),
        read_grid("data/sudoku/solutions/example_3x3_solution.txt")
    );
}

#[test]
fn missing_puzzle_path_returns_error_status() {
    let output = run_sudoku(&["data/sudoku/does_not_exist.txt"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("sudoku failed:"),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
