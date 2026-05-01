use crate::descriptors::value_descriptor_subtypes::{
    supported_operators, supports_operator, unsupported_operator,
};
use crate::descriptors::{
    accessor_helpers, Descriptor, OperatorDescriptor, TypeHeader, ValueDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::BaseValue;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Structural wrapper for value-array descriptors.
///
/// Array validation and operator execution are intentionally deferred; this
/// phase only exposes the element value type and inherited affordances.
pub struct ValueArrayDescriptor {
    holon: HolonReference,
}

impl ValueArrayDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the required element value descriptor.
    pub fn element_value_type(&self) -> Result<ValueDescriptor, HolonError> {
        Ok(ValueDescriptor::from_holon(accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::ElementValueType,
        )?))
    }

    /// Returns operators afforded by this value descriptor across inheritance.
    pub fn supported_operators(&self) -> Result<Vec<OperatorDescriptor>, HolonError> {
        supported_operators(&self.holon)
    }

    /// Returns whether this descriptor affords the supplied operator.
    pub fn supports_operator(&self, op: &OperatorDescriptor) -> Result<bool, HolonError> {
        supports_operator(&self.holon, op)
    }

    /// Array operator execution is not implemented in this phase.
    pub fn apply_operator(
        &self,
        op: &OperatorDescriptor,
        _lhs: &BaseValue,
        _rhs: &BaseValue,
    ) -> Result<bool, HolonError> {
        unsupported_operator(&self.holon, op)
    }
}

impl From<HolonReference> for ValueArrayDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for ValueArrayDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<ValueArrayDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::{MapInteger, MapString};

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "array-value",
            "ValueArrayValueType",
            "Value",
        )?);

        let descriptor = ValueArrayDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("ValueArrayValueType".to_string()));
        Ok(())
    }

    #[test]
    fn element_value_type_returns_linked_target() -> Result<(), HolonError> {
        let context = build_context();
        let element = new_descriptor_holon(&context, "string-value", "StringValueType", "Value")?;
        let mut array =
            new_descriptor_holon(&context, "string-array", "ValueArrayValueType", "Value")?;
        array
            .add_related_holons(CoreRelationshipTypeName::ElementValueType, vec![element.into()])?;

        let descriptor = ValueArrayDescriptor::from_holon(array.into());

        assert_eq!(
            descriptor.element_value_type()?.header().type_name()?,
            MapString("StringValueType".to_string())
        );
        Ok(())
    }

    #[test]
    fn element_value_type_errors_when_missing() -> Result<(), HolonError> {
        let context = build_context();
        let array = new_descriptor_holon(&context, "array", "ValueArrayValueType", "Value")?;
        let descriptor = ValueArrayDescriptor::from_holon(array.into());

        assert!(matches!(
            descriptor.element_value_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "ElementValueType"
        ));
        Ok(())
    }

    #[test]
    fn supported_operators_and_supports_operator_use_affordances() -> Result<(), HolonError> {
        let context = build_context();
        let equals = new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?;
        let not_afforded =
            new_descriptor_holon(&context, "less-than", "LessThanOperator", "Holon")?;
        let mut array = new_descriptor_holon(&context, "array", "ValueArrayValueType", "Value")?;
        array.add_related_holons(
            CoreRelationshipTypeName::AffordsOperator,
            vec![equals.clone().into()],
        )?;

        let descriptor = ValueArrayDescriptor::from_holon(array.into());
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
    fn apply_operator_always_reports_unsupported_operator() -> Result<(), HolonError> {
        let context = build_context();
        let equals = OperatorDescriptor::from_holon(
            new_descriptor_holon(&context, "equals", "EqualsOperator", "Holon")?.into(),
        );
        let descriptor = ValueArrayDescriptor::from_holon(
            new_descriptor_holon(&context, "array", "ValueArrayValueType", "Value")?.into(),
        );

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
