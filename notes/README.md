# Notes

This directory contains prose explanations of the `twa` codebase, a Rust port
of the Three-Weight Algorithm for message-passing on factor graphs.

The notes are meant to make the implementation readable. They explain the
algorithm concepts, the role of message weights, the Rust design choices, and
the source files that carry those ideas.

## Who These Notes Are For

The primary audience is a programmer or researcher who wants to understand how
the Three-Weight Algorithm works and how this repository implements it. You do
not need deep Rust experience. Two background notes explain the Rust features
used here for readers coming from different languages:

1. **[concepts/rust_for_python_readers.md](concepts/rust_for_python_readers.md)** - Rust ideas for readers more
   comfortable in Python.
2. **[concepts/rust_for_cpp_readers.md](concepts/rust_for_cpp_readers.md)** - Rust ideas for readers more
   comfortable in C++.

Readers who already know Rust can skip those and start with the factor-graph
concept notes.

## Directory Structure

The notes are organized into two categories:

- **`concepts/`** - Standalone explanations of ideas that span multiple source
  files. These cover Rust idioms, factor graphs, message passing, message
  weights, Sudoku, and circle packing. Algorithm and domain concept notes also
  point to the source notes where each idea is implemented.
- **`src/`** - File-by-file walkthroughs that mirror the Rust source tree.
  Each source note covers one Rust file in source order.

Source notes are named after the file they document. For example:

- [src/factor_graph.rs.md](src/factor_graph.rs.md) documents
  `src/factor_graph.rs`.
- [src/problems/circle_packing.rs.md](src/problems/circle_packing.rs.md)
  documents `src/problems/circle_packing.rs`.

User-facing guides live in `docs/`.

## Suggested Reading Order

The notes are designed to be read roughly in this order. You can skip ahead if
a topic is already familiar.

### 1. Rust Background

Read one of these only if Rust is still new or if a source note refers to a
language feature you want explained.

1. **[concepts/rust_for_python_readers.md](concepts/rust_for_python_readers.md)** - Modules, ownership, borrowing,
   `Result`, `Option`, structs, enums, closures, slices, vectors, tests, and
   command-line code from a Python reader's point of view.
2. **[concepts/rust_for_cpp_readers.md](concepts/rust_for_cpp_readers.md)** - Crates, module privacy, RAII,
   ownership, `Copy`, `Option`, `Result`, enums, traits, boxed closures,
   slices, vectors, and Cargo from a C++ reader's point of view.

### 2. Algorithm Foundations

Start here to build the mental model used by every source file.

3. **[concepts/factor_graphs.md](concepts/factor_graphs.md)** - What a factor graph is, and how this
   crate represents variables, factors, and edges.
4. **[concepts/message_passing.md](concepts/message_passing.md)** - The iteration model: factor pass,
   variable pass, message exchange, and convergence.
5. **[concepts/weights.md](concepts/weights.md)** - The three message weights: zero, standard, and
   infinite.

### 3. Crate Boundary And Public Types

These notes introduce the public API surface and the small value and handle
types that appear everywhere else.

6. **[src/lib.rs.md](src/lib.rs.md)** - The crate root, module declarations, internal modules, and
   public re-exports.
7. **[src/weighted_value.rs.md](src/weighted_value.rs.md)** - `MessageWeight`, `WeightedValue`, and
   `WeightedValueExchange`.
8. **[src/variable_node.rs.md](src/variable_node.rs.md)** - The `VariableNode` handle.
9. **[src/factor_node.rs.md](src/factor_node.rs.md)** - The `FactorNode` handle.
10. **[src/graph_edge.rs.md](src/graph_edge.rs.md)** - The `GraphEdge` handle.

### 4. Internal Graph Storage

Read these in order. They explain the private state that makes the public
graph API work.

11. **[src/edge_data.rs.md](src/edge_data.rs.md)** - Per-edge `x`, `z`, `u` state, weights, enabled
    state, and convergence tracking.
12. **[src/variable_data.rs.md](src/variable_data.rs.md)** - Per-variable initial value, current
    consensus, connected edges, and lazy enabled-edge cache.
13. **[src/factor_data.rs.md](src/factor_data.rs.md)** - Per-factor edge exchanges, minimization
    dispatch, and enabled-state behavior.
14. **[src/factor_graph.rs.md](src/factor_graph.rs.md)** - The central graph owner: construction,
    iteration, convergence, reinitialization, callbacks, and factor
    enablement.

