use crate::descriptors::{accessor_helpers, Descriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;

/// Runtime wrapper for key-rule descriptors.
///
/// Key-rule identity is intentionally read from the descriptor holon's own
/// `TypeName`/`Extends` chain so user-defined key-rule descriptors can
/// participate without adding key-rule variants to core type-name enums.
pub struct KeyRuleDescriptor {
    holon: HolonReference,
}

impl KeyRuleDescriptor {
    /// Wraps an already-resolved key-rule descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns true when this descriptor resolves to the canonical `NoneRule`.
    pub fn is_keyless(&self) -> Result<bool, HolonError> {
        self.extends_type_name("NoneRule")
    }

    /// Returns true when this descriptor is the abstract `KeyRuleType` or extends it.
    pub fn is_key_rule(&self) -> Result<bool, HolonError> {
        self.extends_type_name("KeyRuleType")
    }

    fn extends_type_name(&self, expected: &str) -> Result<bool, HolonError> {
        let expected_type_name = MapString(expected.to_string());

        match accessor_helpers::search_extends_chain(
            &self.holon,
            std::slice::from_ref(&expected_type_name),
            |type_name| (type_name == &expected_type_name).then_some(()),
        ) {
            Ok(()) => Ok(true),
            Err(HolonError::WrongDescriptorKind { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl From<HolonReference> for KeyRuleDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for KeyRuleDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<KeyRuleDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn is_keyless_matches_none_rule_directly() -> Result<(), HolonError> {
        let context = build_context();
        let none_rule = new_descriptor_holon(&context, "none-rule", "NoneRule", "Holon")?;

        let descriptor = KeyRuleDescriptor::from_holon(none_rule.into());

        assert!(descriptor.is_keyless()?);
        Ok(())
    }

    #[test]
    fn is_keyless_matches_none_rule_through_extends() -> Result<(), HolonError> {
        let context = build_context();
        let none_rule = new_descriptor_holon(&context, "none-rule-parent", "NoneRule", "Holon")?;
        let mut custom_none =
            new_descriptor_holon(&context, "custom-none-rule", "CustomNoneRule", "Holon")?;
        custom_none
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![none_rule.into()])?;

        let descriptor = KeyRuleDescriptor::from_holon(custom_none.into());

        assert!(descriptor.is_keyless()?);
        Ok(())
    }

    #[test]
    fn is_keyless_returns_false_for_other_key_rule_descriptors() -> Result<(), HolonError> {
        let context = build_context();
        let key_rule_type =
            new_descriptor_holon(&context, "key-rule-type", "KeyRuleType", "Holon")?;
        let mut type_name_rule =
            new_descriptor_holon(&context, "type-name-rule", "TypeNameRule", "Holon")?;
        type_name_rule
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![key_rule_type.into()])?;

        let descriptor = KeyRuleDescriptor::from_holon(type_name_rule.into());

        assert!(!descriptor.is_keyless()?);
        Ok(())
    }

    #[test]
    fn is_key_rule_classifies_concrete_and_invalid_rules() -> Result<(), HolonError> {
        let context = build_context();
        let key_rule_type =
            new_descriptor_holon(&context, "classification-key-rule-type", "KeyRuleType", "Holon")?;
        let mut type_name_rule = new_descriptor_holon(
            &context,
            "classification-type-name-rule",
            "TypeNameRule",
            "Holon",
        )?;
        let invalid_rule =
            new_descriptor_holon(&context, "classification-invalid-rule", "NotAKeyRule", "Holon")?;

        type_name_rule
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![key_rule_type.into()])?;

        assert!(KeyRuleDescriptor::from_holon(type_name_rule.into()).is_key_rule()?);
        assert!(!KeyRuleDescriptor::from_holon(invalid_rule.into()).is_key_rule()?);

        Ok(())
    }
}
