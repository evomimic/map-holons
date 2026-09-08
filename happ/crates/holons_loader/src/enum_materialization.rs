use std::sync::Arc;

use holons_prelude::prelude::*;

use crate::errors::ErrorWithContext;

/// Completes the loader's staged inputs before Commit. Normalize all local defaults
/// before copying any inherited default, so nursery order cannot affect native kinds.
pub(crate) fn complete_loaded_values(
    context: &Arc<TransactionContext>,
) -> Result<(usize, Vec<ErrorWithContext>), HolonError> {
    let staged = context.staged_references()?;
    if staged.is_empty() {
        return Ok((0, Vec::new()));
    }
    let materializer = match EnumMaterializer::resolve(context) {
        Ok(materializer) => materializer,
        Err(error) => return Ok((0, vec![ErrorWithContext { error, source_loader_key: None }])),
    };
    let mut errors = Vec::new();
    for mut holon in staged.iter().cloned() {
        if let Err(error) = materializer.materialize_default(&mut holon) {
            errors.push(ErrorWithContext { error, source_loader_key: holon.key()? });
        }
    }
    if !errors.is_empty() {
        return Ok((0, errors));
    }
    let mut deferred_count = 0;
    for mut holon in staged {
        let result = match holon.populate_defaults() {
            Ok(CompletionOutcome::Completed) => materializer.materialize_properties(&mut holon),
            Ok(CompletionOutcome::DeferredNoDescriptor) => {
                deferred_count += 1;
                Ok(())
            }
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            errors.push(ErrorWithContext { error, source_loader_key: holon.key()? });
        }
    }
    Ok((deferred_count, errors))
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
        for property in holon.available_properties()? {
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
