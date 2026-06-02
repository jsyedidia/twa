# `src/factor_graph.rs`

The central public API of the TWA crate. `FactorGraph` owns all variables,
edges, and factors, and drives the iterative message-passing algorithm to
convergence.

---

## Imports

```rust
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};

use crate::edge_data::EdgeData;
use crate::factor_data::FactorData;
use crate::factor_node::FactorNode;
use crate::graph_edge::GraphEdge;
use crate::variable_data::VariableData;
use crate::variable_node::VariableNode;
use crate::weighted_value::{MessageWeight, MinimizationFn, WeightedValue};

type Callback = Box<dyn FnMut()>;
type GraphCallback = Box<dyn FnMut(&mut FactorGraph)>;
```

`StdRng` provides the seeded random number generator. All internal data types
are imported from their respective modules. The callback aliases keep the
struct field types readable while preserving the public callback method
signatures.

---

## Struct Definition

```rust
pub struct FactorGraph {
    learning_rate: f64,
    convergence_delta: f64,
    rng: Box<dyn RngCore>,
    iterations: usize,
    converged: bool,
    variables: Vec<VariableData>,
    edges: Vec<EdgeData>,
    factors: Vec<FactorData>,
    iteration_callbacks: Vec<Callback>,
    reinitialize_callbacks: Vec<Callback>,
    iteration_graph_callbacks: Vec<GraphCallback>,
    reinitialize_graph_callbacks: Vec<GraphCallback>,
}
```

- `learning_rate`: the alpha step size for ADMM disagreement updates.
- `convergence_delta`: threshold for message-difference convergence check.
- `rng`: boxed RNG passed to minimizers for tie-breaking.
- `iterations`: count since last reinitialize.
- `converged`: cached convergence state (sticky once true until reinitialize
  or factor enable/disable).
- `variables`, `edges`, `factors`: arena-style storage indexed by the typed
  handle newtypes.
- `iteration_callbacks`, `reinitialize_callbacks`: user-registered hooks that
  observe iteration/reinitialization events.
- `iteration_graph_callbacks`, `reinitialize_graph_callbacks`: hooks that can
  mutate graph state after an iteration or reinitialization.

The RNG is boxed as `dyn RngCore` to avoid a generic type parameter on
`FactorGraph`, keeping the public API simple.

---

## Constructor

```rust
pub fn new(learning_rate: f64, convergence_delta: f64, random_seed: u64) -> Self {
    Self {
        learning_rate,
        convergence_delta,
        rng: Box::new(StdRng::seed_from_u64(random_seed)),
        iterations: 0,
        converged: false,
        variables: Vec::new(),
        edges: Vec::new(),
        factors: Vec::new(),
        iteration_callbacks: Vec::new(),
        reinitialize_callbacks: Vec::new(),
        iteration_graph_callbacks: Vec::new(),
        reinitialize_graph_callbacks: Vec::new(),
    }
}
```

All collections start empty. The graph is built up by successive calls to
`create_variable`, `create_edge`, and `create_factor`.

---

## Parameter Accessors

```rust
    /// Returns the current learning rate.
    pub fn learning_rate(&self) -> f64 {
        self.learning_rate
    }

    /// Sets the learning rate.
    pub fn set_learning_rate(&mut self, learning_rate: f64) {
        self.learning_rate = learning_rate;
    }

    /// Returns the convergence delta threshold.
    pub fn convergence_delta(&self) -> f64 {
        self.convergence_delta
    }

    /// Sets the convergence delta threshold.
    pub fn set_convergence_delta(&mut self, convergence_delta: f64) {
        self.convergence_delta = convergence_delta;
    }

    /// Reseeds the random number generator.
    pub fn set_random_seed(&mut self, random_seed: u64) {
        self.rng = Box::new(StdRng::seed_from_u64(random_seed));
    }
```

Parameters can be adjusted between iterations or reinitializations.
`set_random_seed` replaces the entire RNG to guarantee deterministic replay.

