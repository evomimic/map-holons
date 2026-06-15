use std::sync::Arc;

use core_types::{HolonError, HolonId};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::Descriptor;
use crate::dances::{BoundDanceInvocation, DeleteHolonParameters};
use crate::reference_layer::ReadableHolon;

pub fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Option<crate::reference_layer::HolonReference>, HolonError> {
    let request = bound_invocation
        .request()
        .cloned()
        .ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: bound_invocation
                .invocation()
                .as_holon_reference()
                .summarize()
                .unwrap_or_else(|_| "DanceInvocation".to_string()),
        })?;
    let request_type = bound_invocation.request_type().ok_or_else(|| {
        HolonError::MissingRequiredRelationship {
            relationship: "RequestType".to_string(),
            descriptor: bound_invocation.dance_descriptor().holon().summarize().unwrap_or_else(
                |_| "DeleteHolon".to_string(),
            ),
        }
    })?;
    let parameters = DeleteHolonParameters::new(request, request_type)?;
    let holon_id = parameters.holon_id()?;

    let local_id = match holon_id {
        HolonId::Local(local_id) => local_id,
        HolonId::External(external_id) => external_id.local_id,
    };

    context.mutation().delete_holon(local_id)?;

    Ok(None)
}
