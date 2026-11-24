//! Canonical loader adaptation; the environment service supplies Space authority.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::{BoundDanceInvocation, DanceResponseReference, RequiredExecutionContext};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use std::sync::Arc;

/// Validates loader-specific authority before crossing the execution boundary.
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound: &BoundDanceInvocation,
) -> Result<DanceResponseReference, HolonError> {
    if bound.dance_descriptor().header().type_name()?.0 != "LoadHolons"
        || bound.required_execution_context() != RequiredExecutionContext::SpaceAuthoritative
    {
        return Err(HolonError::InvalidParameter(
            "LoadHolons requires Space-authoritative LoadHolons".into(),
        ));
    }
    let affording = bound.affording_holon().ok_or_else(|| {
        HolonError::InvalidParameter("LoadHolons requires an affording HolonSpace".into())
    })?;
    super::space_authority::require_transaction_space(context, affording, "LoadHolons")?;
    let request = bound
        .request()
        .ok_or_else(|| HolonError::InvalidParameter("LoadHolons requires a HolonLoadSet".into()))?;
    if !matches!(request, HolonReference::Transient(_)) {
        return Err(HolonError::InvalidParameter("HolonLoadSet must be transient".into()));
    }
    context.invoke_load_holons(bound.invocation().clone())
}
