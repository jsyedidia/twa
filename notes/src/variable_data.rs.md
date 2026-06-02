# src/variable_data.rs

## Role In The System

`VariableData` is the internal per-variable storage for the TWA. It is not part
of the public API; users hold `VariableNode` handles, while `FactorGraph` owns
the actual `VariableData` objects.

A variable tracks its current consensus value, its weight (how certain it is),
the set of all connected edges, and a lazily-maintained cache of which edges
are currently enabled.

Related concept notes:

- [notes/concepts/message_passing.md](../concepts/message_passing.md)
- [notes/concepts/weights.md](../concepts/weights.md)

## State At A Glance

```rust
pub(crate) struct VariableData {
    initial_value: WeightedValue,
    value: f64,
    weight: MessageWeight,
    edges: Vec<GraphEdge>,
    enabled_edges: Vec<GraphEdge>,
    enabled_edges_need_update: bool,
}
```

- `initial_value` — the weighted value used at construction and restored on
  reset.
- `value` — the current scalar consensus value.
- `weight` — the current weight (reflects the variable's certainty level).
- `edges` — all edges connected to this variable, in insertion order.
- `enabled_edges` — the subset of `edges` that are currently active. This is
  a cache rebuilt lazily when `enabled_edges_need_update` is true.
- `enabled_edges_need_update` — dirty flag for the enabled-edges cache.

## Code Walkthrough

### Imports

```rust
use crate::edge_data::EdgeData;
use crate::graph_edge::GraphEdge;
use crate::weighted_value::{MessageWeight, WeightedValue};
```

The variable needs `EdgeData` to check which edges are enabled, `GraphEdge`
for the handle type, and the weight/value types.

### Struct Definition

```rust
#[derive(Debug, Clone)]
pub(crate) struct VariableData {
    initial_value: WeightedValue,
    value: f64,
    weight: MessageWeight,
    edges: Vec<GraphEdge>,
    enabled_edges: Vec<GraphEdge>,
    enabled_edges_need_update: bool,
}
```

`pub(crate)` visibility — only accessible within the crate. `Clone` is derived
so the graph can snapshot state. All fields are private.

### `new`

```rust
impl VariableData {
    pub(crate) fn new(initial_value: WeightedValue) -> Self {
        Self {
            initial_value,
            value: initial_value.value,
            weight: initial_value.weight,
            edges: Vec::new(),
            enabled_edges: Vec::new(),
            enabled_edges_need_update: false,
        }
    }
```

Creates variable data with value and weight from the initial weighted value.
Both edge vectors start empty. The dirty flag is `false` because there are no
edges to rebuild.

### `reset`

```rust
    pub(crate) fn reset(&mut self) {
        self.enabled_edges.clone_from(&self.edges);
        self.enabled_edges_need_update = false;
        self.value = self.initial_value.value;
        self.weight = self.initial_value.weight;
    }
```

Restores the variable to its initial state. All edges are assumed enabled after
reset (the graph will re-enable edges in `EdgeData` as well). Uses
`clone_from` to reuse the existing `Vec` allocation when possible.

### `add_edge`

```rust
    pub(crate) fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
        self.enabled_edges.push(edge);
        self.enabled_edges_need_update = true;
    }
```

Registers a new edge. It goes into both vectors immediately, and the dirty
flag is set because the ordering in `enabled_edges` may not match what a
rebuild would produce (the edge was appended at the end).

### `reenable_edge`

```rust
    pub(crate) fn reenable_edge(&mut self, edge: GraphEdge) {
        assert!(
            self.edges.contains(&edge),
            "VariableData cannot reenable an edge it does not own"
        );

        if self.enabled_edges_need_update {
            return;
        }

        let already_enabled = self.enabled_edges.contains(&edge);
        if !already_enabled {
            self.enabled_edges.push(edge);
        }
    }
```

Re-adds an edge to the enabled set after it was re-enabled in the graph.
Panics if the edge is not owned by this variable (a programming error).
If the cache is already marked dirty, there's no point modifying it — it will
be rebuilt on next access. Avoids duplicates by checking membership first.

### `force_enabled_edges_update`

```rust
    pub(crate) fn force_enabled_edges_update(&mut self) {
        self.enabled_edges_need_update = true;
    }
```

Marks the cache as stale. The next call to `enabled_edges()` will rebuild it
by scanning the actual `EdgeData` states.

### `update_result`

```rust
    pub(crate) fn update_result(&mut self, result: WeightedValue) {
        self.value = result.value;
        self.weight = result.weight;
    }
```

Sets both value and weight from a consensus result. Called during the variable
pass after computing the weighted average of incoming messages.

### `initial_value`

```rust
    pub(crate) fn initial_value(&self) -> WeightedValue {
        self.initial_value
    }
```

Returns the initial weighted value. Used during reset and when the graph needs
to re-initialize edges.

### `value`

```rust
    pub(crate) fn value(&self) -> f64 {
        self.value
    }
```

Returns the current scalar consensus value.

### `weight`

```rust
    pub(crate) fn weight(&self) -> MessageWeight {
        self.weight
    }
```

Returns the current weight (certainty level).

### `edges`

```rust
    pub(crate) fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }
```

Returns all edges, enabled and disabled.

### `enabled_edges`

```rust
    pub(crate) fn enabled_edges(&mut self, edge_data: &[EdgeData]) -> &[GraphEdge] {
        if self.enabled_edges_need_update {
            self.rebuild_enabled_edges(edge_data);
        }

        assert!(
            !self.enabled_edges.is_empty(),
            "VariableData has no enabled edges"
        );

        &self.enabled_edges
    }
```

Returns the enabled edges, rebuilding the cache first if the dirty flag is set.
Panics if no enabled edges exist — this indicates a malformed graph (a variable
with all edges disabled should have been handled differently).

Takes `&mut self` because it may rebuild the internal cache. Takes `edge_data`
as a slice to check which edges are actually enabled at the data level.

### `rebuild_enabled_edges`

```rust
    fn rebuild_enabled_edges(&mut self, edge_data: &[EdgeData]) {
        self.enabled_edges.clear();

        for &edge in &self.edges {
            assert!(
                edge.is_valid() && edge.index() < edge_data.len(),
                "VariableData references an invalid edge"
            );
            if edge_data[edge.index()].is_enabled() {
                self.enabled_edges.push(edge);
            }
        }

        self.enabled_edges_need_update = false;
    }
```

Walks the full edge list in insertion order, keeps only those whose `EdgeData`
is enabled. Panics on invalid or out-of-bounds edge indices (a programming
error). Clears the dirty flag after rebuild.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable_node::VariableNode;

    fn edge_indexes(edges: &[GraphEdge]) -> Vec<usize> {
        edges.iter().map(|e| e.index()).collect()
    }

    fn make_edge_data(count: usize) -> Vec<EdgeData> {
        (0..count)
            .map(|_| {
                EdgeData::new(
                    VariableNode::new(0),
                    WeightedValue::new(0.0, MessageWeight::Standard),
                )
            })
            .collect()
    }
```

Test helpers: `edge_indexes` extracts raw indices for easy comparison;
`make_edge_data` creates dummy enabled edges for testing the cache logic.

```rust
    #[test]
    fn initial_value_and_empty_enabled_edges() {
        let variable = VariableData::new(WeightedValue::new(2.5, MessageWeight::Infinite));

        assert_eq!(variable.initial_value().value, 2.5);
        assert_eq!(variable.initial_value().weight, MessageWeight::Infinite);
        assert_eq!(variable.value(), 2.5);
        assert_eq!(variable.weight(), MessageWeight::Infinite);
        assert!(variable.edges().is_empty());
    }
```

Verifies initial state: value, weight, and empty edge set.

```rust
    #[test]
    #[should_panic(expected = "no enabled edges")]
    fn empty_enabled_edges_panics() {
        let mut variable = VariableData::new(WeightedValue::new(2.5, MessageWeight::Infinite));
        let edge_data: Vec<EdgeData> = Vec::new();
        variable.enabled_edges(&edge_data);
    }
```

Confirms that calling `enabled_edges()` with no edges panics.

```rust
    #[test]
    fn enabled_edges_are_filtered_lazily() {
        let mut edge_data = make_edge_data(3);
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(0));
        variable.add_edge(GraphEdge::new(1));
        variable.add_edge(GraphEdge::new(2));

        assert_eq!(
            edge_indexes(variable.enabled_edges(&edge_data)),
            vec![0, 1, 2]
        );

        // Disable edge 1 in the data, but cache is still clean.
        edge_data[1].disable();
        assert_eq!(
            edge_indexes(variable.enabled_edges(&edge_data)),
            vec![0, 1, 2]
        );

        // Force rebuild — now edge 1 is excluded.
        variable.force_enabled_edges_update();
        assert_eq!(edge_indexes(variable.enabled_edges(&edge_data)), vec![0, 2]);

        // Second call without force uses cached result.
        assert_eq!(edge_indexes(variable.enabled_edges(&edge_data)), vec![0, 2]);
    }