---

## State Queries

```rust
    /// Returns the number of iterations performed since the last reinitialize.
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Returns whether the graph has converged.
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// Returns the total number of variables.
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    /// Returns the total number of edges.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Returns the number of currently enabled edges.
    pub fn num_enabled_edges(&self) -> usize {
        self.edges.iter().filter(|e| e.is_enabled()).count()
    }

    /// Returns the total number of factors.
    pub fn num_factors(&self) -> usize {
        self.factors.len()
    }

    /// Returns the number of currently enabled factors.
    pub fn num_enabled_factors(&self) -> usize {
        self.factors.iter().filter(|f| f.is_enabled()).count()
    }
```

Enabled counts are computed by linear scan. This is acceptable because these
are diagnostic queries, not called on the hot iteration path.

---

## Graph Construction

### create_variable

```rust
pub fn create_variable(
    &mut self,
    initial_value: f64,
    initial_weight: MessageWeight,
) -> VariableNode {
    let node = VariableNode::new(self.variables.len());
    self.variables.push(VariableData::new(WeightedValue::new(
        initial_value,
        initial_weight,
    )));
    node
}
```

Appends a new `VariableData` and returns the index-based handle.

### create_edge

```rust
pub fn create_edge(&mut self, variable: VariableNode) -> GraphEdge {
    self.validate_variable(variable);
    let edge = GraphEdge::new(self.edges.len());
    let initial = self.variables[variable.index()].initial_value();
    self.edges.push(EdgeData::new(variable, initial));
    self.variables[variable.index()].add_edge(edge);
    edge
}
```

Creates an edge, initializes it with the variable's initial value, and
registers the edge with the variable's edge list. Edges are created *before*
the factor that uses them, so edges always know their variable but not their
factor.

### create_factor

```rust
pub fn create_factor(
    &mut self,
    edges: &[GraphEdge],
    minimization_function: MinimizationFn,
) -> FactorNode {
    for &edge in edges {
        self.validate_edge(edge);
    }

    let node = FactorNode::new(self.factors.len());
    self.factors
        .push(FactorData::new(edges, minimization_function));
    node
}
```

Validates all edge handles, then creates the `FactorData`. The factor stores
its own copy of the edge list (inside its exchange buffer).

---

## Value and Weight Accessors

```rust
pub fn value(&self, variable: VariableNode) -> f64 {
    self.validate_variable(variable);
    self.variables[variable.index()].value()
}

pub fn weight(&self, variable: VariableNode) -> MessageWeight {
    self.validate_variable(variable);
    self.variables[variable.index()].weight()
}
```

Read the current consensus value/weight for a variable.

---

## Factor Enable / Disable

```rust
pub fn is_factor_enabled(&self, factor: FactorNode) -> bool {
    self.validate_factor(factor);
    self.factors[factor.index()].is_enabled()
}

pub fn set_factor_enabled(&mut self, factor: FactorNode, enabled: bool) {
    self.validate_factor(factor);
    if enabled {
        self.enable_factor(factor.index());
    } else {
        self.disable_factor(factor.index());
    }
}
```

Delegates to `enable_factor` or `disable_factor` which handle edge state
transitions.

---

## Iterate — The Core Algorithm

```rust
pub fn iterate(&mut self) -> bool {
    if self.converged {
        return true;
    }

    // Factor pass
    for factor in &mut self.factors {
        factor.minimize(&mut self.edges, self.rng.as_mut());
    }

    // Variable pass
    for variable in &mut self.variables {
        let (result, has_lone_standard) = {
            let enabled_edges = variable.enabled_edges(&self.edges);
            (
                Self::enforce_variable_equality(&self.edges, enabled_edges),
                Self::has_lone_standard_message_to_variable(&self.edges, enabled_edges),
            )
        };
        variable.update_result(result);

        for &edge in variable.enabled_edges(&self.edges) {
            let reset_disagreement = has_lone_standard
                && self.edges[edge.index()]
                    .weighted_message_to_variable()
                    .weight
                    == MessageWeight::Standard;
            self.edges[edge.index()].set_result_from_variable(
                result,
                self.learning_rate,
                reset_disagreement,
            );
        }
    }

    self.iterations += 1;
    self.converged = self.all_enabled_edges_converged();

    for callback in &mut self.iteration_callbacks {
        callback();
    }
    self.run_iteration_graph_callbacks();

    self.converged
}
```

