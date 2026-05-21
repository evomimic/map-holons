use crate::descriptors::accessor_helpers::descriptor_label;
use crate::descriptors::value_descriptor_subtypes::constraints::{
    resolve_integer_constraints, IntegerConstraintValidation,
};
use crate::descriptors::value_descriptor_subtypes::helpers::{
    require_supported_operator, supported_operators, supports_operator, type_name_is,
    unsupported_operator, value_kind_mismatch,
};
use crate::descriptors::{Descriptor, OperatorDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;

/// Semantic wrapper for integer value descriptors.
pub struct IntegerValueDescriptor {
    holon: HolonReference,
}

impl IntegerValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Validates that a runtime value is an integer and satisfies descriptor constraints.
    pub fn is_valid(&self, value: &BaseValue) -> Result<(), HolonError> {
        let integer_value = match value {
            BaseValue::IntegerValue(value) => value.0,
            other => return Err(value_kind_mismatch(&self.holon, "Integer", other)),
        };

        let label = descriptor_label(&self.holon);
        for constraint in resolve_integer_constraints(&self.holon)? {
            constraint.is_valid(integer_value, &label)?;
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

    /// Applies an afforded integer operator to two integer operands.
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
            BaseValue::IntegerValue(value) => value,
            other => return Err(value_kind_mismatch(&self.holon, "Integer", other)),
        };
        let rhs = match rhs {
            BaseValue::IntegerValue(value) => value,
            other => return Err(value_kind_mismatch(&self.holon, "Integer", other)),
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

impl From<HolonReference> for IntegerValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for IntegerValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<IntegerValueDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, core_value_type_name, new_descriptor_holon,
    };
    use crate::reference_layer::{TransientReference, WritableHolon};
    use base_types::{MapInteger, MapString};
    use type_names::{
        CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName,
    };

    fn integer_value(value: i64) -> BaseValue {
        BaseValue::IntegerValue(MapInteger(value))
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
            "integer-value",
            "IntegerValueType",
            "Value",
        )?);

        let descriptor = IntegerValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("IntegerValueType".to_string()));
        Ok(())
    }

    #[test]
    fn is_valid_accepts_integer_and_rejects_other_kinds() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        let descriptor = IntegerValueDescriptor::from_holon(holon.into());

        assert!(descriptor.is_valid(&integer_value(7)).is_ok());
        assert!(matches!(
            descriptor.is_valid(&BaseValue::StringValue(MapString("7".to_string()))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));
        Ok(())
    }

    #[test]
    fn is_valid_enforces_integer_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumValue),
            "Holon",
        )?;
        maximum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 10_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut maximum, &family)?;
        let mut value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        let descriptor = IntegerValueDescriptor::from_holon(value.into());

        assert!(descriptor.is_valid(&integer_value(7)).is_ok());
        assert!(matches!(
            descriptor.is_valid(&integer_value(4)),
            Err(HolonError::IntegerOutOfRange { value: 4, min: Some(5), .. })
        ));
        assert!(matches!(
            descriptor.is_valid(&integer_value(11)),
            Err(HolonError::IntegerOutOfRange { value: 11, max: Some(10), .. })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_enforces_inherited_integer_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut minimum, &family)?;
        let mut parent =
            new_descriptor_holon(&context, "parent-value", "ParentIntegerValueType", "Value")?;
        parent.add_related_holons(CoreRelationshipTypeName::Constraints, vec![minimum.into()])?;
        let mut child =
            new_descriptor_holon(&context, "child-value", "ChildIntegerValueType", "Value")?;
        add_extends(&mut child, &parent)?;

        let descriptor = IntegerValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.is_valid(&integer_value(4)),
            Err(HolonError::IntegerOutOfRange { value: 4, min: Some(5), .. })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_honors_exclusive_integer_bounds() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut minimum = new_descriptor_holon(
            &context,
            "exclusive-minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, false)?;
        add_extends(&mut minimum, &family)?;
        let mut maximum = new_descriptor_holon(
            &context,
            "exclusive-maximum",
            &core_holon_type_name(CoreHolonTypeName::MaximumValue),
            "Holon",
        )?;
        maximum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 10_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, false)?;
        add_extends(&mut maximum, &family)?;
        let mut value = new_descriptor_holon(
            &context,
            "integer-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        value.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![minimum.into(), maximum.into()],
        )?;

        let descriptor = IntegerValueDescriptor::from_holon(value.into());

        assert!(matches!(
            descriptor.is_valid(&integer_value(5)),
            Err(HolonError::IntegerOutOfRange { value: 5, min: Some(5), min_inclusive: false, .. })
        ));
        assert!(descriptor.is_valid(&integer_value(6)).is_ok());
        assert!(descriptor.is_valid(&integer_value(9)).is_ok());
        assert!(matches!(
            descriptor.is_valid(&integer_value(10)),
            Err(HolonError::IntegerOutOfRange {
                value: 10,
                max: Some(10),
                max_inclusive: false,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn is_valid_composes_inherited_and_local_integer_constraints() -> Result<(), HolonError> {
        let context = build_context();
        let family = new_descriptor_holon(
            &context,
            "integer-constraint-family",
            &core_holon_type_name(CoreHolonTypeName::IntegerValueConstraint),
            "Holon",
        )?;
        let mut inherited_minimum = new_descriptor_holon(
            &context,
            "inherited-minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        inherited_minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 5_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut inherited_minimum, &family)?;
        let mut local_minimum = new_descriptor_holon(
            &context,
            "local-minimum",
            &core_holon_type_name(CoreHolonTypeName::MinimumValue),
            "Holon",
        )?;
        local_minimum
            .with_property_value(CorePropertyTypeName::ConstraintIntegerValue, 10_i64)?
            .with_property_value(CorePropertyTypeName::ConstraintIsInclusive, true)?;
        add_extends(&mut local_minimum, &family)?;

        let mut parent = new_descriptor_holon(
            &context,
            "parent-value",
            &core_value_type_name(CoreValueTypeName::IntegerValueType),
            "Value",
        )?;
        parent.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![inherited_minimum.into()],
        )?;
        let mut child =
            new_descriptor_holon(&context, "child-value", "ConstrainedIntegerValueType", "Value")?;
        add_extends(&mut child, &parent)?;
        child.add_related_holons(
            CoreRelationshipTypeName::Constraints,
            vec![local_minimum.into()],
        )?;

        let descriptor = IntegerValueDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.is_valid(&integer_value(7)),
            Err(HolonError::IntegerOutOfRange { value: 7, min: Some(10), .. })
        ));
        assert!(descriptor.is_valid(&integer_value(10)).is_ok());
        Ok(())
    }

    #[test]
    fn supported_operators_and_supports_operator_use_affordances() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let not_afforded = new_descriptor_holon(&context, "contains", "ContainsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into(), less_than.clone().into()],
        )?;

        let descriptor = IntegerValueDescriptor::from_holon(value.into());
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
    fn apply_operator_executes_integer_comparisons() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into(), less_than.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals.into());
        let less_than = OperatorDescriptor::from_holon(less_than.into());
        let descriptor = IntegerValueDescriptor::from_holon(value.into());

        assert!(descriptor.apply_operator(&equals, &integer_value(3), &integer_value(3))?);
        assert!(!descriptor.apply_operator(&equals, &integer_value(3), &integer_value(4))?);
        assert!(descriptor.apply_operator(&less_than, &integer_value(2), &integer_value(5))?);
        Ok(())
    }

    #[test]
    fn apply_operator_reports_kind_mismatch_and_unsupported_operator() -> Result<(), HolonError> {
        let context = build_context();
        let contains = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "contains", "ContainsOperator", "Holon")?.into(),
        );
        let equals_holon = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals_holon.clone().into()],
        )?;

        let equals = OperatorDescriptor::from_holon(equals_holon.into());
        let descriptor = IntegerValueDescriptor::from_holon(value.into());

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::StringValue(MapString("3".to_string())),
                &integer_value(3),
            ),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));
        assert!(matches!(
            descriptor.apply_operator(&contains, &integer_value(3), &integer_value(3)),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "ContainsOperator" && value_type == "IntegerValueType"
        ));
        Ok(())
    }

    #[test]
    fn apply_operator_rejects_known_operator_when_not_afforded() -> Result<(), HolonError> {
        let context = build_context();
        let less_than = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?.into(),
        );
        let descriptor = IntegerValueDescriptor::from_holon(
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.apply_operator(&less_than, &integer_value(2), &integer_value(5)),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "LessThanOperator" && value_type == "IntegerValueType"
        ));
        Ok(())
    }
}
