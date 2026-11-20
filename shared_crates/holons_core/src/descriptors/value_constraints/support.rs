use crate::descriptors::ValidExtendsLineage;
use type_names::CoreDefinitionRole;

/// Recognized configuration semantics, including declarable deferred families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintFamily {
    StringLength,
    BytesLength,
    NumericRange,
    ItemCount,
    Cardinality,
    UniqueItems,
    /// Presence-only validation of positional string template source.
    PositionalStringTemplateSyntax,
}

impl ConstraintFamily {
    /// Recognizes Core family roles in an already diagnosed lineage, including extension subtypes.
    pub fn for_lineage(lineage: ValidExtendsLineage<'_>) -> Option<Self> {
        [
            (CoreDefinitionRole::StringLengthConstraint, Self::StringLength),
            (CoreDefinitionRole::BytesLengthConstraint, Self::BytesLength),
            (CoreDefinitionRole::NumericRangeConstraint, Self::NumericRange),
            (CoreDefinitionRole::ItemCountConstraint, Self::ItemCount),
            (CoreDefinitionRole::CardinalityConstraint, Self::Cardinality),
            (CoreDefinitionRole::UniqueItemsConstraint, Self::UniqueItems),
            (
                CoreDefinitionRole::PositionalStringTemplateSyntaxConstraint,
                Self::PositionalStringTemplateSyntax,
            ),
        ]
        .into_iter()
        .find_map(|(role, family)| lineage.contains_role(role).then_some(family))
    }
}

/// Scalar families with a built-in executable implementation.
/// Family recognition for declarations does not grant execution to extension-owned types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarConstraintFamily {
    StringLength,
    BytesLength,
    NumericRange,
    /// Syntax validation for a stored String.
    PositionalStringTemplateSyntax,
}

impl ScalarConstraintFamily {
    /// Native representation consumed by this scalar family.
    pub fn expected_kind(self) -> base_types::BaseValueKind {
        match self {
            Self::StringLength | Self::PositionalStringTemplateSyntax => {
                base_types::BaseValueKind::String
            }
            Self::BytesLength => base_types::BaseValueKind::Bytes,
            Self::NumericRange => base_types::BaseValueKind::Integer,
        }
    }

    /// Selects executable semantics only for an exact Core concrete type key.
    pub fn for_concrete_type(key: &str) -> Option<Self> {
        match key {
            key if key == CoreDefinitionRole::StringLengthConstraint.key() => {
                Some(Self::StringLength)
            }
            key if key == CoreDefinitionRole::BytesLengthConstraint.key() => {
                Some(Self::BytesLength)
            }
            key if key == CoreDefinitionRole::NumericRangeConstraint.key() => {
                Some(Self::NumericRange)
            }
            key if key == CoreDefinitionRole::PositionalStringTemplateSyntaxConstraint.key() => {
                Some(Self::PositionalStringTemplateSyntax)
            }
            _ => None,
        }
    }
}
