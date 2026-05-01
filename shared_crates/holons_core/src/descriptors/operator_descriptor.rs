use crate::descriptors::inheritance::flatten_related_members;
use crate::descriptors::{
    accessor_helpers, Descriptor, OperatorCategory, TypeHeader, ValueDescriptor,
};
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for operator descriptors.
///
/// Operators remain schema-declared holons. This wrapper exposes their
/// descriptor-local metadata without introducing a global operator registry.
pub struct OperatorDescriptor {
    holon: HolonReference,
}

impl OperatorDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the operator descriptor's canonical type name.
    pub fn type_name(&self) -> Result<MapString, HolonError> {
        self.header().type_name()
    }

    /// Returns the optional human-facing display name.
    pub fn display_name(&self) -> Result<Option<MapString>, HolonError> {
        self.header().display_name()
    }

    /// Returns the optional human-facing description.
    pub fn description(&self) -> Result<Option<MapString>, HolonError> {
        self.header().description()
    }

    /// Returns the declared operand count for this operator.
    pub fn arity(&self) -> Result<u8, HolonError> {
        let arity = accessor_helpers::require_integer(&self.holon, CorePropertyTypeName::Arity)?;
        arity.try_into().map_err(|_| HolonError::IntegerOutOfRange {
            value: arity,
            min: u8::MIN.into(),
            max: u8::MAX.into(),
            context: "OperatorDescriptor::arity".to_string(),
        })
    }

    /// Returns the parsed operator category enum value.
    pub fn operator_category(&self) -> Result<OperatorCategory, HolonError> {
        let value = accessor_helpers::require_enum_string(
            &self.holon,
            CorePropertyTypeName::OperatorCategory,
        )?;
        OperatorCategory::parse(&value)
    }

    /// Returns value descriptors that declare this operator as afforded.
    pub fn afforded_by(&self) -> Result<Vec<ValueDescriptor>, HolonError> {
        Ok(flatten_related_members(&self.holon, CoreRelationshipTypeName::AffordedBy)?
            .into_iter()
            .map(ValueDescriptor::from_holon)
            .collect())
    }
}

impl From<HolonReference> for OperatorDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for OperatorDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<OperatorDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapEnumValue;

    fn operator_category_value(value: &str) -> MapEnumValue {
        MapEnumValue(MapString(value.to_string()))
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "operator-descriptor",
            "EqualsOperator",
            "Holon",
        )?);

        let descriptor = OperatorDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("EqualsOperator".to_string()));

        Ok(())
    }

    #[test]
    fn accessors_return_declared_values() -> Result<(), HolonError> {
        let context = build_context();
        let value_type =
            new_descriptor_holon(&context, "integer-value-type", "IntegerValueType", "Value")?;
        let mut holon =
            new_descriptor_holon(&context, "equals-operator", "EqualsOperator", "Holon")?;
        holon
            .with_property_value(CorePropertyTypeName::DisplayName, "Equals")?
            .with_property_value(CorePropertyTypeName::Description, "Returns equality.")?
            .with_property_value(CorePropertyTypeName::Arity, 2_i64)?
            .with_property_value(
                CorePropertyTypeName::OperatorCategory,
                operator_category_value("OperatorCategory.Equality"),
            )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::AffordedBy,
            vec![value_type.clone().into()],
        )?;

        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.type_name()?, MapString("EqualsOperator".to_string()));
        assert_eq!(descriptor.display_name()?, Some(MapString("Equals".to_string())));
        assert_eq!(descriptor.description()?, Some(MapString("Returns equality.".to_string())));
        assert_eq!(descriptor.arity()?, 2);
        assert_eq!(descriptor.operator_category()?, OperatorCategory::Equality);
        assert_eq!(
            descriptor.afforded_by()?[0].header().type_name()?,
            MapString("IntegerValueType".to_string())
        );

        Ok(())
    }

    #[test]
    fn arity_errors_when_required_field_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(&context, "missing-arity", "MissingArity", "Holon")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::EmptyField(field)) if field == "Arity"
        ));

        Ok(())
    }

    #[test]
    fn arity_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "wrong-arity", "WrongArity", "Holon")?;
        holon.with_property_value(CorePropertyTypeName::Arity, "not-an-integer")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Integer"
        ));

        Ok(())
    }

    #[test]
    fn arity_errors_when_integer_is_out_of_range() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(&context, "overflow-arity", "OverflowArity", "Holon")?;
        holon.with_property_value(CorePropertyTypeName::Arity, 256_i64)?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.arity(),
            Err(HolonError::IntegerOutOfRange { value, min, max, context })
                if value == 256 && min == 0 && max == 255 && context == "OperatorDescriptor::arity"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "wrong-operator-category",
            "WrongOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(CorePropertyTypeName::OperatorCategory, "Equality")?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Enum"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_errors_for_unknown_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "unknown-operator-category",
            "UnknownOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(
            CorePropertyTypeName::OperatorCategory,
            operator_category_value("OperatorCategory.Matching"),
        )?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnknownOperatorCategory { value })
                if value == "OperatorCategory.Matching"
        ));

        Ok(())
    }

    #[test]
    fn operator_category_rejects_bare_variant_name() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_descriptor_holon(
            &context,
            "bare-operator-category",
            "BareOperatorCategory",
            "Holon",
        )?;
        holon.with_property_value(
            CorePropertyTypeName::OperatorCategory,
            operator_category_value("Equality"),
        )?;
        let descriptor = OperatorDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.operator_category(),
            Err(HolonError::UnknownOperatorCategory { value }) if value == "Equality"
        ));

        Ok(())
    }
}
