/// A typed handle identifying an edge in a [`FactorGraph`](crate::FactorGraph).
///
/// This is a lightweight newtype around `usize`. An invalid edge uses
/// `usize::MAX` as a sentinel value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphEdge(pub(crate) usize);

impl GraphEdge {
    pub(crate) const INVALID: Self = Self(usize::MAX);

    /// Creates a new edge handle with the given index.
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns `true` if this handle refers to a valid edge.
    pub fn is_valid(&self) -> bool {
        self.0 != usize::MAX
    }

    /// Returns the underlying index.
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

impl Default for GraphEdge {
    fn default() -> Self {
        Self::INVALID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_invalid() {
        let edge = GraphEdge::default();
        assert!(!edge.is_valid());
    }

    #[test]
    fn valid_edge() {
        let edge = GraphEdge::new(0);
        assert!(edge.is_valid());
    }

    #[test]
    fn equality() {
        let a = GraphEdge::new(3);
        let b = GraphEdge::new(3);
        let c = GraphEdge::new(4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
