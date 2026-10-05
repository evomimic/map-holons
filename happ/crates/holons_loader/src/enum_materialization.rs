use std::sync::Arc;

use holons_prelude::prelude::*;

use crate::errors::ErrorWithContext;

/// Runs the loader's final default-population and enum-materialization pass before Commit.
/// Early descriptor attachment may already have populated some values. Normalize all
/// local defaults before attempting further population, then materialize property values,
/// including tokens copied by early attempts, so nursery order cannot affect native kinds.
pub(crate) fn complete_loaded_values(
    context: &Arc<TransactionContext>,
) -> Result<Vec<ErrorWithContext>, HolonError> {
    let staged = context.staged_references()?;
    if staged.is_empty() {
        return Ok(Vec::new());
    }
    let materializer = match EnumMaterializer::resolve(context) {
        Ok(materializer) => materializer,
        Err(error) => return Ok(vec![ErrorWithContext { error, source_loader_key: None }]),
    };
    let mut errors = Vec::new();
    for mut holon in staged.iter().cloned() {
        if let Err(error) = materializer.materialize_default(&mut holon) {
            errors.push(ErrorWithContext { error, source_loader_key: holon.key()? });
        }
    }
    if !errors.is_empty() {
        return Ok(errors);
    }
    for mut holon in staged {
        let result = holon
            .populate_defaults()
            .and_then(|()| materializer.materialize_properties(&mut holon));
        if let Err(error) = result {
            errors.push(ErrorWithContext { error, source_loader_key: holon.key()? });
        }
    }
    Ok(errors)
}

/// Loader-local interpretation of imported enum tokens after relationship resolution.
/// Holds only transaction-bound anchors; materialization changes values, never their lineage.
struct EnumMaterializer {
    enum_family: HolonReference,
    property_family: HolonReference,
}

impl EnumMaterializer {
    fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Ok(Self {
            enum_family: resolve_core_descriptor(context, "EnumValueType.ValueType")?,
            property_family: resolve_core_descriptor(context, "PropertyType.TypeDescriptor")?,
        })
    }

    /// DefaultValue is AnyBaseValue: its native representation is selected by the
    /// owning property descriptor, not by DefaultValue.PropertyType's value type.
    /// Only local defaults are rewritten, preserving inheritance and authored absence.
    fn materialize_default(&self, holon: &mut StagedReference) -> Result<(), HolonError> {
        let Some(BaseValue::StringValue(token)) =
            holon.property_value(CorePropertyTypeName::DefaultValue)?
        else {
            return Ok(());
        };
        let reference = HolonReference::from(holon.clone());
        if equals_or_extends(&reference, &self.property_family)? {
            let property = PropertyDescriptor::from_holon(reference);
            if self.is_enum(&property)? {
                holon
                    .with_property_value(CorePropertyTypeName::DefaultValue, MapEnumValue(token))?;
            }
        }
        Ok(())
    }

    /// Converts populated imported strings only when the effective property selects
    /// an enum family. Invalid token spelling remains intact for enum membership validation.
    fn materialize_properties(&self, holon: &mut StagedReference) -> Result<(), HolonError> {
        let descriptor = match holon.holon_descriptor() {
            Ok(descriptor) => descriptor,
            // Undescribed holons have no contract to select native representations;
            // Commit reports the missing descriptor.
            Err(HolonError::MissingDescribedBy { .. }) => return Ok(()),
            Err(error) => return Err(error),
        };
        for property in descriptor.instance_properties()? {
            let name = property.property_name()?;
            if let Some(BaseValue::StringValue(token)) = holon.property_value(&name)? {
                if self.is_enum(&property)? {
                    holon.with_property_value(name, MapEnumValue(token))?;
                }
            }
        }
        Ok(())
    }

    fn is_enum(&self, property: &PropertyDescriptor) -> Result<bool, HolonError> {
        equals_or_extends(property.value_type()?.holon(), &self.enum_family)
    }
}

#[cfg(test)]
mod tests;
