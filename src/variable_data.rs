use crate::edge_data::EdgeData;
use crate::graph_edge::GraphEdge;
use crate::weighted_value::{MessageWeight, WeightedValue};

/// Internal per-variable storage for the TWA.
///
/// Tracks the variable's current value, weight, initial value, the set of all
/// connected edges, and a lazily-maintained cache of enabled edges.
#[derive(Debug, Clone)]
pub(crate) struct VariableData {
    initial_value: WeightedValue,
    value: f64,
    weight: MessageWeight,
    edges: Vec<GraphEdge>,
    enabled_edges: Vec<GraphEdge>,
    enabled_edges_need_update: bool,
}

impl VariableData {
    /// Creates variable data with the given initial weighted value.
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

    /// Resets the variable to its initial state: value, weight, and enabled
    /// edges are restored, and the enabled-edges cache is marked clean (all
    /// edges are assumed enabled after reset).
    pub(crate) fn reset(&mut self) {
        self.enabled_edges.clone_from(&self.edges);
        self.enabled_edges_need_update = false;
        self.value = self.initial_value.value;
        self.weight = self.initial_value.weight;
    }

    /// Registers a new edge with this variable.
    pub(crate) fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
        self.enabled_edges.push(edge);
        self.enabled_edges_need_update = true;
    }

    /// Re-adds an edge to the enabled set (e.g., after it was re-enabled).
    ///
    /// Panics if the edge is not owned by this variable.
    /// Does nothing if the enabled-edges cache is already marked for rebuild.
    /// Avoids duplicates if the edge is already in the enabled set.
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

    /// Marks the enabled-edges cache as stale, forcing a rebuild on next access.
    pub(crate) fn force_enabled_edges_update(&mut self) {
        self.enabled_edges_need_update = true;
    }

    /// Updates both value and weight from a consensus result.
    pub(crate) fn update_result(&mut self, result: WeightedValue) {
        self.value = result.value;
        self.weight = result.weight;
    }

    /// Returns the initial weighted value this variable was created with.
    pub(crate) fn initial_value(&self) -> WeightedValue {
        self.initial_value
    }

    /// Returns the current scalar value.
    pub(crate) fn value(&self) -> f64 {
        self.value
    }

    /// Returns the current weight.
    pub(crate) fn weight(&self) -> MessageWeight {
        self.weight
    }

    /// Returns all edges (enabled and disabled) connected to this variable.
    pub(crate) fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    /// Returns the enabled edges, rebuilding the cache if necessary.
    ///
    /// Panics if no enabled edges exist (the graph is malformed or all edges
    /// were disabled without removing the variable).
    /// Panics if an edge index is out of bounds in `edge_data`.
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
}

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

    #[test]
    fn initial_value_and_empty_enabled_edges() {
        let variable = VariableData::new(WeightedValue::new(2.5, MessageWeight::Infinite));

        assert_eq!(variable.initial_value().value, 2.5);
        assert_eq!(variable.initial_value().weight, MessageWeight::Infinite);
        assert_eq!(variable.value(), 2.5);
        assert_eq!(variable.weight(), MessageWeight::Infinite);
        assert!(variable.edges().is_empty());
    }

    #[test]
    #[should_panic(expected = "no enabled edges")]
    fn empty_enabled_edges_panics() {
        let mut variable = VariableData::new(WeightedValue::new(2.5, MessageWeight::Infinite));
        let edge_data: Vec<EdgeData> = Vec::new();
        variable.enabled_edges(&edge_data);
    }

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

    #[test]
    #[should_panic(expected = "invalid edge")]
    fn invalid_edge_reference_panics() {
        let edge_data = make_edge_data(1);
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(2)); // index 2 is out of bounds
        variable.force_enabled_edges_update();
        variable.enabled_edges(&edge_data);
    }

    #[test]
    #[should_panic(expected = "does not own")]
    fn reenable_unknown_edge_panics() {
        let mut variable = VariableData::new(WeightedValue::new(1.0, MessageWeight::Standard));
        variable.add_edge(GraphEdge::new(0));
        variable.reenable_edge(GraphEdge::new(3));
    }
}
