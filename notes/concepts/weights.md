# Weights

This note explains how message weights work in this crate. It assumes the
basic message-passing picture from `message_passing.md`: factors send messages
to variables, variables send messages back to factors, and each edge stores the
local values `x`, `z`, and `u`.

## What A Weighted Value Represents

The code packages a scalar value and a message weight together:

```rust
pub struct WeightedValue {
    pub value: f64,
    pub weight: MessageWeight,
}
```

The value says what number the sender is proposing. The weight says how the
receiver should treat that proposal.

For example, `WeightedValue { value: 7.0, weight: MessageWeight::Infinite }`
means "the value should be 7, and I am certain."

And `WeightedValue { value: 0.43, weight: MessageWeight::Zero }` means "I am
passing along the value 0.43, but I have no active opinion about it."

## The Three Weights

```rust
pub enum MessageWeight {
    Zero,
    Standard,
    Infinite,
}
```

The intended meanings are:

- `Zero` — no opinion.
- `Standard` — ordinary finite opinion.
- `Infinite` — certainty.

The method `MessageWeight::value() -> f64` maps these to numeric values:

- `Zero` → `0.0`
- `Standard` → `1.0`
- `Infinite` → `f64::INFINITY`

Most of the graph code does not do arithmetic directly with these numeric
weights. It branches on the three symbolic cases instead. That keeps the TWA
rules explicit and avoids fragile computations involving infinity.

## How Weights Affect Consensus

During the variable pass, the variable must combine incoming messages from all
connected factors. The three weights create a priority system:

1. **If any incoming message has infinite weight**, the variable adopts the
   first such message in enabled-edge order. The implementation does not check
   whether multiple infinite-weight messages agree, so callers and minimizers
   must maintain that invariant.

2. **If no message has infinite weight**, the variable computes the arithmetic
   mean of all standard-weight messages. Every standard weight has the same
   numeric value, `1.0`.

3. **Zero-weight messages are ignored** when any standard-weight message exists.
   When all messages have zero weight, the variable averages all their scalar
   values and reports a zero-weight consensus.

This three-level priority is what gives the algorithm its name. It allows:

- Known values (givens in Sudoku) to be expressed as infinite weight.
- Satisfied constraints to go passive by emitting zero weight.
- Active constraints to negotiate through standard weight.

## Weight Direction

Each edge carries weights in both directions:

- The **leftward weight** (variable → factor direction): set from the
  variable's consensus result.
- The **rightward weight** (factor → variable direction): set from the factor
  minimizer's output.

The edge note [src/edge_data.rs.md](../src/edge_data.rs.md) shows the concrete
storage and accessors for these directions. The weighted value note
[src/weighted_value.rs.md](../src/weighted_value.rs.md) shows the public
`MessageWeight` and `WeightedValue` types.

## Certainty Preservation

When a factor receives an incoming message with infinite weight, it preserves
infinite weight on the same edge's outgoing message. The minimizer still
chooses the outgoing scalar value, so this mechanism preserves the certainty
status, not necessarily the input value. Each minimizer decides how a certain
input interacts with its constraint; for example, `one_hot` uses certain
inputs for constraint propagation.

`FactorData` implements this by recording which incoming exchanges had
infinite weight before the minimizer runs, then restoring infinite weight on
those same outgoing exchanges afterward. See
[src/factor_data.rs.md](../src/factor_data.rs.md).

## How Built-In Factors Use Weights

The built-in minimizer notes are the best place to see the three weights in
small, local examples:

- [src/minimizers/known_value.rs.md](../src/minimizers/known_value.rs.md)
  emits infinite weight because the value is known with certainty.
- [src/minimizers/in_range.rs.md](../src/minimizers/in_range.rs.md) emits
  zero weight when the incoming value already satisfies the range, and
  standard weight when it clamps to a boundary.
- [src/minimizers/one_hot.rs.md](../src/minimizers/one_hot.rs.md) uses
  standard, zero, and infinite incoming weights to decide which candidate wins
  and how strongly to assert that choice.
- [src/minimizers/spy.rs.md](../src/minimizers/spy.rs.md) can either pass
  messages through with zero weight or emit a caller-provided weighted value.

Sudoku mostly demonstrates standard and infinite weights; see
[sudoku.md](sudoku.md). Circle packing demonstrates zero-weight satisfied
constraints heavily; see [circle_packing.md](circle_packing.md).

## Further Reading

- [message_passing.md](message_passing.md) explains how weighted messages move
  through one iteration.
- [src/factor_graph.rs.md](../src/factor_graph.rs.md) explains the variable
  equality calculation that combines incoming weights.
- [src/edge_data.rs.md](../src/edge_data.rs.md) explains how weights are reset
  when an edge is created, reinitialized, disabled, or reenabled.
