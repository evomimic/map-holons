use crate::descriptors::{accessor_helpers, Descriptor, TypeHeader};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::MapString;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

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

    /// Derives the canonical key for a configured constraint instance.
    ///
    /// This is deliberately a resolver only. Loader and Commit conformance do
    /// not invoke it yet, so it neither compares nor rewrites the holon's
    /// authored key.
    pub fn derive_constraint_instance_key(
        &self,
        constraint: &HolonReference,
    ) -> Result<MapString, HolonError> {
        if !self.extends_type_name("ConstraintInstanceRule")? {
            return Err(HolonError::WrongDescriptorKind {
                expected: "ConstraintInstanceRule".to_string(),
                found: self.header().type_name()?.to_string(),
                descriptor: self
                    .holon
                    .key()?
                    .map(|key| key.to_string())
                    .unwrap_or_else(|| "<keyless key rule descriptor>".to_string()),
            });
        }

        let constraint_name = accessor_helpers::require_string(constraint, "ConstraintName")?;
        let constraint_type = accessor_helpers::require_single_related(
            constraint,
            CoreRelationshipTypeName::DescribedBy,
        )?;
        let constraint_type_descriptor = KeyRuleDescriptor::from_holon(constraint_type.clone());
        let constraint_type_name = TypeHeader::new(&constraint_type).type_name()?;

        if !constraint_type_descriptor.extends_type_name("ConstraintType")? {
            return Err(HolonError::WrongDescriptorKind {
                expected: "ConstraintType".to_string(),
                found: constraint_type_name.to_string(),
                descriptor: constraint_type
                    .key()?
                    .map(|key| key.to_string())
                    .unwrap_or_else(|| "<keyless describing type>".to_string()),
            });
        }
        if TypeHeader::new(&constraint_type).is_abstract_type()? {
            return Err(HolonError::WrongDescriptorKind {
                expected: "concrete ConstraintType".to_string(),
                found: constraint_type_name.to_string(),
                descriptor: constraint_type
                    .key()?
                    .map(|key| key.to_string())
                    .unwrap_or_else(|| "<keyless describing type>".to_string()),
            });
        }

        Ok(MapString(format!("{}.{}", constraint_name, constraint_type_name)))
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
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapString;
    use type_names::CoreRelationshipTypeName;

    fn constraint_instance_rule(
        context: &std::sync::Arc<TransactionContext>,
    ) -> Result<KeyRuleDescriptor, HolonError> {
        let key_rule_type = new_descriptor_holon(context, "key-rule-type", "KeyRuleType", "Holon")?;
        let mut constraint_instance_rule = new_descriptor_holon(
            context,
            "constraint-instance-rule",
            "ConstraintInstanceRule",
            "Holon",
        )?;
        constraint_instance_rule
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![key_rule_type.into()])?;
        Ok(KeyRuleDescriptor::from_holon(constraint_instance_rule.into()))
    }

    fn concrete_constraint_type(
        context: &std::sync::Arc<TransactionContext>,
        key: &str,
        type_name: &str,
    ) -> Result<HolonReference, HolonError> {
        let constraint_type = new_descriptor_holon(
            context,
            &format!("{key}-constraint-type-root"),
            "ConstraintType",
            "Holon",
        )?;
        let mut concrete_type = new_descriptor_holon(context, key, type_name, "Holon")?;
        concrete_type
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![constraint_type.into()])?;
        Ok(concrete_type.into())
    }

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

    #[test]
    fn derives_constraint_instance_key_without_comparing_the_authored_key() -> Result<(), HolonError>
    {
        let context = build_context();
        let rule = constraint_instance_rule(&context)?;
        let constraint_type = concrete_constraint_type(
            &context,
            "string-length-constraint",
            "StringLengthConstraint",
        )?;
        let mut constraint =
            new_descriptor_holon(&context, "authored-but-not-derived", "Constraint", "Holon")?;
        constraint.with_property_value("ConstraintName", "Length16k")?;
        constraint.with_descriptor(constraint_type)?;

        assert_eq!(
            rule.derive_constraint_instance_key(&constraint.into())?,
            MapString("Length16k.StringLengthConstraint".to_string())
        );
        Ok(())
    }

    #[test]
    fn constraint_instance_key_requires_constraint_name() -> Result<(), HolonError> {
        let context = build_context();
        let rule = constraint_instance_rule(&context)?;
        let constraint_type = concrete_constraint_type(
            &context,
            "string-length-constraint",
            "StringLengthConstraint",
        )?;
        let mut constraint = new_descriptor_holon(&context, "constraint", "Constraint", "Holon")?;
        constraint.with_descriptor(constraint_type)?;

        assert!(matches!(
            rule.derive_constraint_instance_key(&constraint.into()),
            Err(HolonError::EmptyField(field)) if field == "ConstraintName"
        ));
        Ok(())
    }

    #[test]
    fn constraint_instance_key_rejects_non_constraint_describing_type() -> Result<(), HolonError> {
        let context = build_context();
        let rule = constraint_instance_rule(&context)?;
        let incompatible_type =
            new_descriptor_holon(&context, "incompatible-type", "NotAConstraint", "Holon")?;
        let mut constraint = new_descriptor_holon(&context, "constraint", "Constraint", "Holon")?;
        constraint.with_property_value("ConstraintName", "Length16k")?;
        constraint.with_descriptor(incompatible_type.into())?;

        assert!(matches!(
            rule.derive_constraint_instance_key(&constraint.into()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == "ConstraintType" && found == "NotAConstraint"
        ));
        Ok(())
    }

    #[test]
    fn constraint_instance_key_rejects_an_abstract_constraint_type() -> Result<(), HolonError> {
        let context = build_context();
        let rule = constraint_instance_rule(&context)?;
        let mut abstract_constraint_type =
            new_descriptor_holon(&context, "constraint-type", "ConstraintType", "Holon")?;
        abstract_constraint_type.with_property_value("IsAbstractType", true)?;
        let mut constraint = new_descriptor_holon(&context, "constraint", "Constraint", "Holon")?;
        constraint.with_property_value("ConstraintName", "Length16k")?;
        constraint.with_descriptor(abstract_constraint_type.into())?;

        assert!(matches!(
            rule.derive_constraint_instance_key(&constraint.into()),
            Err(HolonError::WrongDescriptorKind { expected, found, .. })
                if expected == "concrete ConstraintType" && found == "ConstraintType"
        ));
        Ok(())
    }

    #[test]
    fn constraint_instance_key_rejects_duplicate_describing_types() -> Result<(), HolonError> {
        let context = build_context();
        let rule = constraint_instance_rule(&context)?;
        let first_type = concrete_constraint_type(
            &context,
            "string-length-constraint",
            "StringLengthConstraint",
        )?;
        let second_type =
            concrete_constraint_type(&context, "bytes-length-constraint", "BytesLengthConstraint")?;
        let mut constraint = new_descriptor_holon(&context, "constraint", "Constraint", "Holon")?;
        constraint.with_property_value("ConstraintName", "Length16k")?;
        constraint.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![first_type, second_type],
        )?;

        assert!(matches!(
            rule.derive_constraint_instance_key(&constraint.into()),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "DescribedBy" && count == 2
        ));
        Ok(())
    }
}
