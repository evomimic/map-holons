use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::reference_layer::HolonReference;

pub fn invoke(
    context: &Arc<TransactionContext>,
    _bound_invocation: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    let commit_response = context.commit()?;
    Ok(Some(HolonReference::Transient(commit_response)))
}
