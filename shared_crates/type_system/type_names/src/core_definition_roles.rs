//! Stable Core role vocabulary; recognition does not establish definition identity or authority.

/// Core roles recognized on definitions reached through bound references.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CoreDefinitionRole {
    /// The `TypeDescriptor` role.
    TypeDescriptor,
    /// The `HolonType.TypeDescriptor` role.
    HolonType,
    /// The `MetaTypeDescriptor.HolonType` role.
    MetaTypeDescriptor,
    /// The `MetaHolonType.MetaTypeDescriptor` role.
    MetaHolonType,
    /// The `PropertyType.TypeDescriptor` role.
    PropertyType,
    /// The `DeclaredRelationshipType.RelationshipType` role.
    DeclaredRelationshipType,
    /// The `ValueType.TypeDescriptor` role.
    ValueType,
    /// The `StringValueType.ValueType` role.
    StringValueType,
    /// The `IntegerValueType.ValueType` role.
    IntegerValueType,
    /// The `BooleanValueType.ValueType` role.
    BooleanValueType,
    /// The `BytesValueType.ValueType` role.
    BytesValueType,
    /// The `EnumValueType.ValueType` role.
    EnumValueType,
    /// The `ConstraintType.HolonType` role.
    ConstraintType,
    /// The `StringLengthConstraint.ConstraintType` role.
    StringLengthConstraint,
    /// The `BytesLengthConstraint.ConstraintType` role.
    BytesLengthConstraint,
    /// The `NumericRangeConstraint.ConstraintType` role.
    NumericRangeConstraint,
    /// The `ItemCountConstraint.ConstraintType` role.
    ItemCountConstraint,
    /// The `CardinalityConstraint.ConstraintType` role.
    CardinalityConstraint,
    /// The `UniqueItemsConstraint.ConstraintType` role.
    UniqueItemsConstraint,
    /// The `Schema.HolonType` role.
    Schema,
    /// The `Rule.HolonType` role.
    Rule,
    /// The `HolonValidationRule.HolonType` role.
    HolonValidationRule,
    /// The `PropertyValidationRule.HolonType` role.
    PropertyValidationRule,
    /// The `StringValidationRule.HolonType` role.
    StringValidationRule,
    /// The `IntegerValidationRule.HolonType` role.
    IntegerValidationRule,
    /// The `BooleanValidationRule.HolonType` role.
    BooleanValidationRule,
    /// The `BytesValidationRule.HolonType` role.
    BytesValidationRule,
    /// The `EnumValueValidationRule.HolonType` role.
    EnumValueValidationRule,
}

impl CoreDefinitionRole {
    /// Returns the exact record key identifying this role.
    pub const fn key(self) -> &'static str {
        match self {
            Self::TypeDescriptor => "TypeDescriptor",
            Self::HolonType => "HolonType.TypeDescriptor",
            Self::MetaTypeDescriptor => "MetaTypeDescriptor.HolonType",
            Self::MetaHolonType => "MetaHolonType.MetaTypeDescriptor",
            Self::PropertyType => "PropertyType.TypeDescriptor",
            Self::DeclaredRelationshipType => "DeclaredRelationshipType.RelationshipType",
            Self::ValueType => "ValueType.TypeDescriptor",
            Self::StringValueType => "StringValueType.ValueType",
            Self::IntegerValueType => "IntegerValueType.ValueType",
            Self::BooleanValueType => "BooleanValueType.ValueType",
            Self::BytesValueType => "BytesValueType.ValueType",
            Self::EnumValueType => "EnumValueType.ValueType",
            Self::ConstraintType => "ConstraintType.HolonType",
            Self::StringLengthConstraint => "StringLengthConstraint.ConstraintType",
            Self::BytesLengthConstraint => "BytesLengthConstraint.ConstraintType",
            Self::NumericRangeConstraint => "NumericRangeConstraint.ConstraintType",
            Self::ItemCountConstraint => "ItemCountConstraint.ConstraintType",
            Self::CardinalityConstraint => "CardinalityConstraint.ConstraintType",
            Self::UniqueItemsConstraint => "UniqueItemsConstraint.ConstraintType",
            Self::Schema => "Schema.HolonType",
            Self::Rule => "Rule.HolonType",
            Self::HolonValidationRule => "HolonValidationRule.HolonType",
            Self::PropertyValidationRule => "PropertyValidationRule.HolonType",
            Self::StringValidationRule => "StringValidationRule.HolonType",
            Self::IntegerValidationRule => "IntegerValidationRule.HolonType",
            Self::BooleanValidationRule => "BooleanValidationRule.HolonType",
            Self::BytesValidationRule => "BytesValidationRule.HolonType",
            Self::EnumValueValidationRule => "EnumValueValidationRule.HolonType",
        }
    }
}
