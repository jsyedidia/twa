use crate::variable_node::VariableNode;
use crate::weighted_value::{MessageWeight, WeightedValue};

/// Internal per-edge storage for the TWA message-passing algorithm.
///
/// Each edge connects one factor to one variable. It stores the factor-side
/// local value `x`, the variable-side consensus value `z`, the accumulated
/// disagreement `u`, directional message weights, and convergence bookkeeping.
#[derive(Debug, Clone)]
pub(crate) struct EdgeData {
    variable: VariableNode,
    x: f64,
    u: f64,
    z: f64,
    old_message_to_factor: Option<f64>,
    message_difference: Option<f64>,
    weight_to_left: MessageWeight,
    weight_to_right: MessageWeight,
    enabled: bool,
}

impl EdgeData {
    /// Creates a new edge connecting to the given variable, initialized with
    /// the provided value and weight.
    pub(crate) fn new(variable: VariableNode, initial_value: WeightedValue) -> Self {
        let mut edge = Self {
            variable,
            x: 0.0,
            u: 0.0,
            z: 0.0,
            old_message_to_factor: None,
            message_difference: None,
            weight_to_left: MessageWeight::Zero,
            weight_to_right: MessageWeight::Zero,
            enabled: true,
        };
        edge.reset(initial_value);
        edge
    }

    /// Resets the edge to the given initial value, clearing all accumulated
    /// state and re-enabling the edge.
    pub(crate) fn reset(&mut self, reset_value: WeightedValue) {
        self.enabled = true;
        self.x = 0.0;
        self.u = 0.0;
        self.z = reset_value.value;
        self.weight_to_left = reset_value.weight;
        self.weight_to_right = MessageWeight::Zero;
        self.old_message_to_factor = None;
        self.message_difference = None;
    }

    /// Disables this edge so it does not participate in iteration.
    pub(crate) fn disable(&mut self) {
        self.enabled = false;
    }

    /// Returns the variable this edge is connected to.
    pub(crate) fn variable(&self) -> VariableNode {
        self.variable
    }

    /// Returns whether this edge is currently enabled.
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the factor-side local value.
    pub(crate) fn x(&self) -> f64 {
        self.x
    }

    /// Returns the variable-side consensus value.
    pub(crate) fn z(&self) -> f64 {
        self.z
    }

    /// Returns the accumulated disagreement.
    pub(crate) fn u(&self) -> f64 {
        self.u
    }

    /// Returns the scalar message from variable to factor: `z - u`.
    pub(crate) fn message_to_factor(&self) -> f64 {
        self.z - self.u
    }

    /// Returns the scalar message from factor to variable: `x + u`.
    pub(crate) fn message_to_variable(&self) -> f64 {
        self.x + self.u
    }

    /// Returns the weighted message from variable to factor.
    pub(crate) fn weighted_message_to_factor(&self) -> WeightedValue {
        WeightedValue::new(self.message_to_factor(), self.weight_to_left)
    }

    /// Returns the weighted message from factor to variable.
    pub(crate) fn weighted_message_to_variable(&self) -> WeightedValue {
        WeightedValue::new(self.message_to_variable(), self.weight_to_right)
    }

    /// Returns the absolute change in the message-to-factor since the last
    /// factor update, or `None` if fewer than two factor updates have occurred.
    pub(crate) fn message_difference(&self) -> Option<f64> {
        self.message_difference
    }

    /// Updates edge state after the factor has computed its result.
    ///
    /// Sets `x` and the rightward weight from the factor's output, computes
    /// the message difference diagnostic, and resets `u` to zero
    /// if the factor emitted infinite weight (certainty kills disagreement).
    pub(crate) fn set_result_from_factor(&mut self, result: WeightedValue) {
        self.x = result.value;
        self.weight_to_right = result.weight;

        let current_message = self.message_to_factor();
        if let Some(old) = self.old_message_to_factor {
            self.message_difference = Some((current_message - old).abs());
        }
        self.old_message_to_factor = Some(current_message);

        if self.weight_to_right == MessageWeight::Infinite {
            self.u = 0.0;
        }
    }

    /// Updates edge state after the variable has computed consensus.
    ///
    /// Sets `z` and the leftward weight from the variable's result, then
    /// updates `u`. Disagreement is reset to zero when:
    /// - `reset_disagreement` is explicitly requested, or
    /// - the leftward weight is infinite (variable is certain), or
    /// - the rightward weight is infinite (factor was certain), or
    /// - the rightward weight is zero (factor has no opinion).
    pub(crate) fn set_result_from_variable(
        &mut self,
        result: WeightedValue,
        alpha: f64,
        reset_disagreement: bool,
    ) {
        self.z = result.value;
        self.weight_to_left = result.weight;

        if self.should_reset_disagreement(reset_disagreement) {
            self.u = 0.0;
        } else {
            self.u += alpha * (self.x - self.z);
        }
    }

    fn should_reset_disagreement(&self, reset_disagreement: bool) -> bool {
        reset_disagreement
            || self.weight_to_left == MessageWeight::Infinite
            || self.weight_to_right == MessageWeight::Infinite
            || self.weight_to_right == MessageWeight::Zero
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-12
    }

