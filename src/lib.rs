#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod factor_graph;
pub mod factor_node;
pub mod graph_edge;
pub mod minimizers;
pub mod problems;
pub mod variable_node;
pub mod weighted_value;

pub(crate) mod edge_data;
pub(crate) mod factor_data;
pub(crate) mod variable_data;

pub use factor_graph::FactorGraph;
pub use factor_node::FactorNode;
pub use graph_edge::GraphEdge;
pub use minimizers::{
    create_in_range_factor, create_known_value_factor, create_one_hot_factor, create_spy_factor,
};
pub use problems::{
    Circle, CirclePackingVariables, CompactSudokuVariables, CoordinateRange, KissingCircle,
    RadiusCount, SudokuGivens, SudokuPuzzle, SudokuVariables, add_circle_packing_to_factor_graph,
    add_circle_packing_to_factor_graph_fast, add_compact_sudoku_to_factor_graph,
    add_sudoku_to_factor_graph, create_intersection_factor, create_kiss_factor, extract_circles,
    extract_compact_sudoku_state, extract_sudoku_state, generate_circles, max_overlap,
    read_sudoku_puzzle,
};
pub use variable_node::VariableNode;
pub use weighted_value::{MessageWeight, MinimizationFn, WeightedValue, WeightedValueExchange};
