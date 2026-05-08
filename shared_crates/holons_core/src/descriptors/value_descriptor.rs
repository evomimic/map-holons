use crate::descriptors::inheritance::walk_extends_chain;
use crate::descriptors::value_descriptor_subtypes::helpers::{
    supported_operators as collect_supported_operators,
    supports_operator as descriptor_supports_operator,
    unsupported_operator as descriptor_unsupported_operator,
    value_kind_mismatch as descriptor_value_kind_mismatch,
};
use crate::descriptors::{
    accessor_helpers, Descriptor, EnumValueDescriptor, IntegerValueDescriptor, OperatorDescriptor,
    StringValueDescriptor, TypeHeader, ValueArrayDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;

/// Runtime wrapper for value-type descriptors.
///
/// `ValueDescriptor` is the public semantic dispatch point for value validation
/// and operator execution. Subtype wrappers own the local behavior for each
/// value kind, while this wrapper resolves the kind through the descriptor
/// inheritance chain and enforces descriptor-level affordances.
pub struct ValueDescriptor {
    holon: HolonReference,
}

impl ValueDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Validates a runtime value against this descriptor's semantic value kind.
    pub fn is_valid(&self, value: &BaseValue) -> Result<(), HolonError> {
        match self.value_kind()? {
            ValueKind::Integer => {
                IntegerValueDescriptor::from_holon(self.holon.clone()).is_valid(value)
            }
            ValueKind::String => {
                StringValueDescriptor::from_holon(self.holon.clone()).is_valid(value)
            }
            ValueKind::Boolean => self.validate_boolean(value),
            ValueKind::Enum => EnumValueDescriptor::from_holon(self.holon.clone()).is_valid(value),
            ValueKind::Array => Err(self.value_kind_mismatch("Array", value)),
            ValueKind::Other(found) => Err(self.wrong_value_kind(found)),
        }
    }

    /// Returns operators afforded by this descriptor across its inheritance chain.
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        collect_supported_operators(&self.holon)
    }

    /// Returns whether this descriptor affords the supplied operator.
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        descriptor_supports_operator(&self.holon, op)
    }

    /// Applies an afforded operator to runtime operands using this descriptor's value kind.
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        let value_kind = self.value_kind()?;
        if let ValueKind::Other(found) = value_kind {
            return Err(self.wrong_value_kind(found));
        }

        if !self.supports_operator(op)? {
            return self.unsupported_operator(op);
        }

        match value_kind {
            ValueKind::Integer => {
                IntegerValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueKind::String => {
                StringValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueKind::Boolean => self.apply_boolean_operator(op, lhs, rhs),
            ValueKind::Enum => {
                EnumValueDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueKind::Array => {
                // Array execution is explicitly deferred; arrays may expose
                // affordances structurally before they have runtime semantics.
                ValueArrayDescriptor::from_holon(self.holon.clone()).apply_operator(op, lhs, rhs)
            }
            ValueKind::Other(_) => unreachable!("Other returns before affordance checks"),
        }
    }

    fn value_kind(&self) -> Result<ValueKind, HolonError> {
        let mut first_type_name = None;

        for ancestor in walk_extends_chain(&self.holon) {
            let ancestor = ancestor?;
            let type_name = TypeHeader::new(&ancestor).type_name()?;
            if first_type_name.is_none() {
                first_type_name = Some(type_name.to_string());
            }

            match type_name.0.as_str() {
                "IntegerValueType" => return Ok(ValueKind::Integer),
                "StringValueType" => return Ok(ValueKind::String),
                "BooleanValueType" => return Ok(ValueKind::Boolean),
                "EnumValueType" => return Ok(ValueKind::Enum),
                "ValueArrayValueType" => return Ok(ValueKind::Array),
                _ => {}
            }
        }

        Ok(ValueKind::Other(first_type_name.unwrap_or_else(|| "unknown".to_string())))
    }

    fn validate_boolean(&self, value: &BaseValue) -> Result<(), HolonError> {
        match value {
            BaseValue::BooleanValue(_) => Ok(()),
            other => Err(self.value_kind_mismatch("Boolean", other)),
        }
    }

    fn apply_boolean_operator(
        &self,
        op: &OperatorDescriptor,
        lhs: &BaseValue,
        rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        if op.type_name()?.0 != "EqualsOperator" {
            return self.unsupported_operator(op);
        }

        let lhs = match lhs {
            BaseValue::BooleanValue(value) => value,
            other => return Err(self.value_kind_mismatch("Boolean", other)),
        };
        let rhs = match rhs {
            BaseValue::BooleanValue(value) => value,
            other => return Err(self.value_kind_mismatch("Boolean", other)),
        };
        Ok(lhs == rhs)
    }

    fn unsupported_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        descriptor_unsupported_operator(&self.holon, op)
    }

    fn value_kind_mismatch(&self, expected: &str, found: &BaseValue) -> HolonError {
        descriptor_value_kind_mismatch(&self.holon, expected, found)
    }

    fn wrong_value_kind(&self, found: String) -> HolonError {
        HolonError::WrongDescriptorKind {
            expected: "IntegerValueType, StringValueType, BooleanValueType, EnumValueType, or ValueArrayValueType".to_string(),
            found,
            descriptor: accessor_helpers::descriptor_label(&self.holon),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ValueKind {
    Integer,
    String,
    Boolean,
    Enum,
    Array,
    Other(String),
}

impl From<HolonReference> for ValueDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for ValueDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<ValueDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::{MapBoolean, MapEnumValue, MapInteger, MapString};
    use core_types::HolonError;
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "value-descriptor",
            "StringValueType",
            "Value",
        )?);

        let descriptor = ValueDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("StringValueType".to_string()));

        Ok(())
    }

    #[test]
    fn is_valid_routes_by_value_kind() -> Result<(), HolonError> {
        let context = build_context();
        let integer = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?.into(),
        );

        assert!(integer.is_valid(&BaseValue::IntegerValue(MapInteger(42))).is_ok());
        assert!(matches!(
            integer.is_valid(&BaseValue::StringValue(MapString("42".to_string()))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Integer" && found == "String"
        ));

        Ok(())
    }

    #[test]
    fn is_valid_handles_boolean_inline() -> Result<(), HolonError> {
        let context = build_context();
        let boolean = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?.into(),
        );

        assert!(boolean.is_valid(&BaseValue::BooleanValue(MapBoolean(true))).is_ok());
        assert!(matches!(
            boolean.is_valid(&BaseValue::IntegerValue(MapInteger(1))),
            Err(HolonError::ValueKindMismatch { expected, found, .. })
                if expected == "Boolean" && found == "Integer"
        ));

        Ok(())
    }

    #[test]
    fn is_valid_resolves_kind_through_extends_chain() -> Result<(), HolonError> {
        let context = build_context();
        let parent = new_descriptor_holon(&context, "integer-parent", "IntegerValueType", "Value")?;
        let mut child =
            new_descriptor_holon(&context, "integer-child", "CustomIntegerValueType", "Value")?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;

        let descriptor = ValueDescriptor::from_holon(child.into());

        assert!(descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(42))).is_ok());

        Ok(())
    }

    #[test]
    fn is_valid_reports_wrong_descriptor_kind_for_unknown_value_kind() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "unknown-value", "CustomValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.is_valid(&BaseValue::IntegerValue(MapInteger(1))),
            Err(HolonError::WrongDescriptorKind { found, .. }) if found == "CustomValueType"
        ));

        Ok(())
    }

    #[test]
    fn supported_operators_flattens_across_extends() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut parent =
            new_descriptor_holon(&context, "integer-parent", "IntegerValueType", "Value")?;
        let mut child =
            new_descriptor_holon(&context, "integer-child", "CustomIntegerValueType", "Value")?;
        parent.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;
        child.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;

        let descriptor = ValueDescriptor::from_holon(child.into());
        let names = descriptor
            .supported_operators()?
            .into_iter()
            .map(|op| op.type_name().map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["EqualsOperator"]);

        Ok(())
    }

    #[test]
    fn supports_operator_reports_membership() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());

        assert!(descriptor.supports_operator(&OperatorDescriptor::from_holon(equals.into()))?);
        assert!(!descriptor.supports_operator(&OperatorDescriptor::from_holon(less_than.into()))?);

        Ok(())
    }

    #[test]
    fn apply_operator_executes_afforded_integer_operator() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(descriptor.apply_operator(
            &equals,
            &BaseValue::IntegerValue(MapInteger(3)),
            &BaseValue::IntegerValue(MapInteger(3)),
        )?);

        Ok(())
    }

    #[test]
    fn apply_operator_returns_unsupported_when_operator_is_not_afforded() -> Result<(), HolonError>
    {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = ValueDescriptor::from_holon(
            new_descriptor_holon(&context, "integer-value", "IntegerValueType", "Value")?.into(),
        );

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(3)),
                &BaseValue::IntegerValue(MapInteger(3)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "IntegerValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_handles_boolean_equals_inline() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(descriptor.apply_operator(
            &equals,
            &BaseValue::BooleanValue(MapBoolean(true)),
            &BaseValue::BooleanValue(MapBoolean(true)),
        )?);

        Ok(())
    }

    #[test]
    fn apply_operator_reports_wrong_descriptor_kind_for_unknown_value_kind(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "unknown-value", "CustomValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(3)),
                &BaseValue::IntegerValue(MapInteger(3)),
            ),
            Err(HolonError::WrongDescriptorKind { found, .. }) if found == "CustomValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_reports_unsupported_for_afforded_boolean_non_equals_operator(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let less_than = new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut value =
            new_descriptor_holon(&context, "boolean-value", "BooleanValueType", "Value")?;
        value.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![less_than.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(value.into());
        let less_than = OperatorDescriptor::from_holon(less_than.into());

        assert!(matches!(
            descriptor.apply_operator(
                &less_than,
                &BaseValue::BooleanValue(MapBoolean(false)),
                &BaseValue::BooleanValue(MapBoolean(true)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "LessThanOperator" && value_type == "BooleanValueType"
        ));

        Ok(())
    }

    #[test]
    fn apply_operator_dispatches_enum_values() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let parent = new_descriptor_holon(&context, "enum-parent", "EnumValueType", "Value")?;
        let red = new_descriptor_holon(&context, "red", "Red", "EnumVariant")?;
        let blue = new_descriptor_holon(&context, "blue", "Blue", "EnumVariant")?;
        let mut color = new_descriptor_holon(&context, "color", "ColorValueType", "Value")?;
        color.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
        color.add_related_holons(
            CoreRelationshipTypeName::Variants,
            vec![red.into(), blue.into()],
        )?;
        color.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(color.into());
        let equals = OperatorDescriptor::from_holon(equals.into());
        let red = BaseValue::EnumValue(MapEnumValue(MapString("Red".to_string())));
        let blue = BaseValue::EnumValue(MapEnumValue(MapString("Blue".to_string())));

        assert!(descriptor.is_valid(&red).is_ok());
        assert!(descriptor.apply_operator(&equals, &red, &red)?);
        assert!(!descriptor.apply_operator(&equals, &red, &blue)?);

        Ok(())
    }

    #[test]
    fn apply_operator_dispatches_array_to_deferred_execution() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let mut array = new_descriptor_holon(&context, "array", "ValueArrayValueType", "Value")?;
        array.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueDescriptor::from_holon(array.into());
        let equals = OperatorDescriptor::from_holon(equals.into());

        assert!(matches!(
            descriptor.apply_operator(
                &equals,
                &BaseValue::IntegerValue(MapInteger(1)),
                &BaseValue::IntegerValue(MapInteger(1)),
            ),
            Err(HolonError::UnsupportedOperator { operator, value_type, .. })
                if operator == "EqualsOperator" && value_type == "ValueArrayValueType"
        ));

        Ok(())
    }
}
