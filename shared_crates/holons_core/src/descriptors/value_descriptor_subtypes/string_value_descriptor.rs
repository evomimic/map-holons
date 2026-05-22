use crate::descriptors::accessor_helpers::descriptor_label;
use crate::descriptors::value_descriptor_subtypes::constraints::{
    resolve_string_constraints, StringConstraintValidation,
};
use crate::descriptors::value_descriptor_subtypes::helpers::{
    require_supported_operator, supported_operators, supports_operator, type_name_is,
    unsupported_operator, value_kind_mismatch,
};
use crate::descriptors::{Descriptor, OperatorDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;

/// Semantic wrapper for string value descriptors.
pub struct StringValueDescriptor {
    holon: HolonReference,
}

impl StringValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Validates that a runtime value is a string and satisfies descriptor constraints.
    pub fn is_valid(&self, value: &BaseValue) -> Result<(), HolonError> {
        let string_value = match value {
            BaseValue::StringValue(value) => &value.0,
            other => return Err(value_kind_mismatch(&self.holon, "String", other)),
        };

        let label = descriptor_label(&self.holon);
        for constraint in resolve_string_constraints(&self.holon)? {
            constraint.is_valid(string_value, &label)?;
        }

        Ok(())
    }

    /// Returns operators afforded by this value descriptor across inheritance.
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        supported_operators(&self.holon)
    }

    /// Returns whether this descriptor affords the supplied operator.
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        supports_operator(&self.holon, op)
    }

    /// Applies an afforded string operator to two string operands.
    ///
    /// Operators must be declared through this descriptor's `AffordsOperator`
    /// relationships; otherwise execution returns `UnsupportedOperator`.
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        require_supported_operator(&self.holon, op)?;

        let lhs = match lhs {
            BaseValue::StringValue(value) => value,
            other => return Err(value_kind_mismatch(&self.holon, "String", other)),
        };
        let rhs = match rhs {
            BaseValue::StringValue(value) => value,
            other => return Err(value_kind_mismatch(&self.holon, "String", other)),
        };

        if type_name_is(op, "EqualsOperator")? {
            return Ok(lhs == rhs);
        }
        if type_name_is(op, "LessThanOperator")? {
            return Ok(lhs.0 < rhs.0);
        }
        unsupported_operator(&self.holon, op)
    }
}

