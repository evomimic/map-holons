//! Narrow static `QueryDance` adapter for the DanceV2 executor (QRY1).
//!
//! `QueryDance.DanceType` deliberately binds no `DanceImplementation`. After
//! ordinary binding and request/response-contract validation, the executor
//! routes it here instead of generic `ForDance` implementation selection. The
//! adapter maps the request's `RequestedQuery`, `InitialInput` (forwarded by
//! identity as a `HolonCollectionReference`), and `RequestParameters` onto the
//! internal direct Query seam and propagates the error that seam produces.
//!
//! Scaffold-only: [`invoke`] cannot succeed (its `Ok` type is `Infallible`) and
//! the executor never mints a `QueryDanceResponse` on this route. QRY2
//! introduces the success path together with the first operators.

use std::convert::Infallible;
use std::sync::Arc;

use core_types::HolonError;
use type_names::QueryDanceRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::descriptors::DanceDescriptor;
use crate::query_layer::query_core::{
    exactly_one, related_members, HolonCollectionReference, QueryReference,
};
use crate::reference_layer::ReadableHolon;

const QUERY_DANCE_TYPE_NAME: &str = "QueryDance";

/// True when the bound invocation resolved to the `QueryDance` descriptor.
pub(crate) fn is_query_dance(dance_descriptor: &DanceDescriptor) -> Result<bool, HolonError> {
    Ok(dance_descriptor.header().type_name()?.0 == QUERY_DANCE_TYPE_NAME)
}

/// Routes a validated `QueryDance` invocation onto the direct Query seam.
///
/// Request-shape problems surface as the existing structured contract errors
/// (`MissingRequiredRelationship`, `MultipleRelatedHolons`, `WrongDescriptorKind`);
/// a well-formed request reaches the seam, which in QRY1 records the failed
/// execution and returns `HolonError::NotImplemented`.
pub(crate) fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Infallible, HolonError> {
    let Some(request) = bound_invocation.request() else {
        return Err(HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: bound_invocation.invocation().as_holon_reference().summarize()?,
        });
    };

    let query =
        QueryReference::new(exactly_one(request, QueryDanceRelationshipTypeName::RequestedQuery)?)?;
    let input = HolonCollectionReference::new(exactly_one(
        request,
        QueryDanceRelationshipTypeName::InitialInput,
    )?)?;
    let bindings = related_members(request, QueryDanceRelationshipTypeName::RequestParameters)?;

    let execution = query.begin_execution(context, input, bindings)?;
    Err(execution.run().err().unwrap_or_else(|| {
        HolonError::NotImplemented("QueryDance success path (QRY2)".to_string())
    }))
}