```

Tests that disabling an edge in `EdgeData` doesn't immediately affect the
variable's cached enabled list — only after `force_enabled_edges_update()`.

```rust
    #[test]
    fn reenabled_edges_do_not_duplicate() {
        let mut edge_data = make_edge_data(3);
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(0));
        variable.add_edge(GraphEdge::new(1));
        variable.add_edge(GraphEdge::new(2));

        edge_data[1].disable();
        variable.force_enabled_edges_update();
        assert_eq!(edge_indexes(variable.enabled_edges(&edge_data)), vec![0, 2]);

        // Re-enable edge 1 and reenable it in variable (twice — no duplicate).
        edge_data[1].reset(WeightedValue::new(4.0, MessageWeight::Standard));
        variable.reenable_edge(GraphEdge::new(1));
        variable.reenable_edge(GraphEdge::new(1));
        assert_eq!(
            edge_indexes(variable.enabled_edges(&edge_data)),
            vec![0, 2, 1]
        );

        // Force rebuild sorts by original order.
        variable.force_enabled_edges_update();
        assert_eq!(
            edge_indexes(variable.enabled_edges(&edge_data)),
            vec![0, 1, 2]
        );
    }
```

Tests that `reenable_edge` avoids duplicates and that a forced rebuild restores
the original insertion order.

```rust
    #[test]
    fn reset_restores_initial_state() {
        let mut edge_data = make_edge_data(2);
        let mut variable = VariableData::new(WeightedValue::new(3.0, MessageWeight::Zero));
        variable.add_edge(GraphEdge::new(0));
        variable.add_edge(GraphEdge::new(1));

        edge_data[1].disable();
        variable.force_enabled_edges_update();
        variable.update_result(WeightedValue::new(10.0, MessageWeight::Infinite));

        assert_eq!(variable.value(), 10.0);
        assert_eq!(variable.weight(), MessageWeight::Infinite);
        assert_eq!(edge_indexes(variable.enabled_edges(&edge_data)), vec![0]);

        variable.reset();
        assert_eq!(variable.value(), 3.0);
        assert_eq!(variable.weight(), MessageWeight::Zero);
        // After reset, all edges are in the enabled set (regardless of edge_data state).
        assert_eq!(edge_indexes(variable.enabled_edges(&edge_data)), vec![0, 1]);
    }
```

Tests that `reset()` restores value, weight, and assumes all edges enabled.

```rust
    #[test]
    #[should_panic(expected = "no enabled edges")]
    fn no_enabled_edges_panics() {
        let mut edge_data = make_edge_data(2);
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(0));
        variable.add_edge(GraphEdge::new(1));

        edge_data[0].disable();
        edge_data[1].disable();
        variable.force_enabled_edges_update();
        variable.enabled_edges(&edge_data);
    }
```

Tests the panic when all edges are disabled.

```rust
    #[test]
    #[should_panic(expected = "invalid edge")]
    fn invalid_edge_reference_panics() {
        let edge_data = make_edge_data(1);
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(2)); // index 2 is out of bounds
        variable.force_enabled_edges_update();
        variable.enabled_edges(&edge_data);
    }
```

Tests the panic on out-of-bounds edge indices during rebuild.

```rust
    #[test]
    #[should_panic(expected = "does not own")]
    fn reenable_unknown_edge_panics() {
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(0));
        variable.reenable_edge(GraphEdge::new(3));
    }
}
```

Tests the panic when trying to reenable an edge not in this variable's edge
list.
