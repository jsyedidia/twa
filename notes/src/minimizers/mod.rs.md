# src/minimizers/mod.rs

## Role In The System

This module groups the built-in minimizer factories and re-exports their public
constructor functions.

## Code Walkthrough

### Module Documentation

```rust
//! Built-in minimizer factories.
```

The module-level documentation identifies this directory as the home of
reusable factor constructors.

### Child Modules

```rust
pub mod in_range;
pub mod known_value;
pub mod one_hot;
pub mod spy;
```

Each child module owns one minimizer factory.

### Re-exports

```rust
pub use in_range::create_in_range_factor;
pub use known_value::create_known_value_factor;
pub use one_hot::create_one_hot_factor;
pub use spy::create_spy_factor;
```

The re-exports let users import the constructors from `twa::minimizers`.

## Important Invariants

- This module should stay thin; minimizer behavior belongs in the child files.
- Public factory names should remain stable once downstream problem builders
  use them.

## Algorithm Context

Minimizers are the factor-local rules that project incoming messages onto a
constraint. This module only organizes those rules.

## Extension Notes

Add future built-in minimizers here only after adding their implementation,
tests, and source note.
