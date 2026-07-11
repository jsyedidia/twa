# Rust For Python Readers

This note explains the Rust features that appear in `twa`. It is written for
readers who are comfortable with Python and want enough Rust to read the
factor-graph source notes without detouring into a full Rust textbook.

The emphasis is concrete: each section names the kind of code in this repo
that uses the idea.

## Crates, Modules, And Binaries

A Rust package can contain a library crate and one or more binary crates. In
this repo:

- `src/lib.rs` is the library root.
- `src/factor_graph.rs`, `src/weighted_value.rs`, and the other files under
  `src/` are library modules.
- `src/bin/sudoku.rs` is an executable.
- `src/bin/gui.rs` is another executable, available with the `gui` feature.

The library root declares public modules:

```rust
pub mod factor_graph;
pub mod factor_node;
pub mod graph_edge;
pub mod minimizers;
pub mod problems;
pub mod variable_node;
pub mod weighted_value;
```

This is a little like an `__init__.py` deciding which submodules belong to a
Python package, except Rust checks the module graph at compile time.

Client code refers to public items through module paths:

```rust
use twa::FactorGraph;
use twa::MessageWeight;
```

The `::` operator means "look inside this module, trait, or type."

## Static Types

Python usually discovers type mistakes while running. Rust checks them before
the program runs.

This signature says exactly what comes in and what goes out:

```rust
pub fn create_variable(
    &mut self,
    initial_value: f64,
    initial_weight: MessageWeight,
) -> VariableNode
```

It mutates an existing graph, receives a floating-point value and a message
weight, and returns a typed variable handle.

Important types in this repo include:

- `f64`: floating-point values used for messages and coordinates.
- `usize`: indexes, counts, and vector lengths.
- `bool`: convergence and enabled-state flags.
- `u64`: random seeds.
- `Vec<T>`: an owned growable list.
- `Option<T>`: a value that may be present or absent.
- `Result<T, E>`: success or recoverable failure.

Rust's type names often carry algorithm meaning. `VariableNode`,
`FactorNode`, and `GraphEdge` are not just integers; they are distinct handle
types so code cannot accidentally pass a factor where a variable is expected.

## Values, References, And Mutation

Python variables are names bound to objects. Rust distinguishes owned values
from borrowed references.

This function mutates an existing graph:

```rust
pub fn create_known_value_factor(
    graph: &mut FactorGraph,
    variable: VariableNode,
    value: f64,
) -> FactorNode
```

`&mut FactorGraph` means "borrow this graph mutably and exclusively." While
the function runs, no other code can also mutate the same graph.

This function only reads a graph:

```rust
pub fn extract_circles(
    graph: &FactorGraph,
    variables: &CirclePackingVariables,
) -> Vec<Circle>
```

`&FactorGraph` and `&CirclePackingVariables` are shared borrows. The function
can inspect them but cannot modify them.

Small handles are passed by value:

```rust
variable: VariableNode
```

That is cheap because handles are tiny `Copy` types.

## Ownership Without Garbage Collection

Rust does not use a tracing garbage collector. Values are dropped when their
owner goes out of scope.

`FactorGraph` owns the graph storage:

```rust
variables: Vec<VariableData>,
edges: Vec<EdgeData>,
factors: Vec<FactorData>,
```

When the graph is dropped, those vectors release their memory automatically.

Heap storage appears where the size is dynamic:

- graph arenas use `Vec`;
- Sudoku builders store variable grids in `Vec`;
- circle packing stores circle handles and factor handles in `Vec`;
- factor minimizers and callbacks use `Box<dyn FnMut(...)>` for stored
  closures.

Most small values, such as `WeightedValue`, `VariableNode`, and `Circle`, are
copied directly.

## `struct`

A Rust `struct` groups named fields with fixed types:

```rust
pub struct WeightedValue {
    pub value: f64,
    pub weight: MessageWeight,
}
```

This is closer to a Python dataclass than a dictionary. Field names and types
are checked by the compiler.

Fields are private unless marked `pub`. `FactorGraph` is public, but its
fields are private so callers preserve graph invariants through methods.

Public structs can expose fields when they are simple data:

```rust
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}
```

## `enum`