One full iteration:

1. **Factor pass**: Each factor reads its incoming messages, calls its
   minimizer, writes results back (with certainty preservation).
2. **Variable pass**: For each variable, compute the weighted consensus
   (`enforce_variable_equality`), determine if there's a lone standard
   message, update the variable's value/weight, then update each connected
   enabled edge via `set_result_from_variable`.
3. **Convergence check**: If all enabled edges have converged (message
   difference ≤ delta), set `converged = true`.
4. **Callbacks**: Notify listeners. Graph-aware callbacks run after ordinary
   callbacks so they can respond to the final state of the iteration.

The enabled-edge slice is borrowed in two short scopes. The first borrow
computes the consensus and lone-standard flag, then ends before the variable is
updated. The second borrow reuses the same cached slice while writing results
to the edges, so the hot variable pass does not allocate.

---

## iterate_until_converged

```rust
pub fn iterate_until_converged(&mut self, max_iterations: usize) -> bool {
    for _ in 0..max_iterations {
        if self.iterate() {
            return true;
        }
    }
    self.converged
}
```

Convenience wrapper that loops up to a maximum number of iterations.

---

## Reinitialize

```rust
pub fn reinitialize(&mut self) {
    for variable in &mut self.variables {
        let initial = variable.initial_value();
        for &edge in variable.edges() {
            self.edges[edge.index()].reset(initial);
        }
        variable.reset();
    }

    for factor in &mut self.factors {
        factor.reset();
    }

    self.iterations = 0;
    self.converged = false;

    for callback in &mut self.reinitialize_callbacks {
        callback();
    }
    self.run_reinitialize_graph_callbacks();
}
```

Resets all state to the initial configuration: each edge is reset with its
variable's initial value, each variable restores its initial value/weight and
edge caches, and each factor is re-enabled. Graph-aware reinitialize callbacks
then get a chance to adjust enabled state.

---

## Callbacks

```rust
pub fn add_iteration_callback(&mut self, callback: Box<dyn FnMut()>) {
    self.iteration_callbacks.push(callback);
}

pub fn add_reinitialize_callback(&mut self, callback: Box<dyn FnMut()>) {
    self.reinitialize_callbacks.push(callback);
}

pub fn add_iteration_graph_callback(&mut self, callback: Box<dyn FnMut(&mut FactorGraph)>) {
    self.iteration_graph_callbacks.push(callback);
}

pub fn add_reinitialize_graph_callback(&mut self, callback: Box<dyn FnMut(&mut FactorGraph)>) {
    self.reinitialize_graph_callbacks.push(callback);
}
```

Callbacks use `Box<dyn FnMut()>` rather than generic parameters to keep the
API simple and avoid monomorphization. The graph-aware variants receive
`&mut FactorGraph`, which lets problem builders such as circle packing adjust
factor enablement in response to current graph values.

### run_*_graph_callbacks

```rust
fn run_iteration_graph_callbacks(&mut self) {
    let mut callbacks = std::mem::take(&mut self.iteration_graph_callbacks);
    for callback in &mut callbacks {
        callback(self);
    }
    self.iteration_graph_callbacks = callbacks;
}

fn run_reinitialize_graph_callbacks(&mut self) {
    let mut callbacks = std::mem::take(&mut self.reinitialize_graph_callbacks);
    for callback in &mut callbacks {
        callback(self);
    }
    self.reinitialize_graph_callbacks = callbacks;
}
```

