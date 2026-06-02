# src/factor_node.rs

## Role In The System

This module defines `FactorNode`, a lightweight typed handle identifying a
factor in a `FactorGraph`. Identical in structure to `VariableNode` and
`GraphEdge` — a newtype around `usize` — but a distinct type to prevent index
mix-ups at compile time.

## Code Walkthrough

### Struct Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactorNode(pub(crate) usize);
```

A newtype around `usize`. The inner field is `pub(crate)` so internal graph
code can access the raw index, while external users cannot.

### Constants And Constructors

```rust
impl FactorNode {
    pub(crate) const INVALID: Self = Self(usize::MAX);

    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }
```

`INVALID` uses `usize::MAX` as a sentinel. The `new` constructor is
crate-internal.

### `is_valid`

```rust
    pub fn is_valid(&self) -> bool {
        self.0 != usize::MAX
    }
```

Public method checking whether the handle is the sentinel.

### `index`

```rust
    pub(crate) fn index(self) -> usize {
        self.0
    }
}
```

Crate-internal accessor for the raw index.

### Default Implementation

```rust
impl Default for FactorNode {
    fn default() -> Self {
        Self::INVALID
    }
}
```

The default handle is invalid.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_invalid() {
        let node = FactorNode::default();
        assert!(!node.is_valid());
    }
```

Confirms the default is the sentinel.

```rust
    #[test]
    fn valid_node() {
        let node = FactorNode::new(0);
        assert!(node.is_valid());
    }
```

Index 0 is valid.

```rust
    #[test]
    fn equality() {
        let a = FactorNode::new(3);
        let b = FactorNode::new(3);
        let c = FactorNode::new(4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
```

Handles with the same index are equal.
