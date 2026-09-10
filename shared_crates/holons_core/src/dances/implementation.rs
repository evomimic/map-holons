use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::{implementations, BoundDanceInvocation};
use crate::descriptors::{accessor_helpers, DanceDescriptor, TypeHeader};
use crate::reference_layer::{HolonReference, ReadableHolon};
use base_types::{BaseValue, MapString};
use type_names::{
    CoreDanceImplementationName, CoreRelationshipTypeName, DahnDanceImplementationName,
};

/// Runtime wrapper for a dance implementation holon.
#[derive(Debug, Clone)]
pub struct DanceImplementation {
    holon: HolonReference,
}

impl DanceImplementation {
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    pub fn for_dance(&self) -> Result<DanceDescriptor, HolonError> {
        let descriptor = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::ForDance,
        )?;
        Ok(DanceDescriptor::from_holon(descriptor))
    }

    /// Returns the semantic identity authored by this implementation instance.
    pub fn implementation_name(&self) -> Result<MapString, HolonError> {
        match self.holon.property_value("ImplementationName")? {
            Some(BaseValue::StringValue(name)) => Ok(name),
            Some(other) => {
                Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
            }
            None => Err(HolonError::EmptyField("ImplementationName".to_string())),
        }
    }

    pub fn invoke(
        &self,
        context: &Arc<TransactionContext>,
        bound_invocation: &BoundDanceInvocation,
    ) -> Result<Option<HolonReference>, HolonError> {
        let implementation_name = self.implementation_name()?;

        if implementation_name == CoreDanceImplementationName::Commit.as_command_name().0 {
            return implementations::commit::invoke(context, bound_invocation);
        }

        if implementation_name == CoreDanceImplementationName::DeleteHolon.as_command_name().0 {
            return implementations::delete_holon::invoke(context, bound_invocation);
        }

        if implementation_name
            == DahnDanceImplementationName::LocalMaterializeVisualizer.as_implementation_name()
        {
            return implementations::materialize_visualizer::invoke(context, bound_invocation);
        }

        Err(HolonError::NotImplemented(format!(
            "Descriptor-driven invocation is not implemented for DanceImplementation `{}` yet. Implementations must adapt the canonical Dance contract to capabilities available in their execution context.",
            implementation_name
        )))
    }
}

impl From<HolonReference> for DanceImplementation {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::reference_layer::WritableHolon;

    #[test]
    fn implementation_name_reads_the_instance_identity_not_descriptor_metadata(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut implementation = new_test_holon(&context, "local-materializer")?;
        implementation.with_property_value("ImplementationName", "LocalMaterializeVisualizer")?;

        let implementation = DanceImplementation::from_holon(implementation.into());
        assert_eq!(
            implementation.implementation_name()?,
            DahnDanceImplementationName::LocalMaterializeVisualizer.as_implementation_name()
        );
        Ok(())
    }
}