The callback vector is temporarily moved out of `self`. This avoids borrowing
the callback list and the whole graph mutably at the same time.

---

## Private Helpers

### Validation

```rust
fn validate_variable(&self, variable: VariableNode) {
    assert!(
        variable.is_valid() && variable.index() < self.variables.len(),
        "FactorGraph references an invalid variable"
    );
}

fn validate_edge(&self, edge: GraphEdge) {
    assert!(
        edge.is_valid() && edge.index() < self.edges.len(),
        "FactorGraph references an invalid edge"
    );
}

fn validate_factor(&self, factor: FactorNode) {
    assert!(
        factor.is_valid() && factor.index() < self.factors.len(),
        "FactorGraph references an invalid factor"
    );
}
```

All public methods that accept handles call the appropriate validator first.
These panic on invalid input rather than returning `Result`, following Rust's
convention for programmer errors (invalid handles cannot be constructed
accidentally).

### enable_factor / disable_factor

```rust
fn enable_factor(&mut self, factor_index: usize) {
    if !self.factors[factor_index].enable() {
        return;
    }

    for exchange in self.factors[factor_index].exchanges().to_vec() {
        let edge = exchange.edge();
        let variable = self.edges[edge.index()].variable();
        let value = self.variables[variable.index()].value();
        self.edges[edge.index()].reset(WeightedValue::new(value, MessageWeight::Standard));
        self.variables[variable.index()].reenable_edge(edge);
    }

    self.converged = false;
}

fn disable_factor(&mut self, factor_index: usize) {
    if !self.factors[factor_index].disable() {
        return;
    }

    for exchange in self.factors[factor_index].exchanges().to_vec() {
        let edge = exchange.edge();
        let variable = self.edges[edge.index()].variable();
        self.edges[edge.index()].disable();
        self.variables[variable.index()].force_enabled_edges_update();
    }

    self.converged = false;
}
```

When enabling: each edge is reset with the variable's *current* value (not
initial value), and the variable's enabled-edge cache is updated. When
disabling: each edge is disabled and the variable's cache is marked stale.
Both clear `converged` to force future iterations.

### enforce_variable_equality

```rust
fn enforce_variable_equality(edges: &[EdgeData], enabled_edges: &[GraphEdge]) -> WeightedValue {
    let mut all_sum = 0.0;
    let mut standard_sum = 0.0;
    let mut standard_count: usize = 0;

    for &edge in enabled_edges {
        let message = edges[edge.index()].weighted_message_to_variable();
        if message.weight == MessageWeight::Infinite {
            return message;
        }
        if message.weight == MessageWeight::Standard {
            standard_sum += message.value;
            standard_count += 1;
        }
        all_sum += message.value;
    }

    if standard_count > 0 {
        WeightedValue::new(
            standard_sum / standard_count as f64,
            MessageWeight::Standard,
        )
    } else {
        WeightedValue::new(all_sum / enabled_edges.len() as f64, MessageWeight::Zero)
    }
}
```

The consensus rule for a variable with multiple incoming messages:
1. If *any* message has infinite weight, that message wins immediately
   (certainty dominates).
2. Otherwise, if there are any standard-weight messages, average them.
3. Otherwise (all zero weight), average everything with zero weight.

### has_lone_standard_message_to_variable

```rust
fn has_lone_standard_message_to_variable(
    edges: &[EdgeData],
    enabled_edges: &[GraphEdge],
) -> bool {
    let mut standard_count: usize = 0;
    for &edge in enabled_edges {
        let weight = edges[edge.index()].weighted_message_to_variable().weight;
        if weight == MessageWeight::Infinite {
            return false;
        }
        if weight == MessageWeight::Standard {
            standard_count += 1;
        }
    }
    standard_count == 1
}
```

Returns `true` when exactly one edge carries a standard-weight message and
none carry infinite weight. In this case the lone standard edge has its
disagreement reset during the variable pass, which accelerates convergence
when only one factor has a meaningful opinion.