### 5. Built-In Minimizers

Minimizers are the "brains" of factors. They decide what messages a factor
sends.

15. **[src/minimizers/mod.rs.md](src/minimizers/mod.rs.md)** - Minimizer module declarations and
    re-exports.
16. **[src/minimizers/known_value.rs.md](src/minimizers/known_value.rs.md)** - A fixed value with
    infinite weight.
17. **[src/minimizers/in_range.rs.md](src/minimizers/in_range.rs.md)** - A scalar range constraint
    that clamps only when needed.
18. **[src/minimizers/one_hot.rs.md](src/minimizers/one_hot.rs.md)** - A one-hot constraint that picks
    exactly one variable to be 1.
19. **[src/minimizers/spy.rs.md](src/minimizers/spy.rs.md)** - A user-provided callback minimizer for
    observation or custom control.

### 6. Sudoku Problem Builders

These notes show how a concrete discrete problem is assembled from graph
variables and factors.

20. **[concepts/sudoku.md](concepts/sudoku.md)** - How Sudoku cells, candidate values, givens, and
    uniqueness constraints map onto a factor graph.
21. **[src/problems/mod.rs.md](src/problems/mod.rs.md)** - Problem-builder module declarations and
    public re-exports.
22. **[src/problems/sudoku.rs.md](src/problems/sudoku.rs.md)** - Puzzle parsing, direct Sudoku graph
    construction, and state extraction.
23. **[src/problems/compact_sudoku.rs.md](src/problems/compact_sudoku.rs.md)** - Compact Sudoku graph
    construction that prunes variables using givens.
24. **[src/bin/sudoku.rs.md](src/bin/sudoku.rs.md)** - The command-line Sudoku solver frontend.

### 7. Circle Packing

Circle packing exercises continuous variables, zero-weight satisfied
constraints, and dynamic factor enablement.

25. **[concepts/circle_packing.md](concepts/circle_packing.md)** - How boundary, intersection, and
    kiss constraints map onto a factor graph.
26. **[src/problems/circle_packing.rs.md](src/problems/circle_packing.rs.md)** - Circle generation,
    direct and fast builders, intersection factors, kiss factors, extraction,
    overlap measurement, and the dynamic intersection manager.

### 8. GUI Visualizer

The GUI combines Sudoku and circle packing in one interactive native
application.

27. **[src/bin/gui.rs.md](src/bin/gui.rs.md)** - The `egui`/`eframe` visualizer, including
    problem selection, controls, Sudoku drawing, circle-packing drawing, smoke
    testing, and feature-gated build setup.

The external user guide is:

28. **[../docs/gui_help.md](../docs/gui_help.md)** - Running and using the GUI visualizer.

## Tips For Reading

- **Read concept notes before matching source notes.** For example,
  [concepts/weights.md](concepts/weights.md) makes
  [src/weighted_value.rs.md](src/weighted_value.rs.md) and the minimizer notes
  much easier to follow.

- **Use source notes as annotated source.** Central source notes include the
  important Rust code in source order with nearby explanation. They are meant
  to be useful even when the `.rs` file is not open.

- **Expect private helpers to matter.** Much of the graph behavior lives in
  private methods such as consensus calculation, edge reset logic, and dynamic
  factor toggling. Source notes explain those helpers because they are part of
  the implementation even when they are not public API.

- **Follow cross-references.** When a source note links to
  [concepts/message_passing.md](concepts/message_passing.md) or
  [concepts/weights.md](concepts/weights.md), the linked note usually carries
  the broader algorithm idea behind the local code.

- **Separate algorithm behavior from Rust mechanics.** If a Rust feature is
  unfamiliar, check one of the reader notes first. If the Rust syntax is clear
  but the graph behavior is not, return to the factor-graph, message-passing,
  and weights notes.

- **Treat the reading order as a path, not a rule.** If you are working on a
  minimizer, jump to that minimizer. If you are working on Sudoku or circle
  packing, read the matching concept note first, then the builder note.

## Related Project Documents

- [../README.md](../README.md) - Project overview and build commands.
- [../docs/gui_help.md](../docs/gui_help.md) - GUI visualizer usage.

## The Paper

The algorithm is described in:

> N. Derbinsky, J. Bento, V. Elser, J.S. Yedidia, "An Improved Three-Weight Message-Passing Algorithm,"
> https://arxiv.org/abs/1305.1961
