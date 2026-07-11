# Rust For C++ Readers

This note explains the Rust features that matter for reading `twa`, assuming
you are already comfortable with C++ concepts such as values, references,
RAII, templates, lambdas, const-correctness, and standard containers.

The goal is not to teach all of Rust. The goal is to make this repository's
implementation of factor graphs and the Three-Weight Algorithm readable.

## Crates Instead Of Headers And Translation Units

Rust code is organized around crates and modules rather than headers and
source translation units.

In this repo:

- `src/lib.rs` is the library crate root.
- `src/factor_graph.rs`, `src/weighted_value.rs`, and the other files under
  `src/` are library modules.
- `src/bin/sudoku.rs` is a binary crate.
- `src/bin/gui.rs` is another binary crate, compiled only with the `gui`
  feature.

The library root declares modules explicitly:

```rust
pub mod factor_graph;
pub mod factor_node;
pub mod graph_edge;
pub mod minimizers;
pub mod problems;
pub mod variable_node;
pub mod weighted_value;
```

There are no textual includes for normal code organization. Public API,
private implementation, and method bodies live in Rust modules, and the
compiler checks the whole crate as a unit.

Public re-exports in `src/lib.rs` create the crate-level API:

```rust
pub use factor_graph::FactorGraph;
pub use weighted_value::{MessageWeight, WeightedValue};
```

That lets callers write `twa::FactorGraph` instead of importing from deeper
module paths.

## Visibility Is Private By Default

Rust items are private to their module unless marked `pub`.

```rust
pub struct FactorGraph {
    learning_rate: f64,
    convergence_delta: f64,
    variables: Vec<VariableData>,
    edges: Vec<EdgeData>,
    factors: Vec<FactorData>,
}
```

`FactorGraph` is public, but its fields are private. Users can build and run a
graph through methods, but they cannot mutate the graph's internal arenas
directly.

Some fields are public when a type is deliberately a simple data object:

```rust
pub struct Circle {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}
```

This is comparable to choosing between a C++ class with private data members
and an aggregate struct.

## Module Privacy Instead Of PImpl

Rust usually does not need PImpl for library boundaries. Private fields and
private modules hide implementation details without an extra allocation.

In `twa`, handle types such as `VariableNode` are public, but their inner
indexes are crate-private:

```rust
pub struct VariableNode(pub(crate) usize);
```

Downstream users can hold and pass a `VariableNode`, but only code inside the
crate can read its index. Internal storage types such as `EdgeData`,
`VariableData`, and `FactorData` are not exported at all.

## RAII Without Custom Destructors Everywhere

Rust uses deterministic destruction like C++. Owned values are dropped when
they go out of scope, and owned heap allocations release themselves
automatically.

`FactorGraph` owns several vectors:

```rust
variables: Vec<VariableData>,
edges: Vec<EdgeData>,
factors: Vec<FactorData>,
```

When the graph is dropped, the vectors and their contents are dropped. This
repo does not need a custom destructor. Standard library containers own their
resources directly.

Construction is usually done with associated functions:

```rust
pub fn new(learning_rate: f64, convergence_delta: f64, random_seed: u64) -> Self
```

The `Default` trait is used for the common graph configuration:

```rust
impl Default for FactorGraph {
    fn default() -> Self {
        Self::new(1.0, 1e-5, 0)
    }
}
```

## Ownership, Borrowing, And References

Rust references look familiar but carry stronger compile-time rules.

```rust
pub fn create_in_range_factor(
    graph: &mut FactorGraph,
    variable: VariableNode,
    lower: f64,
    upper: f64,
) -> FactorNode
```

`&mut FactorGraph` is an exclusive mutable borrow. While this function is
building the factor, no other code can also access that graph. `VariableNode`
is passed by value because it is a small copyable handle.

Shared borrows use `&T`:

```rust
pub fn max_overlap(
    graph: &FactorGraph,
    variables: &CirclePackingVariables,
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
) -> f64
```

This function reads graph values but cannot mutate the graph.

## Moves, `Copy`, And `Clone`

