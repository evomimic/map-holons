//! Canonical identities of the first active Core validation rules.

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// Fixed Commit rules supported by the initial holon/property/value cohort.
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
            Self::RequiredPropertyPresence => "RequiredPropertyPresence.ValidationRule",
            Self::NoUndescribedProperties => "NoUndescribedProperties.ValidationRule",
            Self::BaseValueKindMatchesString => "BaseValueKindMatchesString.ValidationRule",
            Self::BaseValueKindMatchesInteger => "BaseValueKindMatchesInteger.ValidationRule",
            Self::BaseValueKindMatchesBoolean => "BaseValueKindMatchesBoolean.ValidationRule",
            Self::BaseValueKindMatchesBytes => "BaseValueKindMatchesBytes.ValidationRule",
            Self::BaseValueKindMatchesEnum => "BaseValueKindMatchesEnum.ValidationRule",
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
