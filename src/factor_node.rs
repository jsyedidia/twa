/// A typed handle identifying a factor in a [`FactorGraph`](crate::FactorGraph).
///
/// This is a lightweight newtype around `usize`. An invalid node uses
/// `usize::MAX` as a sentinel value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactorNode(pub(crate) usize);

impl FactorNode {
    pub(crate) const INVALID: Self = Self(usize::MAX);

    /// Creates a new factor node handle with the given index.
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns `true` if this handle refers to a valid factor.
    pub fn is_valid(&self) -> bool {
        self.0 != usize::MAX
    }

    /// Returns the underlying index.
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

impl Default for FactorNode {
    fn default() -> Self {
        Self::INVALID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_invalid() {
        let node = FactorNode::default();
        assert!(!node.is_valid());
    }

    #[test]
    fn valid_node() {
        let node = FactorNode::new(0);
        assert!(node.is_valid());
    }

    #[test]
    fn equality() {
        let a = FactorNode::new(3);
        let b = FactorNode::new(3);
        let c = FactorNode::new(4);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
