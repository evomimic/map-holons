use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::{HolonReference, ReadableHolon};
use core_types::HolonError;
use std::sync::Arc;

/// Resolve a Visualizer's configuration through the service's guest-only read capability.
pub fn invoke(
    context: &Arc<TransactionContext>,
    invocation: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    let visualizer = invocation.affording_holon().ok_or_else(|| {
        HolonError::InvalidParameter("Usage resolution requires an affording Visualizer".into())
    })?;
    let request = invocation.request().ok_or_else(|| {
        HolonError::InvalidParameter("Usage resolution requires a request".into())
    })?;
    let one = |name: &str| -> Result<HolonReference, HolonError> {
        let members = request
            .related_holons(name)?
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        match members.as_slice() {
            [value] => Ok(value.clone()),
            _ => Err(HolonError::InvalidParameter(format!("Expected one {name}"))),
        }
    };
    context
        .resolve_visualizer_usage(visualizer, &one("UsageSubjectType")?, &one("UsageSlot")?)?
        .into_projection(context)
        .map(Some)
}
