use crate::GraphEdge;

/// The three message weights used by the TWA.
///
/// Each weight expresses how strongly a sender believes in its proposed value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageWeight {
    /// No opinion — the sender is passive.
    Zero,
    /// Ordinary finite opinion.
    #[default]
    Standard,
    /// Certainty — the value is non-negotiable.
    Infinite,
}

impl MessageWeight {
    /// Returns the numeric value associated with this weight.
    ///
    /// - `Zero` → `0.0`
    /// - `Standard` → `1.0`
    /// - `Infinite` → `f64::INFINITY`
    pub fn value(self) -> f64 {
        match self {
            MessageWeight::Zero => 0.0,
            MessageWeight::Standard => 1.0,
            MessageWeight::Infinite => f64::INFINITY,
        }
    }
}

/// A scalar value paired with a message weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedValue {
    pub value: f64,
    pub weight: MessageWeight,
}

impl Default for WeightedValue {
    fn default() -> Self {
        Self {
            value: 0.0,
            weight: MessageWeight::Standard,
        }
    }
}

impl WeightedValue {
    /// Creates a new weighted value.
    pub fn new(value: f64, weight: MessageWeight) -> Self {
        Self { value, weight }
    }
}

/// A weighted value exchange entry used by minimizers.
///
/// Each exchange entry corresponds to one edge of a factor. During the factor
/// pass, the graph populates the exchange with the incoming message (from the
/// variable side), then the minimizer reads and overwrites it with its
/// preferred output.
#[derive(Debug, Clone, Copy)]
pub struct WeightedValueExchange {
    edge: GraphEdge,
    weighted_value: WeightedValue,
}

impl WeightedValueExchange {
    /// Creates a new exchange entry for the given edge.
    pub fn new(edge: GraphEdge) -> Self {
        Self {
            edge,
            weighted_value: WeightedValue::default(),
        }
    }

    /// Returns the edge this exchange entry corresponds to.
    pub fn edge(&self) -> GraphEdge {
        self.edge
    }

    /// Returns the current weighted value.
    pub fn get(&self) -> WeightedValue {
        self.weighted_value
    }

    /// Sets the weighted value (called by minimizers to write their output).
    pub fn set(&mut self, weighted_value: WeightedValue) {
        self.weighted_value = weighted_value;
    }
}

/// The type of minimization functions attached to factors.
///
/// A minimizer receives a mutable slice of exchange entries (one per edge) and
/// a random number generator. It reads the incoming messages via `get()` and
/// writes its preferred values via `set()`.
pub type MinimizationFn = Box<dyn FnMut(&mut [WeightedValueExchange], &mut dyn rand::RngCore)>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_values() {
        assert_eq!(MessageWeight::Zero.value(), 0.0);
        assert_eq!(MessageWeight::Standard.value(), 1.0);
        assert!(MessageWeight::Infinite.value().is_infinite());
    }

    #[test]
    fn weight_default_is_standard() {
        assert_eq!(MessageWeight::default(), MessageWeight::Standard);
    }

    #[test]
    fn weighted_value_default() {
        let wv = WeightedValue::default();
        assert_eq!(wv.value, 0.0);
        assert_eq!(wv.weight, MessageWeight::Standard);
    }

    #[test]
    fn weighted_value_new() {
        let wv = WeightedValue::new(3.14, MessageWeight::Infinite);
        assert_eq!(wv.value, 3.14);
        assert_eq!(wv.weight, MessageWeight::Infinite);
    }

    #[test]
    fn exchange_roundtrip() {
        let edge = GraphEdge::new(5);
        let mut ex = WeightedValueExchange::new(edge);
        assert_eq!(ex.edge(), edge);
        assert_eq!(ex.get().value, 0.0);
        assert_eq!(ex.get().weight, MessageWeight::Standard);

        ex.set(WeightedValue::new(7.0, MessageWeight::Infinite));
        assert_eq!(ex.get().value, 7.0);
        assert_eq!(ex.get().weight, MessageWeight::Infinite);
    }
}
