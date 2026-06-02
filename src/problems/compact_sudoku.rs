use crate::{FactorGraph, VariableNode, create_one_hot_factor};

use super::sudoku::{SudokuGivens, cell_index, outer_side_for, validate_givens};

/// Compact Sudoku candidate variables and metadata.
#[derive(Debug, Clone)]
pub struct CompactSudokuVariables {
    pub inner_side: usize,
    pub outer_side: usize,
    pub givens: SudokuGivens,
    pub candidates: Vec<Vec<VariableNode>>,
}

/// Adds a compact Sudoku factor graph and returns surviving candidates.
///
/// Given cells and candidates ruled out by givens in the same row, column, or
/// square are omitted.
///
/// # Panics
///
/// Panics if `inner_side` is zero, any given is out of range, or a non-given
/// cell has no surviving candidates.
pub fn add_compact_sudoku_to_factor_graph(
    graph: &mut FactorGraph,
    inner_side: usize,
    givens: &SudokuGivens,
) -> CompactSudokuVariables {
    let outer_side = outer_side_for(inner_side, "Compact_sudoku inner_side must be positive");
    validate_givens(givens, outer_side, "Compact_sudoku");

    let mut variables = CompactSudokuVariables {
        inner_side,
        outer_side,
        givens: givens.clone(),
        candidates: vec![vec![VariableNode::default(); outer_side]; outer_side * outer_side],
    };

    for row in 0..outer_side {
        for column in 0..outer_side {
            let cell = cell_index(row, column, outer_side);
            if givens.contains_key(&cell) {
                continue;
            }

            let mut cell_candidates = Vec::with_capacity(outer_side);
            for value in 0..outer_side {
                if value_found(givens, inner_side, outer_side, row, column, value) {
                    continue;
                }

                let variable = graph.create_variable(0.0, crate::MessageWeight::Standard);
                variables.candidates[cell][value] = variable;
                cell_candidates.push(variable);
            }

            assert!(
                !cell_candidates.is_empty(),
                "Compact_sudoku cell has no surviving candidates"
            );
            create_one_hot_factor(graph, &cell_candidates);
        }
    }

    for index in 0..outer_side {
        for value in 0..outer_side {
            let mut row_value_variables = Vec::with_capacity(outer_side);
            let mut column_value_variables = Vec::with_capacity(outer_side);
            let mut square_value_variables = Vec::with_capacity(outer_side);

            for offset in 0..outer_side {
                let row_variable =
                    variables.candidates[cell_index(index, offset, outer_side)][value];
                if row_variable.is_valid() {
                    row_value_variables.push(row_variable);
                }

                let column_variable =
                    variables.candidates[cell_index(offset, index, outer_side)][value];
                if column_variable.is_valid() {
                    column_value_variables.push(column_variable);
                }

                let square_row = index / inner_side;
                let square_column = index % inner_side;
                let row_in_square = offset / inner_side;
                let column_in_square = offset % inner_side;
                let row = square_row * inner_side + row_in_square;
                let column = square_column * inner_side + column_in_square;
                let square_variable =
                    variables.candidates[cell_index(row, column, outer_side)][value];
                if square_variable.is_valid() {
                    square_value_variables.push(square_variable);
                }
            }

            add_one_hot_if_nonempty(graph, &row_value_variables);
            add_one_hot_if_nonempty(graph, &column_value_variables);
            add_one_hot_if_nonempty(graph, &square_value_variables);
        }
    }

    variables
}

/// Extracts the selected zero-based value for each cell.
///
/// Givens are copied directly into the state. Non-given cells with no selected
/// surviving candidate are reported as `-1`.
pub fn extract_compact_sudoku_state(
    graph: &FactorGraph,
    variables: &CompactSudokuVariables,
) -> Vec<i32> {
    let mut state = Vec::with_capacity(variables.candidates.len());

    for cell in 0..variables.candidates.len() {
        if let Some(&given) = variables.givens.get(&cell) {
            state.push(given as i32);
            continue;
        }

        let mut selected_value = -1;
        for (value, &variable) in variables.candidates[cell].iter().enumerate() {
            if variable.is_valid() && graph.value(variable) > 0.99 {
                selected_value = value as i32;
            }
        }
        state.push(selected_value);
    }

    state
}

fn square_index(row: usize, column: usize, inner_side: usize) -> usize {
    column / inner_side + inner_side * (row / inner_side)
}

fn square_base_row(square: usize, inner_side: usize) -> usize {
    (square / inner_side) * inner_side
}

fn square_base_column(square: usize, inner_side: usize) -> usize {
    (square % inner_side) * inner_side
}

fn has_given_value(givens: &SudokuGivens, cell: usize, value: usize) -> bool {
    givens.get(&cell).is_some_and(|&given| given == value)
}

fn value_found(
    givens: &SudokuGivens,
    inner_side: usize,
    outer_side: usize,
    row: usize,
    column: usize,
    value: usize,
) -> bool {
    for candidate_column in 0..outer_side {
        if has_given_value(givens, cell_index(row, candidate_column, outer_side), value) {
            return true;
        }
    }

    for candidate_row in 0..outer_side {
        if has_given_value(givens, cell_index(candidate_row, column, outer_side), value) {
            return true;
        }
    }

    let square = square_index(row, column, inner_side);
    let base_row = square_base_row(square, inner_side);
    let base_column = square_base_column(square, inner_side);
    for row_offset in 0..inner_side {
        for column_offset in 0..inner_side {
            if has_given_value(
                givens,
                cell_index(
                    base_row + row_offset,
                    base_column + column_offset,
                    outer_side,
                ),
                value,
            ) {
                return true;
            }
        }
    }

    false
}