### all_enabled_edges_converged

```rust
fn all_enabled_edges_converged(&self) -> bool {
    for edge in &self.edges {
        if !edge.is_enabled() {
            continue;
        }
        match edge.message_difference() {
            Some(diff) if diff <= self.convergence_delta => {}
            _ => return false,
        }
    }
    true
}
```

An edge is converged if its message difference exists and is at or below the
delta threshold. Edges that have not yet had two factor passes (difference is
`None`) are not converged.

---

## Default Implementation

```rust
impl Default for FactorGraph {
    fn default() -> Self {
        Self::new(1.0, 1e-5, 0)
    }
}
```

The default graph uses learning rate 1.0, convergence delta 1e-5, and seed 0.

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-10
    }
```

Tests use a small tolerance because the graph iterates on floating-point
messages.

```rust
    /// Helper: creates a factor that always outputs the given known value
    /// with infinite weight on every exchange.
    fn known_value_minimizer(value: f64) -> MinimizationFn {
        Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
            for exchange in exchanges.iter_mut() {
                exchange.set(WeightedValue::new(value, MessageWeight::Infinite));
            }
        })
    }
```

This helper stands in for the future `known_value` minimizer factory.

```rust
    /// Helper: creates a factor that clamps each exchange's value to the given
    /// range with standard weight.
    fn in_range_minimizer(low: f64, high: f64) -> MinimizationFn {
        Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
            for exchange in exchanges.iter_mut() {
                let clamped = exchange.get().value.clamp(low, high);
                exchange.set(WeightedValue::new(clamped, MessageWeight::Standard));
            }
        })
    }
```

This helper stands in for the future `in_range` minimizer factory.

```rust
    #[test]
    fn known_value_converges() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(7.0));

        assert!(graph.iterate_until_converged(100));
        assert!(graph.converged());
        assert!(nearly_equal(graph.value(variable), 7.0));
        assert_eq!(graph.weight(variable), MessageWeight::Infinite);
        assert!(graph.iterations() >= 2);
    }
```

A one-variable graph with a certain factor converges to that certain value and
marks the variable as infinite weight.

```rust
    #[test]
    fn range_clamps_below_above_and_inside() {
        // Below range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(-2.0, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 0.0));
        }

        // Above range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(5.0, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 1.0));
        }

        // Inside range
        {
            let mut graph = FactorGraph::default();
            let variable = graph.create_variable(0.4, MessageWeight::Standard);
            let edge = graph.create_edge(variable);
            graph.create_factor(&[edge], in_range_minimizer(0.0, 1.0));

            assert!(graph.iterate_until_converged(100));
            assert!(nearly_equal(graph.value(variable), 0.4));
        }
    }
```

This protects the basic standard-weight consensus behavior for clamping below,
above, and inside the allowed interval.

```rust
    #[test]
    fn reinitialize_resets_graph_state() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(2.0, MessageWeight::Standard);
        let edge = graph.create_edge(variable);
        graph.create_factor(&[edge], known_value_minimizer(4.0));

        use std::cell::Cell;
        use std::rc::Rc;

        let iteration_count = Rc::new(Cell::new(0));
        let reinitialize_count = Rc::new(Cell::new(0));

        let ic = Rc::clone(&iteration_count);
        graph.add_iteration_callback(Box::new(move || {
            ic.set(ic.get() + 1);
        }));

        let rc = Rc::clone(&reinitialize_count);
        graph.add_reinitialize_callback(Box::new(move || {
            rc.set(rc.get() + 1);
        }));

        assert!(graph.iterate_until_converged(100));
        assert!(graph.converged());
        assert!(graph.iterations() > 0);
        assert_eq!(iteration_count.get(), graph.iterations());
        assert!(nearly_equal(graph.value(variable), 4.0));

        graph.reinitialize();
        assert!(!graph.converged());
        assert_eq!(graph.iterations(), 0);
        assert_eq!(reinitialize_count.get(), 1);
        assert!(nearly_equal(graph.value(variable), 2.0));

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 4.0));
    }