Rust assignments move values by default. After moving a non-`Copy` value, the
old binding cannot be used.

Small handle and value types derive `Copy`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactorNode(pub(crate) usize);
```

For these types, assignment copies the bits. That is right for handles and
small geometry values.

Heap-owning types such as `Vec<T>`, `String`, and `Box<T>` are not `Copy`.
Cloning them is explicit:

```rust
variables: variables.clone(),
```

The fast circle-packing manager clones `CirclePackingVariables` because it
needs its own copy of the handle lists inside callbacks.

## No Null References

Safe Rust references cannot be null. Optional values use `Option<T>`:

```rust
pub fn add_circle_packing_to_factor_graph(
    graph: &mut FactorGraph,
    circles: &[Circle],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
    kissing_circle: Option<KissingCircle>,
) -> CirclePackingVariables
```

`Option<KissingCircle>` is either `Some(kissing_circle)` or `None`, similar to
`std::optional<KissingCircle>`.

The builder handles the present case explicitly:

```rust
if let Some(kissing_circle) = kissing_circle {
    create_kiss_factor(/* ... */);
}
```

The type system prevents accidental unchecked null access.

## `Result` And Panics

Rust commonly represents recoverable failure with `Result<T, E>`:

```rust
pub fn read_sudoku_puzzle(
    path: impl AsRef<Path>,
) -> Result<SudokuPuzzle, SudokuParseError>
```

The result is either `Ok(SudokuPuzzle)` or `Err(SudokuParseError)`. The `?`
operator propagates errors:

```rust
parse_sudoku_puzzle(&fs::read_to_string(path)?)
```

`impl AsRef<Path>` accepts any argument type that can provide a borrowed
`Path`, including `&Path`, `PathBuf`, and path-like string values.

Invalid programmer inputs often use `assert!`, which panics on failure:

```rust
assert!(
    lower <= upper,
    "create_in_range_factor requires lower <= upper"
);
```

In this repo, parsing and IO return `Result`; invalid graph handles, inverted
ranges, and impossible builder arguments usually panic.

## Enums Are Tagged Unions

Rust enums are algebraic data types. They are close to a type-safe tagged
union or `std::variant`, but integrated into the language.

The three TWA message weights are represented as an enum:

```rust
pub enum MessageWeight {
    Zero,
    Standard,
    Infinite,
}
```

Methods dispatch with `match`:

```rust
match self {
    MessageWeight::Zero => 0.0,
    MessageWeight::Standard => 1.0,
    MessageWeight::Infinite => f64::INFINITY,
}
```

The compiler checks that every variant is handled.

## Traits Instead Of Inheritance For Shared Behavior

Rust traits define behavior that types can implement. They are closer to C++
concepts plus interfaces than to base classes.

This repo uses many standard traits through derives:

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageWeight {
    Zero,
    #[default]
    Standard,
    Infinite,
}
```

`Debug` supports developer formatting, `Default` supplies a default value,
`Clone` and `Copy` control duplication, and equality/hash traits support
comparisons or map keys.

This repo also uses trait objects for runtime-polymorphic callbacks and
minimizers.

## Trait Objects And Boxed Closures

C++ often uses `std::function` for type-erased callable objects. In `twa`,
minimizers use a boxed trait object:

```rust
pub type MinimizationFn =
    Box<dyn FnMut(&mut [WeightedValueExchange], &mut dyn rand::RngCore)>;
```

`Box<dyn FnMut(...)>` means:

- the callable is heap allocated;
- the concrete closure type is erased;
- the closure can mutate captured state when called.

Factor constructors capture their parameters in closures:

```rust
Box::new(move |exchanges, _| {
    let incoming = exchanges[0].get();
    exchanges[0].set(WeightedValue::new(incoming.value, MessageWeight::Zero));
})
```

The `move` keyword moves captured values into the closure, which lets the
factor store the closure after the constructor returns.

## Shared Callback State: `Rc<RefCell<T>>`

Most of this crate uses ordinary ownership and borrowing. The fast
circle-packing builder needs shared callback state: one callback updates the
dynamic manager after each iteration, and another resets it after
reinitialization.

