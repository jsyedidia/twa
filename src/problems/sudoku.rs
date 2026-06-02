use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::num::ParseIntError;
use std::path::Path;

use crate::{FactorGraph, MessageWeight, VariableNode, create_one_hot_factor};

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

impl fmt::Display for SudokuParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SudokuParseError::Io(error) => write!(f, "{error}"),
            SudokuParseError::Empty => write!(f, "Sudoku problem is empty"),
            SudokuParseError::RowCountNotSquare { rows } => {
                write!(f, "Sudoku row count must be a perfect square: {rows}")
            }
            SudokuParseError::NotSquare {
                row,
                columns,
                expected,
            } => write!(
                f,
                "Sudoku problem must be square; row {row} has {columns} columns, expected {expected}"
            ),
            SudokuParseError::InvalidToken {
                row, column, token, ..
            } => write!(
                f,
                "Invalid Sudoku token at row {row}, column {column}: {token}"
            ),
            SudokuParseError::ClueOutOfRange {
                row,
                column,
                token,
                outer_side,
            } => write!(
                f,
                "Sudoku clue out of range at row {row}, column {column}: {token} (expected 1..={outer_side})"
            ),
        }
    }
}

impl std::error::Error for SudokuParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SudokuParseError::Io(error) => Some(error),
            SudokuParseError::InvalidToken { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for SudokuParseError {
    fn from(value: io::Error) -> Self {
        SudokuParseError::Io(value)
    }
}

/// Reads a whitespace-separated Sudoku puzzle file.
///
/// Empty cells are represented by `.`. Clues are one-based in the file and are
/// converted to zero-based values in the returned puzzle.
pub fn read_sudoku_puzzle(path: impl AsRef<Path>) -> Result<SudokuPuzzle, SudokuParseError> {
    parse_sudoku_puzzle(&fs::read_to_string(path)?)
}

/// Parses a whitespace-separated Sudoku puzzle string.
pub fn parse_sudoku_puzzle(input: &str) -> Result<SudokuPuzzle, SudokuParseError> {
    let rows: Vec<Vec<&str>> = input
        .lines()
        .map(|line| line.split_whitespace().collect())
        .filter(|row: &Vec<&str>| !row.is_empty())
        .collect();

    if rows.is_empty() {
        return Err(SudokuParseError::Empty);
    }

    let outer_side = rows.len();
    let inner_side =
        square_root(outer_side).ok_or(SudokuParseError::RowCountNotSquare { rows: outer_side })?;

    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != outer_side {
            return Err(SudokuParseError::NotSquare {
                row: row_index + 1,
                columns: row.len(),
                expected: outer_side,
            });
        }
    }

    let mut givens = SudokuGivens::new();
    for (row_index, row) in rows.iter().enumerate() {
        for (column_index, &token) in row.iter().enumerate() {
            if token == "." {
                continue;
            }
            let value =
                token
                    .parse::<usize>()
                    .map_err(|source| SudokuParseError::InvalidToken {
                        row: row_index + 1,
                        column: column_index + 1,
                        token: token.to_owned(),
                        source,
                    })?;
            if value == 0 || value > outer_side {
                return Err(SudokuParseError::ClueOutOfRange {
                    row: row_index + 1,
                    column: column_index + 1,
                    token: token.to_owned(),
                    outer_side,
                });
            }
            givens.insert(row_index * outer_side + column_index, value - 1);
        }
    }

    Ok(SudokuPuzzle {
        inner_side,
        outer_side,
        givens,
    })
}

