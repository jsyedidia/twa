use crate::edge_data::EdgeData;
use crate::graph_edge::GraphEdge;
use crate::weighted_value::{MessageWeight, MinimizationFn, WeightedValueExchange};
use rand::RngCore;

/// Internal per-factor storage for the TWA.
///
/// A factor owns an exchange buffer (one entry per connected edge), tracks
/// which incoming messages had infinite weight (for certainty preservation),
/// and holds the minimization function that computes factor-side preferences.
pub(crate) struct FactorData {
    minimization_function: MinimizationFn,
    exchanges: Vec<WeightedValueExchange>,
    incoming_infinite_weights: Vec<bool>,
    enabled: bool,
}

impl std::fmt::Debug for FactorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactorData")
            .field("exchanges", &self.exchanges)
            .field("incoming_infinite_weights", &self.incoming_infinite_weights)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl FactorData {
    /// Creates a new factor from the given edges and minimization function.
    pub(crate) fn new(edges: &[GraphEdge], minimization_function: MinimizationFn) -> Self {
        let exchanges: Vec<_> = edges
            .iter()
            .map(|&e| WeightedValueExchange::new(e))
            .collect();
        let incoming_infinite_weights = vec![false; edges.len()];
        Self {
            minimization_function,
            exchanges,
            incoming_infinite_weights,
            enabled: true,
        }
    }

    /// Resets this factor to its initial enabled state.
    pub(crate) fn reset(&mut self) {
        self.enabled = true;
    }

    /// Enables this factor. Returns `true` if the state actually changed.
    pub(crate) fn enable(&mut self) -> bool {
        self.switch_enabled(true)
    }

    /// Disables this factor. Returns `true` if the state actually changed.
    pub(crate) fn disable(&mut self) -> bool {
        self.switch_enabled(false)
    }

    /// Returns whether this factor is currently enabled.
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the exchange buffer (read-only, for inspecting connected edges).
    pub(crate) fn exchanges(&self) -> &[WeightedValueExchange] {
        &self.exchanges
    }

    /// Runs the factor pass for this factor:
    /// 1. Reads incoming messages from all connected edges.
    /// 2. Calls the minimization function.
    /// 3. Writes results back to the edges, preserving infinite-weight certainty.
    pub(crate) fn minimize(&mut self, edge_data: &mut [EdgeData], rng: &mut dyn RngCore) {
        if !self.enabled {
            return;
        }

        // Read incoming messages into the exchange buffer.
        for (index, exchange) in self.exchanges.iter_mut().enumerate() {
            let ei = Self::edge_index(exchange, edge_data.len());
            let incoming = edge_data[ei].weighted_message_to_factor();
            self.incoming_infinite_weights[index] = incoming.weight == MessageWeight::Infinite;
            exchange.set(incoming);
        }

        // Call the minimizer.
        (self.minimization_function)(&mut self.exchanges, rng);

        // Write results back, preserving incoming infinite weight.
        for (index, exchange) in self.exchanges.iter().enumerate() {
            let mut result = exchange.get();
            if self.incoming_infinite_weights[index] {
                result.weight = MessageWeight::Infinite;
            }
            let ei = Self::edge_index(exchange, edge_data.len());
            edge_data[ei].set_result_from_factor(result);
        }
    }

    fn edge_index(exchange: &WeightedValueExchange, edge_data_len: usize) -> usize {
        let edge = exchange.edge();
        assert!(
            edge.is_valid() && edge.index() < edge_data_len,
            "FactorData references an invalid edge"
        );
        edge.index()
    }

    fn switch_enabled(&mut self, enabled: bool) -> bool {
        if self.enabled == enabled {
            return false;
        }
        self.enabled = enabled;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variable_node::VariableNode;
    use crate::weighted_value::WeightedValue;

    fn make_edges() -> Vec<EdgeData> {
        vec![
            EdgeData::new(
                VariableNode::new(0),
                WeightedValue::new(1.0, MessageWeight::Standard),
            ),
            EdgeData::new(
                VariableNode::new(1),
                WeightedValue::new(2.0, MessageWeight::Zero),
            ),
        ]
    }

    #[test]
    fn enable_disable_state() {
        let factor_edges = vec![GraphEdge::new(0)];
        let mut factor = FactorData::new(&factor_edges, Box::new(|_, _| {}));

        assert!(factor.is_enabled());
        assert!(!factor.enable()); // already enabled
        assert!(factor.disable());
        assert!(!factor.is_enabled());
        assert!(!factor.disable()); // already disabled
        assert!(factor.enable());
        assert!(factor.is_enabled());
    }

    #[test]
    fn minimization_uses_ordered_exchanges() {
        use std::cell::Cell;
        use std::rc::Rc;

        let mut edge_data = make_edges();
        let factor_edges = vec![GraphEdge::new(1), GraphEdge::new(0)];
        let minimizer_ran = Rc::new(Cell::new(false));
        let ran_clone = Rc::clone(&minimizer_ran);

        let minimizer: MinimizationFn =
            Box::new(move |exchanges: &mut [WeightedValueExchange], _| {
                ran_clone.set(true);

                assert_eq!(exchanges.len(), 2);
                assert_eq!(exchanges[0].edge(), GraphEdge::new(1));
                assert_eq!(exchanges[1].edge(), GraphEdge::new(0));
                assert_eq!(exchanges[0].get().value, 2.0);
                assert_eq!(exchanges[0].get().weight, MessageWeight::Zero);
                assert_eq!(exchanges[1].get().value, 1.0);
                assert_eq!(exchanges[1].get().weight, MessageWeight::Standard);

                exchanges[0].set(WeightedValue::new(20.0, MessageWeight::Infinite));
                exchanges[1].set(WeightedValue::new(10.0, MessageWeight::Standard));
            });

        let mut factor = FactorData::new(&factor_edges, minimizer);
        let mut rng = rand::rng();
        factor.minimize(&mut edge_data, &mut rng);

        assert!(minimizer_ran.get());
        assert_eq!(edge_data[1].x(), 20.0);
        assert_eq!(
            edge_data[1].weighted_message_to_variable().weight,
            MessageWeight::Infinite
        );
        assert_eq!(edge_data[0].x(), 10.0);
        assert_eq!(
            edge_data[0].weighted_message_to_variable().weight,
            MessageWeight::Standard
        );
    }

    #[test]
    fn incoming_infinite_weight_is_preserved() {
        let mut edge_data = vec![
            EdgeData::new(
                VariableNode::new(0),
                WeightedValue::new(3.0, MessageWeight::Infinite),
            ),
            EdgeData::new(
                VariableNode::new(1),
                WeightedValue::new(4.0, MessageWeight::Standard),
            ),
        ];

        let factor_edges = vec![GraphEdge::new(0), GraphEdge::new(1)];
        let minimizer: MinimizationFn = Box::new(|exchanges: &mut [WeightedValueExchange], _| {
            // Minimizer tries to set zero weight on both — but certainty is preserved.
            exchanges[0].set(WeightedValue::new(30.0, MessageWeight::Zero));
            exchanges[1].set(WeightedValue::new(40.0, MessageWeight::Zero));
        });

        let mut factor = FactorData::new(&factor_edges, minimizer);
        let mut rng = rand::rng();
        factor.minimize(&mut edge_data, &mut rng);

        // Edge 0 had infinite incoming → preserved as infinite outgoing.
        assert_eq!(edge_data[0].weighted_message_to_variable().value, 30.0);
        assert_eq!(
            edge_data[0].weighted_message_to_variable().weight,
            MessageWeight::Infinite
        );
        // Edge 1 had standard incoming → minimizer's zero weight is kept.
        assert_eq!(edge_data[1].weighted_message_to_variable().value, 40.0);
        assert_eq!(
            edge_data[1].weighted_message_to_variable().weight,
            MessageWeight::Zero
        );
    }

    #[test]
    fn disabled_factor_does_not_minimize() {
        let mut edge_data = make_edges();
        let factor_edges = vec![GraphEdge::new(0)];
        let minimizer: MinimizationFn = Box::new(|exchanges: &mut [WeightedValueExchange], _| {
            exchanges[0].set(WeightedValue::new(9.0, MessageWeight::Infinite));
        });

        let mut factor = FactorData::new(&factor_edges, minimizer);
        factor.disable();

        let mut rng = rand::rng();
        factor.minimize(&mut edge_data, &mut rng);

        // Edge should be untouched.
        assert_eq!(edge_data[0].x(), 0.0);
    }
}