Rust enums represent one of several named cases:

```rust
pub enum MessageWeight {
    Zero,
    Standard,
    Infinite,
}
```

This is more precise than using strings such as `"zero"` or integers such as
`0`, `1`, and `2`.

Enums can also carry data. The Sudoku parser's error type uses this style so
each failure can keep useful context, such as an invalid token, its row and
column, or an underlying I/O error.

## `impl` Blocks And Methods

Methods live in `impl` blocks:

```rust
impl WeightedValue {
    pub fn new(value: f64, weight: MessageWeight) -> Self {
        Self { value, weight }
    }
}
```

Inside `impl WeightedValue`, `Self` means `WeightedValue`.

Rust does not put methods inside the struct definition itself. Fields are
declared in one place, and methods are grouped in one or more `impl` blocks.

Methods that receive `&self` read an existing value. Methods that receive
`&mut self` mutate an existing value. Functions without `self`, such as
`WeightedValue::new`, are associated functions.

## `Result` Instead Of Exceptions

Rust usually represents recoverable failure with `Result<T, E>`:

```rust
pub fn read_sudoku_puzzle(
    path: impl AsRef<Path>,
) -> Result<SudokuPuzzle, SudokuParseError>
```

This returns `Ok(SudokuPuzzle)` on success or `Err(SudokuParseError)` on
failure.

`impl AsRef<Path>` means callers may pass any type that can be viewed as a
path, including `&Path`, `PathBuf`, and path-like string values.

The `?` operator propagates errors:

```rust
parse_sudoku_puzzle(&fs::read_to_string(path)?)
```

If reading fails, the surrounding function returns that error immediately. If
it succeeds, the file contents are borrowed by `parse_sudoku_puzzle`.

This repo uses panics for programmer errors, such as invalid graph handles or
impossible geometry:

```rust
assert!(
    exact_distance >= 0.0,
    "create_kiss_factor requires exact_distance >= 0"
);
```

That is different from parse errors, which are expected runtime failures and
therefore use `Result`.

## `Option` Instead Of `None`-Capable Values

Rust uses `Option<T>` when a value may be absent:

```rust
kissing_circle: Option<KissingCircle>
```

An `Option<T>` is either `Some(value)` or `None`.

The circle-packing builder handles the present case with:

```rust
if let Some(kissing_circle) = kissing_circle {
    create_kiss_factor(/* ... */);
}
```

This is like:

```python
if kissing_circle is not None:
    create_kiss_factor(...)
```

but the possibility of absence is explicit in the type.

## Pattern Matching

`match` handles enum variants and other patterns:

```rust
match self {
    MessageWeight::Zero => 0.0,
    MessageWeight::Standard => 1.0,
    MessageWeight::Infinite => f64::INFINITY,
}
```

The compiler checks that all cases are handled.

Shorter pattern forms appear too:

```rust
let Some(neighbor_indexes) = grid.get(&neighbor) else {
    continue;
};
```

This means "continue unless the lookup found a value." If a value exists,
`neighbor_indexes` is available after the statement.

## Traits And Derived Traits

A trait is a set of behavior a type can implement. It is similar to a Python
protocol, but checked statically.

Many traits in this repo are generated with `#[derive(...)]`:

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageWeight {
    Zero,
    #[default]
    Standard,
    Infinite,
}
```

Common derived traits here include:

- `Debug`: developer-oriented formatting.
- `Default`: a default value.
- `Clone`: explicit duplication.
- `Copy`: cheap implicit copying.
- `PartialEq` and `Eq`: equality tests.
- `Hash`: use as a hash-map key.

The `Default` trait lets callers write:

```rust
let mut graph = FactorGraph::default();
```

## Trait Objects And Closures

The factor graph stores minimizers as closures. The type alias is:

```rust
pub type MinimizationFn =
    Box<dyn FnMut(&mut [WeightedValueExchange], &mut dyn rand::RngCore)>;
