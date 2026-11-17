use super::{EnumMembers, EnumMembership};
use crate::descriptors::value_descriptor_subtypes::helpers::{
    single_operand, supported_operators, supports_operator, unsupported_operator,
    value_kind_mismatch, value_type_name,
};
use crate::descriptors::{
    Descriptor, OperatorDescriptor, OperatorKind, ResolvedOperator, TypeHeader, ValueDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;

/// Semantic wrapper for enum value descriptors.
pub struct EnumValueDescriptor {
    holon: HolonReference,
}

impl EnumValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Validates that a runtime enum value is one of this descriptor's variants.
    pub fn is_valid(&self, value: &BaseValue) -> Result<(), HolonError> {
        let variant = match value {
            BaseValue::EnumValue(value) => value.0 .0.clone(),
            other => return Err(value_kind_mismatch(&self.holon, "Enum", other)),
        };

        let membership = EnumMembers::resolve(self)?.membership(&variant);
        if matches!(membership, EnumMembership::Member(_)) {
            return Ok(());
        }
        let detail = match membership {
            EnumMembership::Ambiguous(identities) => format!(
                "ambiguous token claimed by {} distinct concrete variant definitions",
                identities.len()
            ),
            _ => "no concrete variant declares this token".into(),
        };

        Err(HolonError::EnumVariantNotInSchema {
            variant,
            value_type: value_type_name(&self.holon)?,
            descriptor: crate::descriptors::accessor_helpers::descriptor_label(&self.holon),
            detail,
        })
    }

    /// Returns operators afforded by this value descriptor across inheritance.
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        supported_operators(&self.holon)
    }

    /// Returns whether this descriptor affords the supplied operator.
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        supports_operator(&self.holon, op)
    }

    /// Applies an afforded enum operator to two enum operands.
    ///
    /// Operators must be declared through this descriptor's `AffordsOperator`
    /// relationships; otherwise execution returns `UnsupportedOperator`. Execution
    /// shares the resolved operator contract: it compares variant tokens without
    /// checking membership, so callers needing conformance call [`Self::is_valid`].
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        ValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
    }

    /// Executes a resolved enum operator by comparing variant tokens.
    ///
    /// Only the enum representation is checked. Variant membership belongs to
    /// authored-literal and data validation, so invocation reads no variant
    /// declarations: an undeclared token is simply unequal to every other token.
    pub(crate) fn evaluate(
        &self,
        op: &ResolvedOperator,
        subject: &BaseValue,
        operands: &[BaseValue],
    ) -> Result<bool, HolonError> {
        if op.kind() != OperatorKind::Equals {
            return unsupported_operator(&self.holon, op.operator());
        }
        match (subject, single_operand(op, operands)?) {
            (BaseValue::EnumValue(lhs), BaseValue::EnumValue(rhs)) => Ok(lhs == rhs),
            (BaseValue::EnumValue(_), other) | (other, _) => {
                Err(value_kind_mismatch(&self.holon, "Enum", other))
            }
        }
    }
}

