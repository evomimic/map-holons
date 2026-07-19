use std::{fmt, hash::Hash};

/// Read-only graph access required by representation-neutral descriptor algorithms.
///
/// Target collections remain uncollapsed so the semantic kernel, rather than an adapter, owns
/// cardinality errors. `Identity` must represent node identity, not a display name.
pub trait DescriptorGraph {
    type Node: Clone;
    type Identity: Clone + Eq + Hash;
    type MemberKind;
    type Error;

    fn identity(&self, node: &Self::Node) -> Self::Identity;

    fn extends_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error>;

    fn described_by_targets(&self, node: &Self::Node) -> Result<Vec<Self::Node>, Self::Error>;

    fn related_members(
        &self,
        node: &Self::Node,
        member_kind: &Self::MemberKind,
    ) -> Result<Vec<Self::Node>, Self::Error>;

    /// Returns whether `node` is the descriptor that describes type-descriptor holons.
    fn is_type_descriptor(&self, node: &Self::Node) -> Result<bool, Self::Error>;
}

/// Structural descriptor-graph failure independent of a concrete representation's error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorSemanticsError<E, N> {
    Access(E),
    MultipleExtends { descriptor: N, count: usize },
    MultipleDescribedBy { holon: N, count: usize },
    CyclicExtends { descriptor: N },
}

impl<E: fmt::Display, N: fmt::Debug> fmt::Display for DescriptorSemanticsError<E, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => write!(formatter, "{error}"),
            Self::MultipleExtends { descriptor, count } => write!(
                formatter,
                "descriptor {descriptor:?} has {count} Extends targets; expected at most one"
            ),
            Self::MultipleDescribedBy { holon, count } => write!(
                formatter,
                "holon {holon:?} has {count} DescribedBy targets; expected at most one"
            ),
            Self::CyclicExtends { descriptor } => {
                write!(formatter, "cyclic Extends chain at {descriptor:?}")
            }
        }
    }
}
