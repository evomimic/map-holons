use std::sync::Arc;

use core_types::{HolonError, HolonId};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::accessor_helpers;
use crate::dances::BoundDanceInvocation;
use crate::reference_layer::ReadableHolon;
use type_names::CoreRelationshipTypeName;

pub fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Option<crate::reference_layer::HolonReference>, HolonError> {
    let request = bound_invocation.request().ok_or_else(|| HolonError::MissingRequiredRelationship {
        relationship: CoreRelationshipTypeName::Request.as_relationship_name().to_string(),
        descriptor: bound_invocation
            .invocation()
            .as_holon_reference()
            .summarize()
            .unwrap_or_else(|_| "DanceInvocation".to_string()),
    })?;

    let target = accessor_helpers::require_single_related(
        request,
        CoreRelationshipTypeName::ReferenceTarget,
    )?;

    let holon_id = target.holon_id()?;
    let local_id = match holon_id {
        HolonId::Local(local_id) => local_id,
        HolonId::External(external_id) => external_id.local_id,
    };

    context.mutation().delete_holon(local_id)?;

    Ok(None)
}
