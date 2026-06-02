# src/variable_node.rs

## Role In The System

This module defines `VariableNode`, a lightweight typed handle identifying a
variable in a `FactorGraph`. The newtype wrapper prevents accidental mixing
with `FactorNode` or `GraphEdge` indices at compile time.

## Code Walkthrough

### Struct Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableNode(pub(crate) usize);
```

A newtype around `usize`. The inner field is `pub(crate)` so that internal
graph code can access the raw index directly, while external users cannot
construct or inspect arbitrary indices. Deriving `Copy` makes handles trivially
cheap to pass around — they are just a machine word.

### Constants And Constructors

```rust
impl VariableNode {
    pub(crate) const INVALID: Self = Self(usize::MAX);

    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }
```

`INVALID` uses `usize::MAX` as a sentinel for "no variable". The `new`
constructor is crate-internal — only the `FactorGraph` should create handles.

### `is_valid`

```rust
    pub fn is_valid(&self) -> bool {
        self.0 != usize::MAX
    }
```

Public method that checks whether the handle refers to a real variable or is
the sentinel value.

### `index`

```rust
    pub(crate) fn index(self) -> usize {
        self.0
    }
}
```

Crate-internal accessor for the raw index. Used by graph internals to index
into storage vectors.

### Default Implementation

```rust
impl Default for VariableNode {
    fn default() -> Self {
        Self::INVALID
    }
}
```

The default handle is invalid. This is useful for initializing arrays of
handles before they are assigned real values.

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_invalid() {
        let node = VariableNode::default();
        assert!(!node.is_valid());
    }
```

Confirms the default is the sentinel.

```rust
    #[test]
    fn valid_node() {
        let node = VariableNode::new(0);
        assert!(node.is_valid());
    }
```

Index 0 is a valid handle (only `usize::MAX` is invalid).

```rust
    #[test]
    fn equality() {
        let a = VariableNode::new(3);
        let b = VariableNode::new(3);
        let c = VariableNode::new(4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
```

Handles with the same index are equal; different indices are not.