fn add_one_hot_if_nonempty(graph: &mut FactorGraph, candidates: &[VariableNode]) {
    if !candidates.is_empty() {
        create_one_hot_factor(graph, candidates);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problems::sudoku::read_sudoku_puzzle;

    fn count_candidates(variables: &CompactSudokuVariables) -> usize {
        variables
            .candidates
            .iter()
            .flatten()
            .filter(|variable| variable.is_valid())
            .count()
    }

    fn expect_compact_graph_size(
        path: &str,
        expected_factors: usize,
        expected_variables: usize,
        expected_edges: usize,
    ) {
        let puzzle = read_sudoku_puzzle(path).unwrap();
        let mut graph = FactorGraph::default();
        let variables =
            add_compact_sudoku_to_factor_graph(&mut graph, puzzle.inner_side, &puzzle.givens);

        assert_eq!(variables.inner_side, puzzle.inner_side);
        assert_eq!(variables.outer_side, puzzle.outer_side);
        assert_eq!(
            variables.candidates.len(),
            puzzle.outer_side * puzzle.outer_side
        );
        assert_eq!(count_candidates(&variables), expected_variables);
        assert_eq!(graph.num_factors(), expected_factors);
        assert_eq!(graph.num_variables(), expected_variables);
        assert_eq!(graph.num_edges(), expected_edges);
        assert_eq!(graph.num_enabled_factors(), expected_factors);
        assert_eq!(graph.num_enabled_edges(), expected_edges);
    }

    #[test]
    fn compact_graph_counts_match_reference_construction() {
        expect_compact_graph_size("data/sudoku/example_2x2.txt", 32, 8, 32);
        expect_compact_graph_size("data/sudoku/example_3x3.txt", 204, 153, 612);
        expect_compact_graph_size("data/sudoku/example_4x4_medium.txt", 632, 784, 3136);
        expect_compact_graph_size("data/sudoku/example_4x4_hard.txt", 800, 1657, 6628);
        expect_compact_graph_size("data/sudoku/example_5x5.txt", 1356, 1860, 7440);
    }

    #[test]
    fn compact_is_smaller_than_direct_for_9x9_fixture() {
        let puzzle = read_sudoku_puzzle("data/sudoku/example_3x3.txt").unwrap();
        let mut graph = FactorGraph::default();
        let variables =
            add_compact_sudoku_to_factor_graph(&mut graph, puzzle.inner_side, &puzzle.givens);

        assert_eq!(count_candidates(&variables), 153);
        assert!(graph.num_variables() < puzzle.outer_side * puzzle.outer_side * puzzle.outer_side);
        assert!(graph.num_edges() < 4 * puzzle.outer_side * puzzle.outer_side * puzzle.outer_side);
    }

    #[test]
    fn extract_state_includes_givens() {
        let mut graph = FactorGraph::default();
        let givens = SudokuGivens::from([(0, 0), (5, 1)]);
        let variables = add_compact_sudoku_to_factor_graph(&mut graph, 2, &givens);
        let state = extract_compact_sudoku_state(&graph, &variables);

        assert_eq!(state.len(), 16);
        assert_eq!(state[0], 0);
        assert_eq!(state[5], 1);
    }

    #[test]
    #[should_panic(expected = "inner_side must be positive")]
    fn rejects_zero_inner_side() {
        let mut graph = FactorGraph::default();
        add_compact_sudoku_to_factor_graph(&mut graph, 0, &SudokuGivens::new());
    }

    #[test]
    #[should_panic(expected = "given cell is out of range")]
    fn rejects_out_of_range_given_cell() {
        let mut graph = FactorGraph::default();
        add_compact_sudoku_to_factor_graph(&mut graph, 2, &SudokuGivens::from([(16, 0)]));
    }

    #[test]
    #[should_panic(expected = "given value is out of range")]
    fn rejects_out_of_range_given_value() {
        let mut graph = FactorGraph::default();
        add_compact_sudoku_to_factor_graph(&mut graph, 2, &SudokuGivens::from([(0, 4)]));
    }

    #[test]
    fn solves_2x2_fixture() {
        let puzzle = read_sudoku_puzzle("data/sudoku/example_2x2.txt").unwrap();
        let mut graph = FactorGraph::new(1.0, 1e-5, 42);
        let variables =
            add_compact_sudoku_to_factor_graph(&mut graph, puzzle.inner_side, &puzzle.givens);

        assert!(graph.iterate_until_converged(1000));
        assert_eq!(
            extract_compact_sudoku_state(&graph, &variables),
            vec![0, 1, 2, 3, 3, 2, 1, 0, 1, 0, 3, 2, 2, 3, 0, 1]
        );
    }

    #[test]
    fn solves_3x3_fixture() {
        let puzzle = read_sudoku_puzzle("data/sudoku/example_3x3.txt").unwrap();
        let mut graph = FactorGraph::new(1.0, 1e-5, 42);
        let variables =
            add_compact_sudoku_to_factor_graph(&mut graph, puzzle.inner_side, &puzzle.givens);

        assert!(graph.iterate_until_converged(1000));
        assert_eq!(
            extract_compact_sudoku_state(&graph, &variables),
            vec![
                4, 2, 3, 5, 6, 7, 8, 0, 1, 5, 6, 1, 0, 8, 4, 2, 3, 7, 0, 8, 7, 2, 3, 1, 4, 5, 6, 7,
                4, 8, 6, 5, 0, 3, 1, 2, 3, 1, 5, 7, 4, 2, 6, 8, 0, 6, 0, 2, 8, 1, 3, 7, 4, 5, 8, 5,
                0, 4, 2, 6, 1, 7, 3, 1, 7, 6, 3, 0, 8, 5, 2, 4, 2, 3, 4, 1, 7, 5, 0, 6, 8,
            ]
        );
    }
}
