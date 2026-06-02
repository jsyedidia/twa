# src/graph_edge.rs

## Role In The System

This module defines `GraphEdge`, a lightweight typed handle identifying an edge
in a `FactorGraph`. Each edge connects exactly one factor to exactly one
variable. The newtype prevents accidental mixing with variable or factor
indices.

## Code Walkthrough

### Struct Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphEdge(pub(crate) usize);
```

A newtype around `usize`. The inner field is `pub(crate)` so internal graph
code can access the raw index, while external users cannot.

### Constants And Constructors

```rust
impl GraphEdge {
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
impl Default for GraphEdge {
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
        let edge = GraphEdge::default();
        assert!(!edge.is_valid());
    }
```

Confirms the default is the sentinel.

```rust
    #[test]
    fn valid_edge() {
        let edge = GraphEdge::new(0);
        assert!(edge.is_valid());
    }
```

Index 0 is valid.

```rust
    #[test]
    fn equality() {
        let a = GraphEdge::new(3);
        let b = GraphEdge::new(3);
        let c = GraphEdge::new(4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
```

Handles with the same index are equal.
