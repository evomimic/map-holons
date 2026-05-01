use std::collections::HashSet;

use crate::descriptors::inheritance::flatten_related_members;
use crate::descriptors::value_descriptor_subtypes::{
    supported_operators, supports_operator, type_name_is, unsupported_operator,
    value_kind_mismatch, value_type_name,
};
use crate::descriptors::{Descriptor, OperatorDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

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

        if self.declared_variants()?.contains(&variant) {
            return Ok(());
        }

        Err(HolonError::EnumVariantNotInSchema {
            variant,
            value_type: value_type_name(&self.holon)?,
            descriptor: crate::descriptors::accessor_helpers::descriptor_label(&self.holon),
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

    /// Applies an enum operator to two enum operands.
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        if !type_name_is(op, "EqualsOperator")? {
            return unsupported_operator(&self.holon, op);
        }

        self.is_valid(lhs)?;
        self.is_valid(rhs)?;

        match (lhs, rhs) {
            (BaseValue::EnumValue(lhs), BaseValue::EnumValue(rhs)) => Ok(lhs == rhs),
            _ => unreachable!("is_valid guarantees enum operands"),
        }
    }

    fn declared_variants(&self) -> Result<HashSet<String>, HolonError> {
        let mut variants = HashSet::new();
        for variant in flatten_related_members(&self.holon, CoreRelationshipTypeName::Variants)? {
            variants.insert(TypeHeader::new(&variant).type_name()?.to_string());
        }
        Ok(variants)
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
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::{MapEnumValue, MapInteger, MapString};

    fn enum_value(value: &str) -> BaseValue {
        BaseValue::EnumValue(MapEnumValue(MapString(value.to_string())))
    }

    fn enum_descriptor_with_variants() -> Result<EnumValueDescriptor, HolonError> {
        let context = build_context();
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut value = new_descriptor_holon(&context, "color", "ColorValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        Ok(EnumValueDescriptor::from_holon(value.into()))
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
            .map(|op| op.type_name().map(|name| name.to_string()))
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
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = enum_descriptor_with_variants()?;

        assert!(descriptor.apply_operator(&equals, &enum_value("Red"), &enum_value("Red"))?);
        assert!(!descriptor.apply_operator(&equals, &enum_value("Red"), &enum_value("Blue"))?);
        Ok(())
    }

    #[test]
    fn apply_operator_reports_invalid_variant_and_unsupported_operator() -> Result<(), HolonError> {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let less_than = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?.into(),
        );
        let descriptor = enum_descriptor_with_variants()?;

        assert!(matches!(
            descriptor.apply_operator(&equals, &enum_value("Green"), &enum_value("Red")),
            Err(HolonError::EnumVariantNotInSchema { variant, .. }) if variant == "Green"
        ));
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
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = enum_descriptor_with_variants()?;

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
}
