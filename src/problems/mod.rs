//! Problem builders built on top of the factor graph API.

pub mod circle_packing;
pub mod compact_sudoku;
pub mod sudoku;

pub use circle_packing::{
    Circle, CirclePackingVariables, CoordinateRange, KissingCircle, RadiusCount,
    add_circle_packing_to_factor_graph, add_circle_packing_to_factor_graph_fast,
    create_intersection_factor, create_kiss_factor, extract_circles, generate_circles, max_overlap,
};
pub use compact_sudoku::{
    CompactSudokuVariables, add_compact_sudoku_to_factor_graph, extract_compact_sudoku_state,
};
pub use sudoku::{
    SudokuGivens, SudokuPuzzle, SudokuVariables, add_sudoku_to_factor_graph, extract_sudoku_state,
    read_sudoku_puzzle,
};