    #[test]
    fn reset_and_messages() {
        let edge = EdgeData::new(
            VariableNode::new(4),
            WeightedValue::new(2.0, MessageWeight::Infinite),
        );

        assert_eq!(edge.variable(), VariableNode::new(4));
        assert!(edge.is_enabled());
        assert_eq!(edge.x(), 0.0);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.z(), 2.0);
        assert_eq!(edge.message_to_factor(), 2.0);
        assert_eq!(edge.message_to_variable(), 0.0);
        assert_eq!(
            edge.weighted_message_to_factor().weight,
            MessageWeight::Infinite
        );
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Zero
        );
        assert!(edge.message_difference().is_none());
    }

    #[test]
    fn factor_and_variable_updates() {
        let mut edge = EdgeData::new(
            VariableNode::new(1),
            WeightedValue::new(2.0, MessageWeight::Standard),
        );

        // Factor sets x=5 with infinite weight → u resets to 0.
        edge.set_result_from_factor(WeightedValue::new(5.0, MessageWeight::Infinite));
        assert_eq!(edge.x(), 5.0);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.weighted_message_to_variable().value, 5.0);
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Infinite
        );
        assert!(edge.message_difference().is_none());

        // Variable sets z=3 with standard weight. Right weight is infinite → u resets.
        edge.set_result_from_variable(
            WeightedValue::new(3.0, MessageWeight::Standard),
            0.25,
            false,
        );
        assert_eq!(edge.z(), 3.0);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.message_to_factor(), 3.0);
        assert_eq!(
            edge.weighted_message_to_factor().weight,
            MessageWeight::Standard
        );

        // Factor sets x=6 with standard weight. Now message_difference exists.
        edge.set_result_from_factor(WeightedValue::new(6.0, MessageWeight::Standard));
        assert_eq!(edge.x(), 6.0);
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Standard
        );
        assert!(edge.message_difference().is_some());
        assert!(nearly_equal(edge.message_difference().unwrap(), 1.0));

        // Variable sets z=4. Standard/standard → u accumulates.
        edge.set_result_from_variable(
            WeightedValue::new(4.0, MessageWeight::Standard),
            0.25,
            false,
        );
        assert_eq!(edge.z(), 4.0);
        assert!(nearly_equal(edge.u(), 0.5));
        assert!(nearly_equal(edge.message_to_factor(), 3.5));

        // Variable sets z=4 with infinite weight → u resets.
        edge.set_result_from_variable(
            WeightedValue::new(4.0, MessageWeight::Infinite),
            0.25,
            false,
        );
        assert_eq!(edge.z(), 4.0);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(
            edge.weighted_message_to_factor().weight,
            MessageWeight::Infinite
        );
        assert_eq!(edge.message_to_factor(), 4.0);
    }

    #[test]
    fn zero_weight_to_variable_resets_disagreement() {
        let mut edge = EdgeData::new(
            VariableNode::new(1),
            WeightedValue::new(2.0, MessageWeight::Standard),
        );

        edge.set_result_from_factor(WeightedValue::new(7.0, MessageWeight::Standard));
        edge.set_result_from_variable(WeightedValue::new(3.0, MessageWeight::Standard), 0.5, false);
        assert!(nearly_equal(edge.u(), 2.0));

        // Factor emits zero weight → next variable update resets u.
        edge.set_result_from_factor(WeightedValue::new(5.0, MessageWeight::Zero));
        edge.set_result_from_variable(WeightedValue::new(4.0, MessageWeight::Standard), 0.5, false);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.message_to_factor(), 4.0);
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Zero
        );
    }

    #[test]
    fn explicit_reset_disagreement_flag() {
        let mut edge = EdgeData::new(
            VariableNode::new(1),
            WeightedValue::new(2.0, MessageWeight::Standard),
        );

        edge.set_result_from_factor(WeightedValue::new(7.0, MessageWeight::Standard));
        edge.set_result_from_variable(WeightedValue::new(3.0, MessageWeight::Standard), 0.5, false);
        assert!(nearly_equal(edge.u(), 2.0));

        edge.set_result_from_factor(WeightedValue::new(8.0, MessageWeight::Standard));
        // Explicit reset_disagreement=true → u resets despite standard/standard.
        edge.set_result_from_variable(WeightedValue::new(10.0, MessageWeight::Standard), 0.5, true);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.message_to_factor(), 10.0);
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Standard
        );
    }

    #[test]
    fn disable_and_reset() {
        let mut edge = EdgeData::new(
            VariableNode::new(3),
            WeightedValue::new(1.0, MessageWeight::Standard),
        );

        edge.disable();
        assert!(!edge.is_enabled());

        edge.set_result_from_factor(WeightedValue::new(7.0, MessageWeight::Standard));
        edge.set_result_from_variable(WeightedValue::new(2.0, MessageWeight::Standard), 0.5, false);
        assert!(edge.u() != 0.0);

        edge.reset(WeightedValue::new(9.0, MessageWeight::Zero));
        assert!(edge.is_enabled());
        assert_eq!(edge.x(), 0.0);
        assert_eq!(edge.u(), 0.0);
        assert_eq!(edge.z(), 9.0);
        assert_eq!(
            edge.weighted_message_to_factor().weight,
            MessageWeight::Zero
        );
        assert_eq!(
            edge.weighted_message_to_variable().weight,
            MessageWeight::Zero
        );
        assert!(edge.message_difference().is_none());
    }
}
