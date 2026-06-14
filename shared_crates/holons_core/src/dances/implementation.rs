use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{accessor_helpers, DanceDescriptor, TypeHeader};
use crate::dances::{implementations, BoundDanceInvocation};
use crate::reference_layer::HolonReference;
use type_names::{CoreDanceImplementationName, CoreRelationshipTypeName};

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
        let descriptor =
            accessor_helpers::require_single_related(&self.holon, CoreRelationshipTypeName::ForDance)?;
        Ok(DanceDescriptor::from_holon(descriptor))
    }

    pub fn invoke(
        &self,
        context: &Arc<TransactionContext>,
        bound_invocation: &BoundDanceInvocation,
    ) -> Result<Option<HolonReference>, HolonError> {
        let implementation_name = self.header().type_name()?;

        if implementation_name == CoreDanceImplementationName::Commit.as_command_name().0 {
            return implementations::commit::invoke(context, bound_invocation);
        }

        if implementation_name == CoreDanceImplementationName::DeleteHolon.as_command_name().0 {
            return implementations::delete_holon::invoke(context, bound_invocation);
        }

        Err(HolonError::NotImplemented(format!(
            "Descriptor-driven host invocation is not implemented for DanceImplementation `{}` yet. Guest-side persistence primitives should be wrapped explicitly rather than treated as canonical host dances.",
            implementation_name
        )))
    }
}

impl From<HolonReference> for DanceImplementation {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}
