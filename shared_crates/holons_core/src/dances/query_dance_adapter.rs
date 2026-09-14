//! Narrow static `QueryDance` adapter for the DanceV2 executor (QRY1).
//!
//! `QueryDance.DanceType` deliberately binds no `DanceImplementation`. After
//! ordinary binding and request/response-contract validation, the executor
//! routes it here instead of generic `ForDance` implementation selection. The
//! adapter maps the request's `RequestedQuery`, `InitialInput`, and
//! `RequestParameters` onto the internal direct Query seam and returns the
//! error it produces.
//!
//! This is scaffold-only: [`invoke`] cannot succeed. It has no response-shaped
//! return type, and the executor never mints a `QueryDanceResponse` on this
//! route. QRY2 introduces the success path together with the first operators.

use std::convert::Infallible;
use std::sync::Arc;

use core_types::HolonError;
use type_names::QueryDanceRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::BoundDanceInvocation;
use crate::descriptors::DanceDescriptor;
use crate::query_layer::query_core::{read_input_carrier, QueryReference};
use crate::reference_layer::{HolonReference, ReadableHolon};

const QUERY_DANCE_TYPE_NAME: &str = "QueryDance";

/// True when the bound invocation resolved to the `QueryDance` descriptor.
pub(crate) fn is_query_dance(dance_descriptor: &DanceDescriptor) -> Result<bool, HolonError> {
    Ok(dance_descriptor.header().type_name()?.0 == QUERY_DANCE_TYPE_NAME)
}

/// Routes a validated `QueryDance` invocation onto the direct Query seam and
/// returns the error that route produces.
///
/// Request-shape problems surface as the existing structured contract errors
/// (`MissingRequiredRelationship`, `MultipleRelatedHolons`, `WrongDescriptorKind`);
/// a well-formed request reaches the seam, which in QRY1 records the failed
/// execution and returns `HolonError::NotImplemented`.
pub(crate) fn invoke(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> HolonError {
    match route_to_seam(context, bound_invocation) {
        Ok(never) => match never {},
        Err(error) => error,
    }
}

fn route_to_seam(
    context: &Arc<TransactionContext>,
    bound_invocation: &BoundDanceInvocation,
) -> Result<Infallible, HolonError> {
    let request =
        bound_invocation.request().ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: summarize(bound_invocation.invocation().as_holon_reference()),
        })?;

    let query =
        QueryReference::new(exactly_one(request, QueryDanceRelationshipTypeName::RequestedQuery)?)?;
    let input =
        read_input_carrier(&exactly_one(request, QueryDanceRelationshipTypeName::InitialInput)?)?;
    let bindings = members(request, QueryDanceRelationshipTypeName::RequestParameters)?;

    // The QRY1 seam has no success path; if it ever returns `Ok`, that is a
    // QRY2 change that must also introduce response construction here.
    let _ = query.execute(context, input, bindings)?;
    Err(HolonError::NotImplemented("QueryDance response construction (QRY2)".to_string()))
}

fn members(
    holon: &HolonReference,
    relationship: QueryDanceRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection = holon.related_holons(relationship)?;
    let members = collection
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();
    Ok(members)
}

fn exactly_one(
    holon: &HolonReference,
    relationship: QueryDanceRelationshipTypeName,
) -> Result<HolonReference, HolonError> {
    let relationship_name = relationship.as_relationship_name().to_string();
    let members = members(holon, relationship)?;
    match members.as_slice() {
        [single] => Ok(single.clone()),
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: relationship_name,
            descriptor: summarize(holon),
        }),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: relationship_name,
            descriptor: summarize(holon),
            count: many.len(),
        }),
    }
}

fn summarize(holon: &HolonReference) -> String {
    holon.summarize().unwrap_or_else(|_| holon.reference_id_string())
}
