use crate::descriptors::{accessor_helpers, Descriptor, TypeHeader, ValueDescriptor};
use crate::reference_layer::HolonReference;
use core_types::{HolonError, PropertyName};
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

/// Runtime wrapper for property descriptors.
///
/// This remains a thin view in Phase 1/2 so later value-type behavior can land
/// on a stable wrapper without changing call-site types.
pub struct PropertyDescriptor {
    holon: HolonReference,
}

impl PropertyDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the runtime property name declared by this descriptor.
    pub fn property_name(&self) -> Result<PropertyName, HolonError> {
        Ok(PropertyName(accessor_helpers::require_string(
            &self.holon,
            CorePropertyTypeName::PropertyName,
        )?))
    }

    /// Returns whether instances must provide this property.
    pub fn is_required(&self) -> Result<bool, HolonError> {
        accessor_helpers::require_bool(&self.holon, CorePropertyTypeName::IsRequired)
    }

    /// Returns the value descriptor reached through the required `ValueType` relationship.
    pub fn value_type(&self) -> Result<ValueDescriptor, HolonError> {
        let value_type = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::ValueType,
        )?;
        Ok(ValueDescriptor::from_holon(value_type))
    }
}

impl From<HolonReference> for PropertyDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for PropertyDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<PropertyDescriptor>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapString;
    use core_types::HolonError;
    use type_names::CoreRelationshipTypeName;

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon = HolonReference::from(&new_descriptor_holon(
            &context,
            "property-descriptor",
            "PropertyType",
            "Property",
        )?);

        let descriptor = PropertyDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("PropertyType".to_string()));

        Ok(())
    }

    #[test]
    fn structural_accessors_return_declared_values() -> Result<(), HolonError> {
        let context = build_context();
        let value_type =
            new_descriptor_holon(&context, "string-value-type", "StringValueType", "Value")?;
        let mut holon =
            new_descriptor_holon(&context, "title-property", "TitleProperty", "Property")?;
        holon
            .with_property_value(CorePropertyTypeName::PropertyName, "title")?
            .with_property_value(CorePropertyTypeName::IsRequired, true)?;
        holon.add_related_holons(CoreRelationshipTypeName::ValueType, vec![value_type.into()])?;

        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert_eq!(descriptor.property_name()?.to_string(), "title");
        assert!(descriptor.is_required()?);
        assert_eq!(
            descriptor.value_type()?.header().type_name()?,
            MapString("StringValueType".to_string())
        );

        Ok(())
    }

    #[test]
    fn value_type_errors_when_required_relationship_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            new_descriptor_holon(&context, "missing-value-type", "MissingValueType", "Property")?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.value_type(),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "ValueType"
        ));

        Ok(())
    }
}
