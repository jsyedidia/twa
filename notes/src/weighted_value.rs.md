# src/weighted_value.rs

## Role In The System

This module defines the core value types used throughout the TWA: message
weights, weighted values, and the exchange type that minimizers interact with.
Every other module in the crate depends on these types.

Related concept notes:

- [notes/concepts/weights.md](../concepts/weights.md)
- [notes/concepts/message_passing.md](../concepts/message_passing.md)

## Code Walkthrough

### Import

```rust
use crate::GraphEdge;
```

`GraphEdge` identifies the edge associated with each exchange entry.

### MessageWeight Enum

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageWeight {
    Zero,
    #[default]
    Standard,
    Infinite,
}
```

The three symbolic weights. The `#[default]` attribute on `Standard` means
`MessageWeight::default()` returns `Standard`, which is the most common weight
for active messages. Deriving `Copy` makes weights trivially cheap to pass
around. `Hash` is derived so weights can be used as map keys if needed.

### `value`

```rust
impl MessageWeight {
    pub fn value(self) -> f64 {
        match self {
            MessageWeight::Zero => 0.0,
            MessageWeight::Standard => 1.0,
            MessageWeight::Infinite => f64::INFINITY,
        }
    }
}
```

Maps each symbolic weight to a numeric value. Most graph code branches on the
enum variants directly rather than using these numeric values, which avoids
fragile computations involving infinity. This method exists for the cases where
a numeric weight is genuinely needed (e.g., weighted averaging).

### WeightedValue Struct

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedValue {
    pub value: f64,
    pub weight: MessageWeight,
}
```

Pairs a proposed scalar value with a weight indicating how strongly the sender
believes in it. Both fields are public because minimizers read and write these
directly — there is no invariant to protect.

`PartialEq` is derived but not `Eq` because `f64` does not implement `Eq`
(NaN ≠ NaN).

### WeightedValue Default

```rust
impl Default for WeightedValue {
    fn default() -> Self {
        Self {
            value: 0.0,
            weight: MessageWeight::Standard,
        }
    }
}
```

The default weighted value is zero with standard weight. This cannot use
`#[derive(Default)]` because the desired default for `value` is `0.0` (which
happens to match `f64`'s default) but we want explicit control over the pairing.

### `WeightedValue::new`

```rust
impl WeightedValue {
    pub fn new(value: f64, weight: MessageWeight) -> Self {
        Self { value, weight }
    }
}
```

A convenience constructor. Since both fields are public, callers can also use
struct literal syntax directly.

### WeightedValueExchange Struct

```rust
#[derive(Debug, Clone, Copy)]
pub struct WeightedValueExchange {
    edge: GraphEdge,
    weighted_value: WeightedValue,
}
```

The exchange type is what minimizers see during the factor pass. The graph
creates one exchange entry per edge of a factor, populates it with the incoming
message (from the variable side), then hands a mutable slice to the minimizer.
The minimizer reads `get()` and writes back via `set()`.

Fields are private because the graph controls construction and the edge
association must not change after creation.

### `WeightedValueExchange::new`

```rust
impl WeightedValueExchange {
    pub fn new(edge: GraphEdge) -> Self {
        Self {
            edge,
            weighted_value: WeightedValue::default(),
        }
    }
```

Creates an exchange entry for the given edge, initialized with the default
weighted value. The graph will overwrite `weighted_value` with the actual
incoming message before passing the slice to the minimizer.

### `edge`

```rust
    pub fn edge(&self) -> GraphEdge {
        self.edge
    }
```

Returns the edge this exchange entry corresponds to. Multi-edge minimizers
like `one_hot` use this to track per-edge state (e.g., which edge carried
infinite weight).

### `get`

```rust
    pub fn get(&self) -> WeightedValue {
        self.weighted_value
    }
```

Returns the current weighted value. Minimizers call this to read the incoming
message.

### `set`

```rust
    pub fn set(&mut self, weighted_value: WeightedValue) {
        self.weighted_value = weighted_value;
    }
}
```

Overwrites the weighted value with the minimizer's output. After the minimizer
returns, the graph reads this to update the edge's factor-side state.

### Minimization Function Type

```rust
pub type MinimizationFn = Box<dyn FnMut(&mut [WeightedValueExchange], &mut dyn rand::RngCore)>;
```

Factors store minimizers behind a boxed trait object so `FactorGraph` can own
different closure types in one vector. The mutable slice is the factor's
exchange buffer; the RNG is passed through for minimizers that need
tie-breaking randomness.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_values() {
        assert_eq!(MessageWeight::Zero.value(), 0.0);
        assert_eq!(MessageWeight::Standard.value(), 1.0);
        assert!(MessageWeight::Infinite.value().is_infinite());
    }
```

Verifies the numeric mapping for all three weights.

```rust
    #[test]
    fn weight_default_is_standard() {
        assert_eq!(MessageWeight::default(), MessageWeight::Standard);
    }
```

Confirms the `#[default]` attribute works as intended.

```rust
    #[test]
    fn weighted_value_default() {
        let wv = WeightedValue::default();
        assert_eq!(wv.value, 0.0);
        assert_eq!(wv.weight, MessageWeight::Standard);
    }
```

Checks the default weighted value.

```rust
    #[test]
    fn weighted_value_new() {
        let wv = WeightedValue::new(3.14, MessageWeight::Infinite);
        assert_eq!(wv.value, 3.14);
        assert_eq!(wv.weight, MessageWeight::Infinite);
    }
```

Tests the convenience constructor.

```rust
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
```

Exercises the full exchange lifecycle: create with an edge, verify defaults,
write a new value, and read it back.