/// Adds a direct Sudoku factor graph and returns candidate variables.
///
/// The direct builder creates one variable for every `(cell, value)` pair.
///
/// # Panics
///
/// Panics if `inner_side` is zero or any given is out of range.
pub fn add_sudoku_to_factor_graph(
    graph: &mut FactorGraph,
    inner_side: usize,
    givens: &SudokuGivens,
) -> SudokuVariables {
    let outer_side = outer_side_for(inner_side, "Sudoku inner_side must be positive");
    validate_givens(givens, outer_side, "Sudoku");

    let mut variables = Vec::with_capacity(outer_side * outer_side);

    for cell in 0..outer_side * outer_side {
        let given = givens.get(&cell).copied();
        let given_value = given.unwrap_or(outer_side);
        let initial_weight = if given.is_some() {
            MessageWeight::Infinite
        } else {
            MessageWeight::Standard
        };

        let mut options = Vec::with_capacity(outer_side);
        for value in 0..outer_side {
            let initial_value = if value == given_value { 1.0 } else { 0.0 };
            options.push(graph.create_variable(initial_value, initial_weight));
        }

        create_one_hot_factor(graph, &options);
        variables.push(options);
    }

    for index in 0..outer_side {
        for (value, _) in variables[0].iter().enumerate() {
            let mut row_value_variables = Vec::with_capacity(outer_side);
            let mut column_value_variables = Vec::with_capacity(outer_side);
            let mut square_value_variables = Vec::with_capacity(outer_side);

            for offset in 0..outer_side {
                row_value_variables.push(variables[cell_index(index, offset, outer_side)][value]);
                column_value_variables
                    .push(variables[cell_index(offset, index, outer_side)][value]);

                let square_row = index / inner_side;
                let square_column = index % inner_side;
                let row_in_square = offset / inner_side;
                let column_in_square = offset % inner_side;
                let row = square_row * inner_side + row_in_square;
                let column = square_column * inner_side + column_in_square;
                square_value_variables.push(variables[cell_index(row, column, outer_side)][value]);
            }

            create_one_hot_factor(graph, &row_value_variables);
            create_one_hot_factor(graph, &column_value_variables);
            create_one_hot_factor(graph, &square_value_variables);
        }
    }

    variables
}

/// Extracts the selected zero-based value for each cell.
///
/// A candidate with graph value greater than `0.99` is considered selected.
/// Cells with no selected candidate are reported as `-1`.
pub fn extract_sudoku_state(graph: &FactorGraph, variables: &SudokuVariables) -> Vec<i32> {
    let mut state = Vec::with_capacity(variables.len());

    for options in variables {
        let mut selected_value = -1;
        for (value, &variable) in options.iter().enumerate() {
            if graph.value(variable) > 0.99 {
                selected_value = value as i32;
            }
        }
        state.push(selected_value);
    }

    state
}

pub(crate) fn outer_side_for(inner_side: usize, message: &str) -> usize {
    assert!(inner_side > 0, "{message}");
    inner_side * inner_side
}

pub(crate) fn cell_index(row: usize, column: usize, outer_side: usize) -> usize {
    row * outer_side + column
}

pub(crate) fn square_root(value: usize) -> Option<usize> {
    let root = (value as f64).sqrt() as usize;
    if root * root == value {
        Some(root)
    } else if (root + 1) * (root + 1) == value {
        Some(root + 1)
    } else {
        None
    }
}

