use crate::descriptors::{
    accessor_helpers, walk_extends_chain, Descriptor, TypeHeader, ValueDescriptor,
};
use crate::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use base_types::BaseValue;
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

    /// Populates this descriptor's effective default only when the target has
    /// no authored value and the effective property is required.
    pub(crate) fn populate_default_if_required_and_absent<H>(
        &self,
        target: &mut H,
    ) -> Result<(), HolonError>
    where
        H: ReadableHolon + WritableHolon + ?Sized,
    {
        let property_name = PropertyName(self.header().type_name()?);
        if target.property_value(&property_name)?.is_some() {
            return Ok(());
        }
        if !self.effective_is_value_required()? {
            return Ok(());
        }
        if let Some(default_value) = self.effective_default_value()? {
            target.with_property_value(property_name, default_value)?;
        }
        Ok(())
    }

    fn effective_is_value_required(&self) -> Result<bool, HolonError> {
        match self.effective_property_value(CorePropertyTypeName::IsValueRequired)? {
            Some(BaseValue::BooleanValue(value)) => Ok(value.0),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into()))
            }
            None => {
                // Preserve the existing 1.x wrapper behavior for callers that
                // still construct legacy descriptor fixtures.
                match self.effective_property_value(CorePropertyTypeName::IsRequired)? {
                    Some(BaseValue::BooleanValue(value)) => Ok(value.0),
                    Some(other) => {
                        Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into()))
                    }
                    None => Err(HolonError::EmptyField("IsValueRequired".into())),
                }
            }
        }
    }

    fn effective_default_value(&self) -> Result<Option<BaseValue>, HolonError> {
        self.effective_property_value(CorePropertyTypeName::DefaultValue)
    }

    /// Resolves a descriptor-definition property self-first across `L(P)`.
    /// The property descriptor's own effective-member semantics are deliberately
    /// separate from the `L(D(H))` walk used to select this descriptor.
    fn effective_property_value(
        &self,
        name: CorePropertyTypeName,
    ) -> Result<Option<BaseValue>, HolonError> {
        for ancestor in walk_extends_chain(&self.holon) {
            if let Some(value) = ancestor?.property_value(name.clone())? {
                return Ok(Some(value));
            }
        }
        Ok(None)
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
    use crate::descriptors::test_support::new_test_holon;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon};
    use crate::reference_layer::{ReadableHolon, WritableHolon};
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
    fn property_name_errors_when_required_field_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon = new_descriptor_holon(
            &context,
            "missing-property-name",
            "MissingPropertyName",
            "Property",
        )?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.property_name(),
            Err(HolonError::EmptyField(field)) if field == "PropertyName"
        ));

        Ok(())
    }

    #[test]
    fn property_name_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon =
            new_descriptor_holon(&context, "wrong-property-name", "WrongPropertyName", "Property")?;
        holon.with_property_value(CorePropertyTypeName::PropertyName, true)?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.property_name(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "String"
        ));

        Ok(())
    }

    #[test]
    fn is_required_errors_when_required_field_is_missing() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            new_descriptor_holon(&context, "missing-is-required", "MissingIsRequired", "Property")?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.is_required(),
            Err(HolonError::EmptyField(field)) if field == "IsRequired"
        ));

        Ok(())
    }

    #[test]
    fn is_required_errors_when_required_field_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon =
            new_descriptor_holon(&context, "wrong-is-required", "WrongIsRequired", "Property")?;
        holon.with_property_value(CorePropertyTypeName::IsRequired, "not-a-boolean")?;
        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.is_required(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean"
        ));

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

    #[test]
    fn value_type_errors_when_multiple_targets_exist() -> Result<(), HolonError> {
        let context = build_context();
        let value_type_a =
            new_descriptor_holon(&context, "string-value-type-a", "StringValueTypeA", "Value")?;
        let value_type_b =
            new_descriptor_holon(&context, "string-value-type-b", "StringValueTypeB", "Value")?;
        let mut holon = new_descriptor_holon(
            &context,
            "multiple-value-types",
            "MultipleValueTypes",
            "Property",
        )?;
        holon.add_related_holons(
            CoreRelationshipTypeName::ValueType,
            vec![value_type_a.into(), value_type_b.into()],
        )?;

        let descriptor = PropertyDescriptor::from_holon(holon.into());

        assert!(matches!(
            descriptor.value_type(),
            Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
                if relationship == "ValueType" && count == 2
        ));

        Ok(())
    }

    #[test]
    fn populate_default_populates_required_absent_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "enabled-property", "Enabled", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "default-target")?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(
            target.property_value("Enabled")?,
            Some(base_types::BaseValue::BooleanValue(base_types::MapBoolean(false)))
        );
        Ok(())
    }

    #[test]
    fn populate_default_preserves_authored_value() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "enabled-property", "Enabled", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, true)?
            .with_property_value(CorePropertyTypeName::DefaultValue, false)?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "authored-target")?;
        target.with_property_value("Enabled", true)?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(
            target.property_value("Enabled")?,
            Some(base_types::BaseValue::BooleanValue(base_types::MapBoolean(true)))
        );
        Ok(())
    }

    #[test]
    fn populate_default_leaves_optional_value_absent() -> Result<(), HolonError> {
        let context = build_context();
        let mut property =
            new_descriptor_holon(&context, "optional-property", "Optional", "Property")?;
        property
            .with_property_value(CorePropertyTypeName::IsValueRequired, false)?
            .with_property_value(CorePropertyTypeName::DefaultValue, "ignored")?;
        let descriptor = PropertyDescriptor::from_holon(property.into());
        let mut target = new_test_holon(&context, "optional-target")?;

        descriptor.populate_default_if_required_and_absent(&mut target)?;

        assert_eq!(target.property_value("Optional")?, None);
        Ok(())
    }
}