That code uses:

```rust
let manager = Rc::new(RefCell::new(DynamicIntersectionManager::new(/* ... */)));
```

`Rc<T>` is single-threaded reference counting, similar in spirit to
`std::shared_ptr<T>` without atomic reference counts. `RefCell<T>` moves borrow
checking to runtime for this one object, so callbacks can call
`borrow_mut()` when the graph invokes them.

Use this as a narrow tool. Most graph data remains owned directly by
`FactorGraph`.

## Arrays, Slices, And `Vec`

Rust distinguishes fixed arrays, borrowed slices, and owned vectors:

- `[T; N]`: fixed-size array.
- `&[T]`: borrowed view of contiguous elements.
- `&mut [T]`: mutable borrowed view.
- `Vec<T>`: growable owning vector.

Problem builders accept borrowed slices:

```rust
pub fn generate_circles(
    random_seed: u64,
    radii: &[RadiusCount],
    horizontal_range: CoordinateRange,
    vertical_range: CoordinateRange,
) -> Vec<Circle>
```

The caller keeps ownership of the input radius list. The function returns an
owned `Vec<Circle>`.

Factor minimizers receive mutable slices of exchange entries:

```rust
FnMut(&mut [WeightedValueExchange], &mut dyn rand::RngCore)
```

That is the Rust equivalent of a non-owning mutable span.

## Iterators And Loops

Rust supports explicit `for` loops and iterator chains.

```rust
for factor in &mut self.factors {
    factor.minimize(&mut self.edges, self.rng.as_mut());
}
```

`&mut self.factors` iterates by mutable reference.

Iterator adapters appear when transforming or filtering:

```rust
self.edges.iter().filter(|e| e.is_enabled()).count()
```

`.iter()` borrows each edge, `.filter(...)` keeps enabled ones, and `.count()`
consumes the iterator to produce a length.

## Strings, Paths, And CLI Code

Rust distinguishes borrowed and owned strings:

- `&str`: borrowed string data.
- `String`: owned growable string.

Filesystem paths use:

- `&Path`: borrowed path.
- `PathBuf`: owned path buffer.

The Sudoku CLI uses `clap` derives to turn a struct into command-line parsing:

```rust
#[derive(Debug, Parser)]
struct Options {
    puzzle_path: PathBuf,
}
```

Command-line code owns parsed values because the options struct outlives the
parser call.

## Attributes And Macros

Attributes start with `#[]` and attach metadata to the item that follows:

```rust
#![forbid(unsafe_code)]
#[derive(Debug, Clone)]
#[cfg(test)]
#[test]
```

`#![forbid(unsafe_code)]` applies to the whole crate. `#[cfg(test)]` includes
test-only code only when running tests.

Macros end in `!`:

```rust
assert_eq!(graph.num_variables(), 6);
println!("{solution}");
```

Macros are expanded by the compiler before ordinary type checking.

## Cargo Instead Of CMake

Cargo is Rust's build system and package manager.

| Task | Cargo command |
| --- | --- |
| Build | `cargo build` |
| Test | `cargo test` |
| Release build | `cargo build --release` |
| Format check | `cargo fmt --check` |
| Lint | `cargo clippy` |
| Run Sudoku CLI | `cargo run --bin sudoku -- data/sudoku/example_3x3.txt` |

Dependencies and feature flags live in `Cargo.toml`.

## Reading This Repo

For the algorithm core, start with:

- [factor_graphs.md](factor_graphs.md)
- [message_passing.md](message_passing.md)
- [weights.md](weights.md)
- [src/factor_graph.rs.md](../src/factor_graph.rs.md)

For domain builders, read:

- [sudoku.md](sudoku.md)
- [circle_packing.md](circle_packing.md)

Watch for three recurring Rust patterns:

1. Public APIs expose small typed handles instead of internal storage.
2. Builders take `&mut FactorGraph` while they add variables, edges, and
   factors.
3. Minimizers are boxed closures that read and write exchange slices.