```

The pieces mean:

- `Box<...>`: heap-owned value.
- `dyn FnMut(...)`: some callable object whose exact type is not named.
- `&mut [WeightedValueExchange]`: mutable borrowed list of exchanges.
- `&mut dyn rand::RngCore`: mutable random generator interface.

Closures use vertical bars for parameters:

```rust
Box::new(move |exchanges, _| {
    let incoming = exchanges[0].get();
    exchanges[0].set(WeightedValue::new(incoming.value, MessageWeight::Zero));
})
```

The `move` keyword stores captured values inside the closure. That matters
because the factor keeps the closure after the constructor returns.

## Shared Callback State

Most code in this repo uses ordinary ownership. The fast circle-packing
builder needs shared mutable state between two graph callbacks, so it uses:

```rust
let manager = Rc::new(RefCell::new(DynamicIntersectionManager::new(/* ... */)));
```

`Rc<T>` is reference-counted ownership for single-threaded code. `RefCell<T>`
allows checked mutable borrowing at runtime. This combination is useful for
callbacks, where the ordinary compile-time borrow pattern would be too rigid.

You do not need this pattern for most Rust code in the repo. It is used in a
small, contained place.

## Arrays, Slices, And Vectors

Rust has several sequence-like types:

- `[T; N]`: fixed-size array.
- `&[T]`: borrowed slice.
- `&mut [T]`: mutable borrowed slice.
- `Vec<T>`: growable heap-allocated vector.

Problem builders accept borrowed slices:

```rust
pub fn add_circle_packing_to_factor_graph(
    graph: &mut FactorGraph,
    circles: &[Circle],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    kissing_circle: Option<KissingCircle>,
) -> CirclePackingVariables
```

The caller keeps ownership of `circles`; the builder only reads it.

The graph owns vectors because the number of variables, edges, and factors is
not known at compile time.

## Iterators

Rust iterators are lazy chains of operations over sequences. They are similar
to Python generator pipelines, but statically typed.

This graph query counts enabled factors:

```rust
self.factors.iter().filter(|f| f.is_enabled()).count()
```

`.iter()` borrows each factor, `.filter(...)` keeps the enabled ones, and
`.count()` consumes the iterator to produce a number.

For simple mutation, ordinary loops are common:

```rust
for factor in &mut self.factors {
    factor.minimize(&mut self.edges, self.rng.as_mut());
}
```

## Strings, Paths, And Command-Line Code

Rust distinguishes borrowed string slices from owned strings:

- `&str`: borrowed string data.
- `String`: owned growable string.

Filesystem paths use:

- `&Path`: borrowed path.
- `PathBuf`: owned path buffer.

The Sudoku parser reads from a path:

```rust
pub fn read_sudoku_puzzle(
    path: impl AsRef<Path>,
) -> Result<SudokuPuzzle, SudokuParseError>
```

The Sudoku CLI owns the parsed path:

```rust
struct Options {
    puzzle_path: PathBuf,
}
```

## Macros And Attributes

Macros end in `!` and are expanded by the compiler:

```rust
assert_eq!(graph.num_factors(), 9);
println!("{solution}");
```

Attributes start with `#[]` and attach metadata to the item that follows:

```rust
#![forbid(unsafe_code)]
#[derive(Debug, Clone)]
#[cfg(test)]
#[test]
```

The command-line binary uses `clap` attributes:

```rust
#[derive(Debug, Parser)]
#[command(name = "sudoku")]
```

Those attributes generate command-line parsing code from the options struct.

## Tests

Rust unit tests often live near the code they test:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn known_value_converges() {
        /* ... */
    }
}
```

Tests can access private items from the module that contains them, which is
useful for graph internals. The Sudoku binary also has tests for CLI-facing
formatting and option names.

Run all tests with:

```text
cargo test
```

## Reading This Repo

For algorithm code, start with:

- [factor_graphs.md](factor_graphs.md)
- [message_passing.md](message_passing.md)
- [weights.md](weights.md)
- [src/factor_graph.rs.md](../src/factor_graph.rs.md)

For problem builders, read:

- [sudoku.md](sudoku.md)
- [circle_packing.md](circle_packing.md)

For Rust mechanics, watch for the recurring pattern:

1. Public APIs use explicit types and typed handles.
2. Builders borrow a graph mutably while they add variables, edges, and
   factors.
3. Factors store boxed closures that read and write weighted-value exchanges.
4. Recoverable errors use `Result`; invalid programmer inputs use assertions.