impl From<HolonReference> for StringValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for StringValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<StringValueDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, core_value_type_name, new_descriptor_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use base_types::{MapInteger, MapString};
    use core_types::SchemaInvalidityKind;
    use type_names::{
        CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName,
    };

    fn string_value(value: &str) -> BaseValue {
        BaseValue::StringValue(MapString(value.to_string()))
    }

    fn add_extends(
        child: &mut TransientReference,
        parent: &TransientReference,
    ) -> Result<(), HolonError> {
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone().into()])?;
        Ok(())
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "string-value",
            "StringValueType",
            "Value",
        )?);

        let descriptor = StringValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("StringValueType".to_string()));
        Ok(())
    }

    #[test]
    fn is_valid_accepts_string_and_rejects_other_kinds() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        let descriptor = StringValueDescriptor::from_holon(holon.into());

        assert!(descriptor.is_valid(&string_value("alpha")).is_ok());
        assert!(matches!(
            descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(7))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "String" && found == "Integer"
        ));
        Ok(())
    }

    #[test]
    fn is_valid_enforces_string_length_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 2_i64)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumLength),
            "Holon",
        )?;
        maximum.with_property_value(CorePropertyTypeName::ConstraintLength, 4_i64)?;
        add_extends(&mut maximum, &family)?;
        let mut value = new_descriptor_holon(
            &context,
            "string-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        let descriptor = StringValueDescriptor::from_holon(value.into());

        assert!(descriptor.is_valid(&string_value("map")).is_ok());
        assert!(matches!(
            descriptor.is_valid(&string_value("m")),
            Err(HolonError::StringLengthOutOfRange { length: 1, min: Some(2), .. })
        ));
        assert!(matches!(
            descriptor.is_valid(&string_value("holon")),
            Err(HolonError::StringLengthOutOfRange { length: 5, max: Some(4), .. })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_propagates_descriptor_schema_invalidity() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        add_extends(&mut minimum, &family)?;
        let mut value = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        value.add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;

        let descriptor = StringValueDescriptor::from_holon(value.into());

        assert!(matches!(
            descriptor.is_valid(&string_value("map")),
            Err(HolonError::DescriptorSchemaInvalid {
                kind: SchemaInvalidityKind::MissingConstraintParameter,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_counts_unicode_scalar_values_for_length_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 1_i64)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumLength),
            "Holon",
        )?;
        maximum.with_property_value(CorePropertyTypeName::ConstraintLength, 1_i64)?;
        add_extends(&mut maximum, &family)?;
        let mut value = new_descriptor_holon(
            &context,
            "string-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        let descriptor = StringValueDescriptor::from_holon(value.into());

        assert!(descriptor.is_valid(&string_value("\u{e9}")).is_ok());
        Ok(())
    }

    #[test]
    fn is_valid_enforces_inherited_string_length_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 3_i64)?;
        add_extends(&mut minimum, &family)?;
        let mut parent = new_descriptor_holon(
            &context,
            "parent-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        parent.add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;
        let mut child =
            new_descriptor_holon(&context, "child-value", "ConstrainedStringValueType", "Value")?;
        add_extends(&mut child, &parent)?;

        let descriptor = StringValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.is_valid(&string_value("hi")),
            Err(HolonError::StringLengthOutOfRange { length: 2, min: Some(3), .. })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_composes_inherited_and_local_string_length_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "string-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::StringValueConstraint),
            "Holon",
        )?;
        let mut inherited_minimum = new_descriptor_holon(
            &context,
            "inherited-minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        inherited_minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 2_i64)?;
        add_extends(&mut inherited_minimum, &family)?;
        let mut local_minimum = new_descriptor_holon(
            &context,
            "local-minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumLength),
            "Holon",
        )?;
        local_minimum.with_property_value(CorePropertyTypeName::ConstraintLength, 4_i64)?;
        add_extends(&mut local_minimum, &family)?;

        let mut parent = new_descriptor_holon(
            &context,
            "parent-value",
            &core_value_type_name(CoreValueTypeName::StringValueType),
            "Value",
        )?;
        parent.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![inherited_minimum.into()],
        )?;
        let mut child =
            new_descriptor_holon(&context, "child-value", "ConstrainedStringValueType", "Value")?;
        add_extends(&mut child, &parent)?;
        child.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![local_minimum.into()],
        )?;

        let descriptor = StringValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.is_valid(&string_value("map")),
            Err(HolonError::StringLengthOutOfRange { length: 3, min: Some(4), .. })
        ));
        assert!(descriptor.is_valid(&string_value("maps")).is_ok());
        Ok(())
    }

    #[test]
    fn supported_operators_and_supports_operator_use_affordances() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let not_afforded = new_descriptor_holon(&context, "contains", "ContainsOperator", "Holon")?;
        let mut value = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into(), less_than.clone().into()],
        )?;

        let descriptor = StringValueDescriptor::from_holon(value.into());
        let names = descriptor
            .supported_operators()?
            .into_iter()
            .map(|op| op.operator_name().map(|name| name.0.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["EqualsOperator", "LessThanOperator"]);
        assert!(descriptor.supports_operator(&OperatorDescriptor::from_holon(equals.into()))?);
        assert!(
            !descriptor.supports_operator(&OperatorDescriptor::from_holon(not_afforded.into()))?
        );
        Ok(())
    }

    #[test]
    fn apply_operator_executes_string_comparisons() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into(), less_than.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals.into());
        let less_than = OperatorDescriptor::from_holon(less_than.into());
        let descriptor = StringValueDescriptor::from_holon(value.into());

        assert!(descriptor.apply_operator(&equals, &string_value("a"), &string_value("a"))?);
        assert!(!descriptor.apply_operator(&equals, &string_value("a"), &string_value("b"))?);
        assert!(descriptor.apply_operator(&less_than, &string_value("a"), &string_value("b"))?);
        Ok(())
    }

    #[test]
    fn apply_operator_reports_kind_mismatch_and_unsupported_operator() -> Result<(), HolonError> {
        let context = build_context();
        let contains = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "contains", "ContainsOperator", "Holon")?.into(),
        );
        let equals_holon = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals_holon.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals_holon.into());
        let descriptor = StringValueDescriptor::from_holon(value.into());

        assert!(matches!(
            descriptor.apply_operator(&equals, &BaseValue::IntegerValue(MapInteger(3)), &string_value("3")),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "String" && found == "Integer"
        ));
        assert!(matches!(
            descriptor.apply_operator(&contains, &string_value("a"), &string_value("a")),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "ContainsOperator" && value_type == "StringValueType"
        ));
        Ok(())
    }

    #[test]
    fn apply_operator_rejects_known_operator_when_not_afforded() -> Result<(), HolonError> {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = StringValueDescriptor::from_holon(
            new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.apply_operator(&equals, &string_value("a"), &string_value("a")),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "StringValueType"
        ));
        Ok(())
    }
}