```

Reinitialization restores initial values, clears convergence state, resets the
iteration count, and fires the reinitialize callback.

```rust
    #[test]
    fn factor_enable_disable() {
        let mut graph = FactorGraph::default();
        let variable = graph.create_variable(1.0, MessageWeight::Standard);

        let edge1 = graph.create_edge(variable);
        graph.create_factor(&[edge1], in_range_minimizer(0.0, 20.0));

        let edge2 = graph.create_edge(variable);
        let optional_factor = graph.create_factor(&[edge2], known_value_minimizer(10.0));

        assert_eq!(graph.num_variables(), 1);
        assert_eq!(graph.num_edges(), 2);
        assert_eq!(graph.num_enabled_edges(), 2);
        assert_eq!(graph.num_factors(), 2);
        assert_eq!(graph.num_enabled_factors(), 2);

        graph.set_factor_enabled(optional_factor, false);
        assert!(!graph.is_factor_enabled(optional_factor));
        assert_eq!(graph.num_enabled_factors(), 1);
        assert_eq!(graph.num_enabled_edges(), 1);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 1.0));

        graph.set_factor_enabled(optional_factor, true);
        assert!(graph.is_factor_enabled(optional_factor));
        assert_eq!(graph.num_enabled_factors(), 2);
        assert_eq!(graph.num_enabled_edges(), 2);

        assert!(graph.iterate_until_converged(100));
        assert!(nearly_equal(graph.value(variable), 10.0));
    }
```

Disabling a factor removes its edge from iteration; re-enabling restores the
edge and lets the factor influence consensus again.

```rust
    #[test]
    fn lone_standard_message_resets_disagreement() {
        let mut graph = FactorGraph::new(0.5, 1e-5, 0);
        let variable = graph.create_variable(0.0, MessageWeight::Standard);
        let first_edge = graph.create_edge(variable);
        let second_edge = graph.create_edge(variable);

        graph.create_factor(
            &[first_edge],
            Box::new(|exchanges: &mut [_], _: &mut dyn RngCore| {
                exchanges[0].set(WeightedValue::new(10.0, MessageWeight::Standard));
            }),
        );

        use std::cell::Cell;
        use std::rc::Rc;

        let calls = Rc::new(Cell::new(0));
        let calls_clone = Rc::clone(&calls);
        graph.create_factor(
            &[second_edge],
            Box::new(move |exchanges: &mut [_], _: &mut dyn RngCore| {
                if calls_clone.get() == 0 {
                    exchanges[0].set(WeightedValue::new(0.0, MessageWeight::Standard));
                } else {
                    exchanges[0].set(WeightedValue::new(0.0, MessageWeight::Zero));
                }
                calls_clone.set(calls_clone.get() + 1);
            }),
        );

        // After iteration 1: average of 10 and 0 = 5
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 5.0));

        // After iteration 2: second factor emits zero weight → lone standard.
        // Without reset, disagreement would slow convergence.
        // With reset on the lone standard edge, value jumps to 12.5
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 12.5));

        // After iteration 3: converging toward 10.0
        assert!(!graph.iterate());
        assert!(nearly_equal(graph.value(variable), 10.0));
    }
```

This test covers the special lone-standard reset path that affects the dual
disagreement update on edges.

```rust
    #[test]
    #[should_panic(expected = "invalid variable")]
    fn invalid_variable_panics() {
        let graph = FactorGraph::default();
        graph.value(VariableNode::new(0));
    }

    #[test]
    #[should_panic(expected = "invalid edge")]
    fn invalid_edge_panics() {
        let mut graph = FactorGraph::default();
        graph.create_variable(0.0, MessageWeight::Standard);
        graph.create_factor(&[GraphEdge::new(3)], Box::new(|_, _| {}));
    }
}
```

Invalid handles are programmer errors and panic at the validation boundary.