impl From<HolonReference> for EnumValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for EnumValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<EnumValueDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_operator, new_descriptor_holon, new_kind_descriptor,
    };
    use crate::reference_layer::WritableHolon;
    use base_types::{MapEnumValue, MapInteger, MapString};
    use type_names::{CoreOperatorTypeName, CoreRelationshipTypeName};

    fn enum_value(value: &str) -> BaseValue {
        BaseValue::EnumValue(MapEnumValue(MapString(value.to_string())))
    }

    /// A `ColorValueType` descriptor whose enum value kind comes from an anchored parent.
    fn color_value_type(
        context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
    ) -> Result<crate::reference_layer::TransientReference, HolonError> {
        let anchor: HolonReference = context
            .mutation()
            .stage_new_holon(new_kind_descriptor(context, "EnumValueType", None, true)?)?
            .into();
        let mut value = new_descriptor_holon(context, "color", "ColorValueType", "Value")?;
        value.with_property_value(
            type_names::CorePropertyTypeName::DefinesInstanceTypeKind,
            false,
        )?;
        value.add_related_holons(CoreRelationshipTypeName::Extends, vec![anchor])?;
        Ok(value)
    }

    fn enum_descriptor_with_variants() -> Result<EnumValueDescriptor, HolonError> {
        let context = build_context();
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = color_value_type(&context)?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        Ok(EnumValueDescriptor::from_holon(value.into()))
    }

    fn membership_fixture(rows: &[(&str, &str, bool)]) -> Result<EnumValueDescriptor, HolonError> {
        let context = build_context();
        let mut value = color_value_type(&context)?;
        let mut targets = Vec::new();
        for &(key, token, is_abstract) in rows {
            let mut member = new_descriptor_holon(&context, key, token, "EnumVariant")?;
            member.with_property_value("IsAbstractType", is_abstract)?;
            member.with_property_value("DisplayName", "Presentation")?;
            targets.push(member.into());
        }
        value.add_related_holons(CoreRelationshipTypeName::Variants, targets)?;
        Ok(EnumValueDescriptor::from_holon(value.into()))
    }

    #[test]
    fn membership_accepts_only_exact_tokens() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[
            ("Red.ColorValueType", "Red", false),
            ("accent", "é", false),
            ("key-token", "key-token", false),
            ("display-token", "Presentation", false),
        ])?;
        for token in ["Red", "é", "key-token", "Presentation"] {
            descriptor.is_valid(&enum_value(token))?;
        }
        for token in ["red", "Red.ColorValueType", "accent", "e\u{301}", "display-token"] {
            assert!(
                matches!(
                    descriptor.is_valid(&enum_value(token)),
                    Err(HolonError::EnumVariantNotInSchema { .. })
                ),
                "{token}"
            );
        }
        Ok(())
    }

    #[test]
    fn display_name_does_not_substitute_for_a_token() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[("Red.ColorValueType", "Red", false)])?;
        assert!(matches!(
            descriptor.is_valid(&enum_value("Presentation")),
            Err(HolonError::EnumVariantNotInSchema { .. })
        ));
        Ok(())
    }

    #[test]
    fn duplicate_tokens_retain_distinct_claimants_and_provenance() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[("red", "Red", false), ("other-red", "Red", false)])?;
        let members = EnumMembers::resolve(&descriptor)?;
        let identities: Vec<_> =
            members.members().iter().map(|member| member.identity.clone()).collect();
        assert_eq!(members.duplicate_tokens()["Red"], identities);
        assert_eq!(members.membership("Red"), EnumMembership::Ambiguous(identities));
        let owner = crate::DefinitionIdentity::for_optional_context(
            descriptor.holon(),
            descriptor.holon().transaction_context().as_ref(),
        )?;
        assert!(members.members().iter().all(|member| member.declared_on == owner));
        match descriptor.is_valid(&enum_value("Red")).unwrap_err() {
            HolonError::EnumVariantNotInSchema { descriptor: label, detail, .. } => {
                assert_eq!(
                    label,
                    crate::descriptors::accessor_helpers::descriptor_label(descriptor.holon())
                );
                assert!(detail.contains("ambiguous"));
                assert!(detail.contains("2 distinct"));
            }
            error => panic!("unexpected error: {error}"),
        }
        Ok(())
    }

    #[test]
    fn abstract_targets_never_supply_or_duplicate_tokens() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[
            ("anchor", "Anchor", true),
            ("red", "Red", false),
            ("abstract-red", "Red", true),
        ])?;
        let members = EnumMembers::resolve(&descriptor)?;
        assert_eq!(members.abstract_members().count(), 2);
        assert!(members.duplicate_tokens().is_empty());
        assert_eq!(members.membership("Anchor"), EnumMembership::Undeclared);
        assert!(matches!(
            descriptor.is_valid(&enum_value("Anchor")),
            Err(HolonError::EnumVariantNotInSchema { detail, .. })
                if detail.contains("no concrete variant declares")
        ));
        descriptor.is_valid(&enum_value("Red"))?;
        Ok(())
    }

    #[test]
    fn repeated_identity_is_one_token_claimant() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[("red", "Red", false)])?;
        let members = EnumMembers::resolve(&descriptor)?;
        let repeated =
            EnumMembers::new(vec![members.members()[0].clone(), members.members()[0].clone()]);
        assert!(repeated.duplicate_tokens().is_empty());
        assert!(matches!(repeated.membership("Red"), EnumMembership::Member(_)));
        Ok(())
    }

    #[test]
    fn sub_enum_does_not_inherit_parent_tokens() -> Result<(), HolonError> {
        let descriptor = membership_fixture(&[("red", "Red", false)])?;
        let context = descriptor.holon().transaction_context().unwrap();
        let mut child = new_descriptor_holon(&context, "child", "Child", "Value")?;
        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![descriptor.holon().clone()],
        )?;
        assert_eq!(
            EnumMembers::resolve(&EnumValueDescriptor::from_holon(child.into()))?.membership("Red"),
            EnumMembership::Undeclared
        );
        Ok(())
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "enum-value",
            "EnumValueType",
            "Value",
        )?);

        let descriptor = EnumValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("EnumValueType".to_string()));
        Ok(())
    }

    #[test]
    fn is_valid_accepts_declared_variant_and_rejects_other_kinds() -> Result<(), HolonError> {
        let descriptor = enum_descriptor_with_variants()?;

        assert!(descriptor.is_valid(&enum_value("Red")).is_ok());
        assert!(matches!(
            descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(7))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Enum" && found == "Integer"
        ));
        Ok(())
    }

    #[test]
    fn is_valid_rejects_variant_not_declared_by_schema() -> Result<(), HolonError> {
        let descriptor = enum_descriptor_with_variants()?;

        assert!(matches!(
            descriptor.is_valid(&enum_value("Green")),
            Err(HolonError::EnumVariantNotInSchema { variant, value_type, .. })
                if variant == "Green" && value_type == "ColorValueType"
        ));
        Ok(())
    }

    #[test]
    fn is_valid_rejects_variant_when_no_variants_are_declared() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = EnumValueDescriptor::from_holon(
            new_descriptor_holon(&context, "empty-enum", "EmptyEnumValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.is_valid(&enum_value("AnyVariant")),
            Err(HolonError::EnumVariantNotInSchema { variant, value_type, .. })
                if variant == "AnyVariant" && value_type == "EmptyEnumValueType"
        ));
        Ok(())
    }

    #[test]
    fn supported_operators_and_supports_operator_use_affordances() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let not_afforded =
            new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value = new_descriptor_holon(&context, "enum-value", "EnumValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = EnumValueDescriptor::from_holon(value.into());
        let names = descriptor
            .supported_operators()?
            .into_iter()
            .map(|op| op.operator_name().map(|name| name.0.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["EqualsOperator"]);
        assert!(descriptor.supports_operator(&OperatorDescriptor::from_holon(equals.into()))?);
        assert!(
            !descriptor.supports_operator(&OperatorDescriptor::from_holon(not_afforded.into()))?
        );
        Ok(())
    }

    #[test]
    fn apply_operator_executes_enum_equality() -> Result<(), HolonError> {
        let context = build_context();
        let equals = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None)?;
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = color_value_type(&context)?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals.into());
        let descriptor = EnumValueDescriptor::from_holon(value.into());
        assert!(descriptor.apply_operator(&equals, &enum_value("Red"), &enum_value("Red"))?);
        assert!(!descriptor.apply_operator(&equals, &enum_value("Red"), &enum_value("Blue"))?);
        Ok(())
    }

    #[test]
    fn apply_operator_compares_undeclared_tokens_without_membership_checks(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let equals = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None)?;
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = color_value_type(&context)?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals.into());
        let descriptor = EnumValueDescriptor::from_holon(value.into());
        assert!(!descriptor.apply_operator(&equals, &enum_value("Green"), &enum_value("Red"))?);
        assert!(descriptor.apply_operator(&equals, &enum_value("Green"), &enum_value("Green"))?);
        // Conformance stays available as an explicit check.
        assert!(matches!(
            descriptor.is_valid(&enum_value("Green")),
            Err(HolonError::EnumVariantNotInSchema { variant, .. }) if variant == "Green"
        ));
        Ok(())
    }

    #[test]
    fn apply_operator_reports_unsupported_operator() -> Result<(), HolonError> {
        let context = build_context();
        let equals_holon = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None)?;
        let less_than = OperatorDescriptor::from_holon(
            core_operator(&context, CoreOperatorTypeName::LessThanOperator, None)?.into(),
        );
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = color_value_type(&context)?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals_holon.clone().into()],
        )?;

        let descriptor = EnumValueDescriptor::from_holon(value.into());
        assert!(matches!(
            descriptor.apply_operator(&less_than, &enum_value("Red"), &enum_value("Blue")),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "LessThanOperator" && value_type == "ColorValueType"
        ));
        Ok(())
    }

    #[test]
    fn apply_operator_reports_kind_mismatch_for_non_enum_operand() -> Result<(), HolonError> {
        let context = build_context();
        let equals = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None)?;
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = color_value_type(&context)?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals.into());
        let descriptor = EnumValueDescriptor::from_holon(value.into());
        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(1)),
                &enum_value("Red"),
            ),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Enum" && found == "Integer"
        ));
        Ok(())
    }

    #[test]
    fn apply_operator_rejects_known_operator_when_not_afforded() -> Result<(), HolonError> {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            core_operator(&context, CoreOperatorTypeName::EqualsOperator, None)?.into(),
        );
        let descriptor = enum_descriptor_with_variants()?;

        assert!(matches!(
            descriptor.apply_operator(&equals, &enum_value("Red"), &enum_value("Red")),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "ColorValueType"
        ));
        Ok(())
    }
}
