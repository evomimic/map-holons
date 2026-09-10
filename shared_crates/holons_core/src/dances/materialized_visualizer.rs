use base_types::{BaseValue, MapString};
use core_types::HolonError;

use crate::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use type_names::{CorePropertyTypeName, DahnPropertyTypeName, ToPropertyName};

/// Typed transient response body for a materialized Visualizer artifact.
///
/// The wrapper owns no executable bytes. Its holon contains only the opaque
/// artifact capability and module metadata; the host artifact backend retains
/// and serves the verified executable content.
#[derive(Debug, Clone)]
pub struct MaterializedVisualizer {
    holon: HolonReference,
}

impl MaterializedVisualizer {
    /// Wraps a holon after verifying its concrete response-body descriptor.
    pub fn new(holon: HolonReference) -> Result<Self, HolonError> {
        let descriptor = holon.holon_descriptor()?;
        let found = descriptor.header().type_name()?;
        let expected = MapString("MaterializedVisualizer".to_string());

        if found != expected {
            return Err(HolonError::WrongDescriptorKind {
                expected: expected.to_string(),
                found: found.to_string(),
                descriptor: found.to_string(),
            });
        }

        Ok(Self { holon })
    }

    /// Returns the bound response-body holon reference.
    pub fn as_holon_reference(&self) -> &HolonReference {
        &self.holon
    }

    /// Consumes the wrapper and returns its bound response-body reference.
    pub fn into_inner(self) -> HolonReference {
        self.holon
    }

    /// Returns the opaque host artifact capability issued for this response.
    pub fn artifact_handle(&self) -> Result<MapString, HolonError> {
        required_string_property(&self.holon, DahnPropertyTypeName::VisualizerArtifactHandle)
    }

    /// Returns the declared executable module format.
    pub fn module_format(&self) -> Result<MapString, HolonError> {
        required_string_property(&self.holon, DahnPropertyTypeName::VisualizerModuleFormat)
    }

    /// Returns the declared module entrypoint.
    pub fn entrypoint(&self) -> Result<MapString, HolonError> {
        required_string_property(&self.holon, CorePropertyTypeName::Entrypoint)
    }

    /// Records the opaque host artifact capability.
    pub fn set_artifact_handle(&mut self, handle: MapString) -> Result<(), HolonError> {
        self.holon.with_property_value(DahnPropertyTypeName::VisualizerArtifactHandle, handle)?;
        Ok(())
    }

    /// Records the executable module format supplied by the verified manifest.
    pub fn set_module_format(&mut self, format: MapString) -> Result<(), HolonError> {
        self.holon.with_property_value(DahnPropertyTypeName::VisualizerModuleFormat, format)?;
        Ok(())
    }

    /// Records the module entrypoint supplied by the verified manifest.
    pub fn set_entrypoint(&mut self, entrypoint: MapString) -> Result<(), HolonError> {
        self.holon.with_property_value(CorePropertyTypeName::Entrypoint, entrypoint)?;
        Ok(())
    }
}

impl From<MaterializedVisualizer> for HolonReference {
    fn from(value: MaterializedVisualizer) -> Self {
        value.into_inner()
    }
}

fn required_string_property<T: ToPropertyName>(
    holon: &HolonReference,
    property_name: T,
) -> Result<MapString, HolonError> {
    let property_name = property_name.to_property_name();
    match holon.property_value(property_name.clone())? {
        Some(BaseValue::StringValue(value)) => Ok(value),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(property_name.0 .0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};

    #[test]
    fn stores_only_artifact_capability_and_module_metadata() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = new_descriptor_holon(
            &context,
            "materialized-visualizer-descriptor",
            "MaterializedVisualizer",
            "Projection",
        )?;
        let mut response_body = new_test_holon(&context, "materialized-visualizer")?;
        response_body.with_descriptor(descriptor.into())?;

        let mut visualizer = MaterializedVisualizer::new(response_body.into())?;
        visualizer.set_artifact_handle(MapString("artifact:opaque".into()))?;
        visualizer.set_module_format(MapString("ESModule".into()))?;
        visualizer.set_entrypoint(MapString("default".into()))?;

        assert_eq!(visualizer.artifact_handle()?, MapString("artifact:opaque".into()));
        assert_eq!(visualizer.module_format()?, MapString("ESModule".into()));
        assert_eq!(visualizer.entrypoint()?, MapString("default".into()));
        assert!(visualizer
            .as_holon_reference()
            .property_value("VisualizerModuleSource")?
            .is_none());
        Ok(())
    }
}
