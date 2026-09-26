//! Narrow static `QueryDance` adapter for the DanceV2 executor.
//!
//! `QueryDance.DanceType` deliberately binds no `DanceImplementation`. After
//! ordinary binding and request/response-contract validation, the executor
//! routes it here instead of generic `ForDance` implementation selection. The
//! adapter maps the Dance contract onto the internal direct Query seam:
//!
//! - the validated `AffordingHolon` (a `HolonSpace`, per `DanceAffordedBy`)
//!   becomes the invocation's focal space — Query itself imports no Dance type;
//! - `RequestedQuery` becomes the `QueryReference`;
//! - the optional `InitialInput` is forwarded by identity as a
//!   `HolonCollectionReference` (the root expression decides whether it is
//!   required or forbidden);
//! - `RequestParameters` are forwarded as unresolved bindings.
//!
//! On success the seam's result collection holon is returned for the executor
//! to attach as the `QueryDanceResponse` body; every failure propagates the
//! seam's own error.

use std::sync::Arc;

use core_types::HolonError;
use type_names::{CoreRelationshipTypeName, QueryDanceRelationshipTypeName, ToRelationshipName};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::descriptors::DanceDescriptor;
use crate::query_layer::query_core::{
    exactly_one, related_members, zero_or_one, FocalSpaceReference, HolonCollectionReference,
    QueryReference,
};
use crate::reference_layer::{HolonReference, ReadableHolon};

const QUERY_DANCE_TYPE_NAME: &str = "QueryDance";

/// True when the bound invocation resolved to the `QueryDance` descriptor.
pub(crate) fn is_query_dance(dance_descriptor: &DanceDescriptor) -> Result<bool, HolonError> {
    Ok(dance_descriptor.header().type_name()?.0 == QUERY_DANCE_TYPE_NAME)
}

/// Routes a validated `QueryDance` invocation onto the direct Query seam and
/// returns the result collection holon.
///
/// Request-shape problems surface as the existing structured contract errors
/// (`MissingRequiredRelationship`, `MultipleRelatedHolons`, `WrongDescriptorKind`);
/// a well-formed request reaches the seam, whose errors propagate unchanged.
pub(crate) fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<HolonReference, HolonError> {
    let invocation = bound_invocation.invocation().as_holon_reference();
    let Some(request) = bound_invocation.request() else {
        return Err(HolonError::MissingRequiredRelationship {
            relationship: CoreRelationshipTypeName::Request.to_relationship_name().to_string(),
            descriptor: invocation.summarize()?,
        });
    };
    let Some(affording_holon) = bound_invocation.affording_holon() else {
        return Err(HolonError::MissingRequiredRelationship {
            relationship: CoreRelationshipTypeName::AffordingHolon
                .to_relationship_name()
                .to_string(),
            descriptor: invocation.summarize()?,
        });
    };

    let focal_space = FocalSpaceReference::new(affording_holon.clone())?;
    let query =
        QueryReference::new(exactly_one(request, QueryDanceRelationshipTypeName::RequestedQuery)?)?;
    let input = zero_or_one(request, QueryDanceRelationshipTypeName::InitialInput)?
        .map(HolonCollectionReference::new)
        .transpose()?;
    let bindings = related_members(request, QueryDanceRelationshipTypeName::RequestParameters)?;

    let result = query.begin_execution(context, focal_space, input, bindings)?.run()?;
    Ok(result.as_holon_reference().clone())
}
