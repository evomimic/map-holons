//! Canonical identities of Core validation rules with registered handlers.

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// Fixed Commit rule identities. Canonical bindings activate independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter)]
pub enum CoreValidationRuleName {
    /// DS-PROP-001: required effective properties must be present.
    RequiredPropertyPresence,
    /// DS-PROP-003: populated properties must obey the additional-property policy.
    NoUndescribedProperties,
    /// String values use the native string representation.
    BaseValueKindMatchesString,
    /// Integer values use the native integer representation.
    BaseValueKindMatchesInteger,
    /// Boolean values use the native Boolean representation.
    BaseValueKindMatchesBoolean,
    /// Bytes values use the native byte-sequence representation.
    BaseValueKindMatchesBytes,
    /// Enum values use the native enum representation.
    BaseValueKindMatchesEnum,
    /// DS-STRUCT-002: at most one direct Extends parent.
    AtMostOneDirectParent,
    /// DS-STRUCT-003: Extends lineage has no cycle.
    AcyclicExtendsLineage,
    /// DS-STRUCT-004: parented lineage reaches TypeDescriptor.
    ExtendsLineageTerminatesAtTypeDescriptor,
    /// DS-STRUCT-005: TypeDescriptor is the unique descriptor root.
    UniqueTypeDescriptorRoot,
    /// DS-KIND-001: local instance-kind anchor designation.
    LocalInstanceKindAnchorDesignation,
    /// DS-KIND-002: kind anchors are abstract.
    InstanceKindAnchorsAreAbstract,
    /// DS-KIND-003: only TypeDescriptor lacks an instance kind.
    TypeDescriptorRootKindException,
    /// DS-KIND-004: describing category matches the graph-derived requirement.
    DescribingCategoryCompatibility,
    /// DS-KIND-005: descriptor status corresponds to a meta-type describer.
    DescriptorMetaTypeCorrespondence,
    /// DS-CONTRACT-001: inherited member identities are not redeclared.
    NoInheritedMemberRedeclaration,
    /// DS-CONTRACT-002: semantic names are unique within each member namespace.
    UniqueSemanticMemberNames,
    /// DS-CONTRACT-003: effective member definitions have C2 structure.
    WellFormedEffectiveMemberDefinitions,
    /// DS-CONTRACT-004: member instance kinds match their contract positions.
    ContractMemberKindCompatibility,
    /// DS-CONSTRAINT-001: inherited effective constraint obligations remain intact.
    InheritedValueConstraintNonRelaxation,
    /// DS-SCHEMA-001: versioned dependencies are acyclic.
    SchemaDependenciesAcyclic,
    /// DS-SCHEMA-002: authored cross-schema references have direct dependencies.
    CrossSchemaDependenciesDeclared,
}

impl CoreValidationRuleName {
    /// Recognizes an exact canonical key; unknown extension rules remain unclassified.
    /// This converts vocabulary only and does not establish runtime reference identity.
    pub fn from_key(key: &str) -> Option<Self> {
        // Derive iteration from the enum so a new variant cannot be omitted here.
        Self::iter().find(|name| name.as_str() == key)
    }

    /// Fully qualified schema key; display labels never select a handler.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaDependenciesAcyclic => "SchemaDependenciesAcyclic.ValidationRule",
            Self::CrossSchemaDependenciesDeclared => {
                "CrossSchemaDependenciesDeclared.ValidationRule"
            }
            Self::InheritedValueConstraintNonRelaxation => {
                "InheritedValueConstraintNonRelaxation.ValidationRule"
            }
            Self::RequiredPropertyPresence => "RequiredPropertyPresence.ValidationRule",
            Self::NoUndescribedProperties => "NoUndescribedProperties.ValidationRule",
            Self::BaseValueKindMatchesString => "BaseValueKindMatchesString.ValidationRule",
            Self::BaseValueKindMatchesInteger => "BaseValueKindMatchesInteger.ValidationRule",
            Self::BaseValueKindMatchesBoolean => "BaseValueKindMatchesBoolean.ValidationRule",
            Self::BaseValueKindMatchesBytes => "BaseValueKindMatchesBytes.ValidationRule",
            Self::BaseValueKindMatchesEnum => "BaseValueKindMatchesEnum.ValidationRule",
            Self::AtMostOneDirectParent => "AtMostOneDirectParent.ValidationRule",
            Self::AcyclicExtendsLineage => "AcyclicExtendsLineage.ValidationRule",
            Self::ExtendsLineageTerminatesAtTypeDescriptor => {
                "ExtendsLineageTerminatesAtTypeDescriptor.ValidationRule"
            }
            Self::UniqueTypeDescriptorRoot => "UniqueTypeDescriptorRoot.ValidationRule",
            Self::LocalInstanceKindAnchorDesignation => {
                "LocalInstanceKindAnchorDesignation.ValidationRule"
            }
            Self::InstanceKindAnchorsAreAbstract => "InstanceKindAnchorsAreAbstract.ValidationRule",
            Self::TypeDescriptorRootKindException => {
                "TypeDescriptorRootKindException.ValidationRule"
            }
            Self::DescribingCategoryCompatibility => {
                "DescribingCategoryCompatibility.ValidationRule"
            }
            Self::DescriptorMetaTypeCorrespondence => {
                "DescriptorMetaTypeCorrespondence.ValidationRule"
            }
            Self::NoInheritedMemberRedeclaration => "NoInheritedMemberRedeclaration.ValidationRule",
            Self::UniqueSemanticMemberNames => "UniqueSemanticMemberNames.ValidationRule",
            Self::WellFormedEffectiveMemberDefinitions => {
                "WellFormedEffectiveMemberDefinitions.ValidationRule"
            }
            Self::ContractMemberKindCompatibility => {
                "ContractMemberKindCompatibility.ValidationRule"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CoreValidationRuleName::{self, *};
    use strum::IntoEnumIterator;

    #[test]
    fn every_variant_has_a_unique_round_trip_key() {
        let mut keys = std::collections::HashSet::new();
        for rule in CoreValidationRuleName::iter() {
            assert!(keys.insert(rule.as_str()), "duplicate canonical rule key");
            assert_eq!(CoreValidationRuleName::from_key(rule.as_str()), Some(rule));
        }
    }

    #[test]
    fn unknown_or_noncanonical_keys_are_not_recognized() {
        for key in [
            "Unknown.ValidationRule",
            "RequiredPropertyPresence",
            "requiredpropertypresence.validationrule",
            "RequiredPropertyPresence.ValidationRule ",
        ] {
            assert_eq!(super::CoreValidationRuleName::from_key(key), None);
        }
    }

    #[test]
    fn canonical_keys_are_fully_qualified() {
        for (rule, expected) in [
            (RequiredPropertyPresence, "RequiredPropertyPresence.ValidationRule"),
            (NoUndescribedProperties, "NoUndescribedProperties.ValidationRule"),
            (BaseValueKindMatchesString, "BaseValueKindMatchesString.ValidationRule"),
            (BaseValueKindMatchesInteger, "BaseValueKindMatchesInteger.ValidationRule"),
            (BaseValueKindMatchesBoolean, "BaseValueKindMatchesBoolean.ValidationRule"),
            (BaseValueKindMatchesBytes, "BaseValueKindMatchesBytes.ValidationRule"),
            (BaseValueKindMatchesEnum, "BaseValueKindMatchesEnum.ValidationRule"),
        ] {
            assert_eq!(rule.as_str(), expected);
            assert_eq!(super::CoreValidationRuleName::from_key(expected), Some(rule));
        }
    }
}