pub(crate) fn validate_givens(givens: &SudokuGivens, outer_side: usize, name: &str) {
    let num_cells = outer_side * outer_side;
    for (&cell, &value) in givens {
        assert!(cell < num_cells, "{name} given cell is out of range");
        assert!(value < outer_side, "{name} given value is out of range");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_graph_size(graph: &FactorGraph, outer_side: usize) {
        assert_eq!(graph.num_variables(), outer_side * outer_side * outer_side);
        assert_eq!(graph.num_factors(), 4 * outer_side * outer_side);
        assert_eq!(graph.num_edges(), 4 * outer_side * outer_side * outer_side);
        assert_eq!(graph.num_enabled_factors(), graph.num_factors());
        assert_eq!(graph.num_enabled_edges(), graph.num_edges());
    }

    #[test]
    fn size_checks_for_4x4() {
        let mut graph = FactorGraph::default();
        let givens = SudokuGivens::from([(0, 0), (5, 1), (10, 2), (15, 3)]);

        let variables = add_sudoku_to_factor_graph(&mut graph, 2, &givens);

        assert_eq!(variables.len(), 16);
        for options in &variables {
            assert_eq!(options.len(), 4);
        }
        expect_graph_size(&graph, 4);
    }

    #[test]
    fn size_checks_for_9x9() {
        let mut graph = FactorGraph::default();
        let givens = SudokuGivens::from([(0, 4), (10, 2), (80, 8)]);

        let variables = add_sudoku_to_factor_graph(&mut graph, 3, &givens);

        assert_eq!(variables.len(), 81);
        for options in &variables {
            assert_eq!(options.len(), 9);
        }
        expect_graph_size(&graph, 9);
    }

    #[test]
    fn extract_state_reports_thresholded_values() {
        let mut graph = FactorGraph::default();
        let givens = SudokuGivens::from([(0, 2), (3, 1)]);

        let variables = add_sudoku_to_factor_graph(&mut graph, 2, &givens);
        let initial_state = extract_sudoku_state(&graph, &variables);

        assert_eq!(initial_state.len(), 16);
        assert_eq!(initial_state[0], 2);
        assert_eq!(initial_state[3], 1);
        assert_eq!(initial_state[1], -1);
    }

    #[test]
    #[should_panic(expected = "inner_side must be positive")]
    fn rejects_zero_inner_side() {
        let mut graph = FactorGraph::default();
        add_sudoku_to_factor_graph(&mut graph, 0, &SudokuGivens::new());
    }

    #[test]
    #[should_panic(expected = "given cell is out of range")]
    fn rejects_out_of_range_given_cell() {
        let mut graph = FactorGraph::default();
        add_sudoku_to_factor_graph(&mut graph, 2, &SudokuGivens::from([(16, 0)]));
    }

    #[test]
    #[should_panic(expected = "given value is out of range")]
    fn rejects_out_of_range_given_value() {
        let mut graph = FactorGraph::default();
        add_sudoku_to_factor_graph(&mut graph, 2, &SudokuGivens::from([(0, 4)]));
    }

    #[test]
    fn parses_puzzle_file_format() {
        let puzzle = parse_sudoku_puzzle(
            "1 . 3 .\n\
             . 3 . 1\n\
             2 . 4 .\n\
             . 4 . 2\n",
        )
        .unwrap();

        assert_eq!(puzzle.inner_side, 2);
        assert_eq!(puzzle.outer_side, 4);
        assert_eq!(puzzle.givens[&0], 0);
        assert_eq!(puzzle.givens[&2], 2);
        assert_eq!(puzzle.givens[&15], 1);
    }

    #[test]
    fn rejects_invalid_puzzle_shape_and_tokens() {
        assert!(matches!(
            parse_sudoku_puzzle("1 2 3\n1 2 3\n1 2 3\n"),
            Err(SudokuParseError::RowCountNotSquare { .. })
        ));
        assert!(matches!(
            parse_sudoku_puzzle("1 2 3 4\n1 2 3\n1 2 3 4\n1 2 3 4\n"),
            Err(SudokuParseError::NotSquare { .. })
        ));
        assert!(matches!(
            parse_sudoku_puzzle("x . . .\n. . . .\n. . . .\n. . . .\n"),
            Err(SudokuParseError::InvalidToken { .. })
        ));
        assert!(matches!(
            parse_sudoku_puzzle("5 . . .\n. . . .\n. . . .\n. . . .\n"),
            Err(SudokuParseError::ClueOutOfRange { .. })
        ));
    }

    #[test]
    fn solves_4x4_puzzle() {
        let mut graph = FactorGraph::new(1.0, 1e-5, 0);
        let givens = SudokuGivens::from([
            (0, 0),
            (2, 2),
            (5, 2),
            (7, 0),
            (8, 1),
            (10, 3),
            (13, 3),
            (15, 1),
        ]);
        let solution = vec![0, 1, 2, 3, 3, 2, 1, 0, 1, 0, 3, 2, 2, 3, 0, 1];

        let variables = add_sudoku_to_factor_graph(&mut graph, 2, &givens);

        assert!(graph.iterate_until_converged(200));
        assert_eq!(extract_sudoku_state(&graph, &variables), solution);
    }
}
