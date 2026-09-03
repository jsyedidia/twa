# src/edge_data.rs

## Role In The System

`EdgeData` is the internal storage for one graph edge. It is not part of the
public API; users hold `GraphEdge` handles, while `FactorGraph` owns the actual
`EdgeData` objects.

An edge connects one factor to one variable. It stores the factor-side local
value `x`, the variable-side consensus value `z`, the accumulated disagreement
`u`, directional message weights, and enough previous-message state to report
message-difference diagnostics.

Related concept notes:

- [notes/concepts/message_passing.md](../concepts/message_passing.md)
- [notes/concepts/weights.md](../concepts/weights.md)

## State At A Glance

```rust
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
```

- `variable` — the variable endpoint of this edge; fixed at creation.
- `x` — the factor-side local value.
- `z` — the variable-side consensus value.
- `u` — the accumulated disagreement (dual variable in ADMM terms).
- `old_message_to_factor` — the previous `z - u` value, used to compute a
  message-difference diagnostic.
- `message_difference` — `|current - old|` message change, exposed through
  `FactorGraph::max_message_difference()`.
- `weight_to_left` — weight on the variable→factor message direction.
- `weight_to_right` — weight on the factor→variable message direction.
- `enabled` — whether this edge participates in iteration.

## Code Walkthrough

### Imports

```rust
use crate::weighted_value::{MessageWeight, WeightedValue};
use crate::variable_node::VariableNode;
```

The edge needs the weight enum, weighted value type, and the variable handle.

### Struct Definition

```rust
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
```

`pub(crate)` visibility — only the crate internals (primarily `FactorGraph`)
can access this type. `Clone` is derived so the graph can snapshot state if
needed. All fields are private.

### `new`

```rust
impl EdgeData {
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
```

Constructs an edge, then immediately calls `reset` to apply the initial value.
The two-step construction ensures `reset` is the single source of truth for
initialization logic.

### `reset`

```rust
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
```

Clears all accumulated state. The variable-side consensus `z` takes the reset
value; the factor-side `x` starts at zero. The leftward weight (variable→factor
direction) adopts the reset weight; the rightward weight starts at zero (no
factor opinion yet). Message-difference bookkeeping is cleared. The edge is
re-enabled.

### `disable`

```rust
    pub(crate) fn disable(&mut self) {
        self.enabled = false;
    }
```

Marks the edge as inactive. Disabled edges are skipped during factor/variable
passes and message-difference diagnostics.

### Accessors

```rust
    pub(crate) fn variable(&self) -> VariableNode {
        self.variable
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn x(&self) -> f64 {
        self.x
    }

    pub(crate) fn z(&self) -> f64 {
        self.z
    }

    pub(crate) fn u(&self) -> f64 {
        self.u
    }
```

Simple getters for the edge's core state. The `x`, `z`, and `u` inspectors are
compiled only for tests; production iteration uses message-level accessors
instead of reaching into those fields directly.

### `message_to_factor`

```rust
    pub(crate) fn message_to_factor(&self) -> f64 {
        self.z - self.u
    }
```

The scalar message from variable to factor: `n = z - u`. This is the value the
factor sees as the "incoming opinion" from the variable side.

### `message_to_variable`

```rust
    pub(crate) fn message_to_variable(&self) -> f64 {
        self.x + self.u
    }
```

The scalar message from factor to variable: `m = x + u`. This is the value the
variable sees as the factor's "incoming opinion."

### `weighted_message_to_factor`

```rust
    pub(crate) fn weighted_message_to_factor(&self) -> WeightedValue {
        WeightedValue::new(self.message_to_factor(), self.weight_to_left)
    }
```

Packages the scalar variable→factor message with its weight. The weight tells
the factor how strongly the variable believes in this value.

### `weighted_message_to_variable`

```rust
    pub(crate) fn weighted_message_to_variable(&self) -> WeightedValue {
        WeightedValue::new(self.message_to_variable(), self.weight_to_right)
    }
```

Packages the scalar factor→variable message with its weight. The weight tells
the variable how strongly the factor believes in this value.

### `message_difference`

```rust
    pub(crate) fn message_difference(&self) -> Option<f64> {
        self.message_difference
    }
```

Returns the absolute change in the message-to-factor since the last factor
update. `None` until at least two factor updates have occurred. This is a
dual-state diagnostic; the default graph convergence check is based on variable
belief values.

### `set_result_from_factor`

```rust
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
```

Called after the factor pass. Updates `x` and the rightward weight from the
minimizer's output. Computes the message difference diagnostic by comparing the
current `z - u` against the stored previous value. If the factor emitted
infinite weight (certainty), disagreement `u` is reset to zero — there is no
point accumulating disagreement against a certain value.

### `set_result_from_variable`

```rust
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
```

Called after the variable pass. Updates `z` and the leftward weight from the
variable's consensus result. Then updates `u`:

- If disagreement should be reset (see below), `u` becomes zero.
- Otherwise, `u` accumulates: `u += alpha * (x - z)`. The `alpha` parameter
  is the learning rate. When `alpha = 1.0`, this is the standard ADMM update.

### `should_reset_disagreement`

```rust
    fn should_reset_disagreement(&self, reset_disagreement: bool) -> bool {
        reset_disagreement
            || self.weight_to_left == MessageWeight::Infinite
            || self.weight_to_right == MessageWeight::Infinite
            || self.weight_to_right == MessageWeight::Zero
    }
```

Disagreement is reset when:

1. The caller explicitly requests it (`reset_disagreement = true`).
2. The variable is certain (infinite left weight).
3. The factor was certain (infinite right weight).
4. The factor has no opinion (zero right weight) — there is nothing to
   disagree about.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn nearly_equal(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-12
    }
```

A helper for floating-point comparison with tolerance.

```rust
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
```

Verifies fresh edge state: `z` takes the initial value, `x` and `u` are zero,
messages are computed correctly, weights are set as expected.

```rust
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
```

Exercises a sequence of factor and variable updates, testing that `x`, `z`,
`u`, weights, and message differences evolve correctly through multiple rounds.
Key behaviors tested: infinite-weight factor resets `u`, standard/standard
accumulates `u`, infinite-weight variable resets `u`.

```rust
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
```

Tests that when a factor emits zero weight (no opinion), the subsequent
variable update resets `u` to zero.

```rust
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
```

Tests that passing `reset_disagreement = true` forces `u` to zero even when
both weights are standard.

```rust
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
```

Tests that `disable()` marks the edge inactive, and that `reset()` restores
full initial state including re-enabling and clearing convergence bookkeeping.
