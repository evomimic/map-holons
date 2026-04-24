use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use core_types::HolonError;
use type_names::CorePropertyTypeName;

/// Borrowed projection of the shared descriptor header fields.
///
/// `TypeHeader` does not own data or cache values; it centralizes the common
/// property extraction logic used across descriptor wrappers.
pub struct TypeHeader<'a> {
    holon: &'a HolonReference,
}

impl<'a> TypeHeader<'a> {
    pub(crate) fn new(holon: &'a HolonReference) -> Self {
        Self { holon }
    }

    /// Returns the required singular type name from the backing descriptor.
    pub fn type_name(&self) -> Result<MapString, HolonError> {
        self.require_string(CorePropertyTypeName::TypeName)
    }

    /// Returns the optional plural type name when the schema provides one.
    pub fn type_name_plural(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::TypeNamePlural)
    }

    /// Returns the optional human-facing display name.
    pub fn display_name(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::DisplayName)
    }

    /// Returns the optional plural display name.
    pub fn display_name_plural(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::DisplayNamePlural)
    }

    /// Returns the optional human-facing description.
    pub fn description(&self) -> Result<Option<MapString>, HolonError> {
        self.optional_string(CorePropertyTypeName::Description)
    }

    /// Returns whether the descriptor is marked abstract.
    pub fn is_abstract_type(&self) -> Result<bool, HolonError> {
        self.require_bool(CorePropertyTypeName::IsAbstractType)
    }

    /// Returns the required instance kind string from the header.
    pub fn instance_type_kind(&self) -> Result<MapString, HolonError> {
        self.require_string(CorePropertyTypeName::InstanceTypeKind)
    }

    fn require_string(&self, prop: CorePropertyTypeName) -> Result<MapString, HolonError> {
        // Required string fields share the same missing-vs-wrong-type semantics.
        let property_name = prop.as_property_name();
        match self.holon.property_value(prop)? {
            Some(BaseValue::StringValue(value)) => Ok(value),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }

    fn optional_string(&self, prop: CorePropertyTypeName) -> Result<Option<MapString>, HolonError> {
        // Optional string fields still fail loudly when the stored value shape is wrong.
        match self.holon.property_value(prop)? {
            Some(BaseValue::StringValue(value)) => Ok(Some(value)),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "String".to_string()))
            }
            None => Ok(None),
        }
    }

    fn require_bool(&self, prop: CorePropertyTypeName) -> Result<bool, HolonError> {
        // Boolean header fields mirror the required-string contract with a different base type.
        let property_name = prop.as_property_name();
        match self.holon.property_value(prop)? {
            Some(BaseValue::BooleanValue(value)) => Ok(value.0),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{:?}", other), "Boolean".to_string()))
            }
            None => Err(HolonError::EmptyField(property_name.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::reference_layer::WritableHolon;
    use base_types::MapString;
    use core_types::HolonError;

    fn build_header_holon() -> Result<HolonReference, HolonError> {
        let context = build_context();
        // Populate the full shared header surface for the happy-path assertions.
        let mut holon = new_test_holon(&context, "header-holon")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::TypeNamePlural, "HolonTypes")
            })
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::DisplayName, "Holon Type")
            })
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::DisplayNamePlural, "Holon Types")
            })
            .and_then(|holon| {
                holon.with_property_value(
                    CorePropertyTypeName::Description,
                    "Descriptor header test holon",
                )
            })
            .and_then(|holon| holon.with_property_value(CorePropertyTypeName::IsAbstractType, true))
            .and_then(|holon| {
                holon.with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")
            })?;

        Ok(HolonReference::Transient(holon))
    }

    #[test]
    fn header_accessors_return_expected_values() -> Result<(), HolonError> {
        let holon_ref = build_header_holon()?;
        let header = TypeHeader::new(&holon_ref);

        assert_eq!(header.type_name()?, MapString("HolonType".to_string()));
        assert_eq!(header.type_name_plural()?, Some(MapString("HolonTypes".to_string())));
        assert_eq!(header.display_name()?, Some(MapString("Holon Type".to_string())));
        assert_eq!(header.display_name_plural()?, Some(MapString("Holon Types".to_string())));
        assert_eq!(
            header.description()?,
            Some(MapString("Descriptor header test holon".to_string()))
        );
        assert!(header.is_abstract_type()?);
        assert_eq!(header.instance_type_kind()?, MapString("Holon".to_string()));

        Ok(())
    }

    #[test]
    fn type_name_errors_when_required_property_missing() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_test_holon(&context, "missing-type-name")?;
        holon
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert!(matches!(
            header.type_name(),
            Err(HolonError::EmptyField(field)) if field == "TypeName"
        ));

        Ok(())
    }

    #[test]
    fn optional_accessors_return_none_when_absent() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_test_holon(&context, "optional-header")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert_eq!(header.type_name_plural()?, None);
        assert_eq!(header.display_name()?, None);
        assert_eq!(header.display_name_plural()?, None);
        assert_eq!(header.description()?, None);

        Ok(())
    }

    #[test]
    fn type_name_errors_when_property_value_has_wrong_type() -> Result<(), HolonError> {
        let context = build_context();
        let mut holon = new_test_holon(&context, "wrong-type-name")?;
        holon
            .with_property_value(CorePropertyTypeName::TypeName, true)?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let holon_ref = HolonReference::Transient(holon);
        let header = TypeHeader::new(&holon_ref);

        assert!(matches!(
            header.type_name(),
            Err(HolonError::UnexpectedValueType(_, expected)) if expected == "String"
        ));

        Ok(())
    }

    #[test]
    fn required_header_accessors_error_when_property_missing() -> Result<(), HolonError> {
        let context = build_context();
        // Exercise each required accessor with its own missing-property shape.
        let mut missing_bool_holon = new_test_holon(&context, "missing-is-abstract")?;
        missing_bool_holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;

        let missing_bool_ref = HolonReference::Transient(missing_bool_holon);
        let missing_bool_header = TypeHeader::new(&missing_bool_ref);

        assert!(matches!(
            missing_bool_header.is_abstract_type(),
            Err(HolonError::EmptyField(field)) if field == "IsAbstractType"
        ));

        let mut missing_kind_holon = new_test_holon(&context, "missing-instance-type-kind")?;
        missing_kind_holon
            .with_property_value(CorePropertyTypeName::TypeName, "HolonType")?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?;

        let missing_kind_ref = HolonReference::Transient(missing_kind_holon);
        let missing_kind_header = TypeHeader::new(&missing_kind_ref);

        assert!(matches!(
            missing_kind_header.instance_type_kind(),
            Err(HolonError::EmptyField(field)) if field == "InstanceTypeKind"
        ));

        Ok(())
    }
}
