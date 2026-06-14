use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{
    DanceDescriptor, DanceResponseDescriptor, Descriptor,
};
use crate::dances::{DanceImplementation, DanceInvocation, DanceResponseReference};
use crate::reference_layer::{ReadableHolon, WritableHolon};

/// Executes a descriptor-driven dance invocation and returns a typed response
/// reference.
///
/// The executor acts as a choreographer. It binds the invocation into a
/// resolved execution context, validates the descriptor-backed contract,
/// selects one implementation, invokes it, and mints a response holon
/// described by the dance's declared response type. This behavior follows the
/// host-side dance execution model described in `dances-design-spec`.
pub async fn execute_dance_v2(
    context: &Arc<TransactionContext>,
    invocation: DanceInvocation,
) -> Result<DanceResponseReference, HolonError> {
    let bound_invocation = invocation.bind()?;
    validate_bound_invocation(&bound_invocation)?;
    let implementation = select_implementation(bound_invocation.dance_descriptor())?;
    let response_descriptor = bound_invocation.response_type()?;
    let response_body = implementation.invoke(context, &bound_invocation)?;
    build_response_reference(context, &response_descriptor, response_body)
}

fn select_implementation(
    dance_descriptor: &DanceDescriptor,
) -> Result<DanceImplementation, HolonError> {
    // Static execution currently requires exactly one `ForDance`
    // implementation. Broader activation and selection policy can be layered
    // onto this seam later without changing the executor entry point.
    let implementations = dance_descriptor.implementation_candidates()?;

    match implementations.as_slice() {
        [] => Err(HolonError::DescriptorDeclarationNotFound {
            kind: "dance implementation".to_string(),
            name: dance_descriptor.header().type_name()?.to_string(),
            descriptor: dance_descriptor.holon().summarize()?,
        }),
        [single] => Ok(single.clone()),
        many => Err(HolonError::DuplicateInheritedDeclaration {
            kind: "dance implementation".to_string(),
            name: dance_descriptor.header().type_name()?.to_string(),
            descriptor: format!(
                "{} ({} candidates)",
                dance_descriptor.holon().summarize()?,
                many.len()
            ),
        }),
    }
}

fn build_response_reference(
    context: &Arc<TransactionContext>,
    response_descriptor: &DanceResponseDescriptor,
    body: Option<crate::reference_layer::HolonReference>,
) -> Result<DanceResponseReference, HolonError> {
    let mut response = context
        .mutation()
        .new_holon(Some(MapString("dance-response".to_string())))?;
    response.with_descriptor(response_descriptor.holon().clone())?;
    if let Some(body_ref) = body {
        response_descriptor.attach_response_body(&mut response, body_ref)?;
    }
    DanceResponseReference::new(response.into())
}

fn validate_bound_invocation(
    bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<(), HolonError> {
    validate_request_contract(bound_invocation)?;
    validate_affording_holon_contract(bound_invocation)?;
    validate_invocation_source(bound_invocation)?;
    validate_response_descriptor(&bound_invocation.response_type()?)?;
    Ok(())
}

fn validate_request_contract(
    bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<(), HolonError> {
    match (bound_invocation.request_type(), bound_invocation.request()) {
        (Some(_), None) => Err(HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: bound_invocation.invocation().as_holon_reference().summarize()?,
        }),
        (None, Some(_)) => Err(HolonError::InvalidRelationship(
            "Request".to_string(),
            bound_invocation.invocation().as_holon_reference().summarize()?,
        )),
        (Some(expected_type), Some(request_holon)) => {
            let request_descriptor = request_holon.holon_descriptor()?;
            if request_descriptor.header().type_name()? != expected_type.header().type_name()? {
                return Err(HolonError::WrongDescriptorKind {
                    expected: expected_type.header().type_name()?.to_string(),
                    found: request_descriptor.header().type_name()?.to_string(),
                    descriptor: request_descriptor.header().type_name()?.to_string(),
                });
            }

            // TODO: the current schema models `RequestType` as a generic
            // `HolonType`. A dedicated request-contract descriptor would allow
            // richer validation than simple descriptor identity matching.
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

fn validate_affording_holon_contract(
    bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<(), HolonError> {
    if let Some(affording_descriptor) = bound_invocation.affording_holon_descriptor() {
        affording_descriptor.get_dance_by_name(bound_invocation.dance_descriptor().dance_name()?)?;
    }

    // TODO: once the schema exposes an explicit affording-holon requirement,
    // validate required/optional/forbidden subject presence here instead of
    // treating affordance validation as purely opportunistic.
    Ok(())
}

fn validate_invocation_source(
    _bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<(), HolonError> {
    // Binding parses the enum value eagerly. If binding succeeded, the source
    // is structurally valid for this execution posture.
    Ok(())
}

fn validate_response_descriptor(
    response_descriptor: &DanceResponseDescriptor,
) -> Result<(), HolonError> {
    let _ = response_descriptor.response_body()?;
    Ok(())
}

// TODO: move single-related cardinality helpers onto `ReadableHolon` (or an
// adjacent shared reference-layer surface) instead of keeping them local here.
#[allow(dead_code)]
fn single_related(
    holon: &crate::reference_layer::HolonReference,
    relationship: CoreRelationshipTypeName,
) -> Result<crate::reference_layer::HolonReference, HolonError> {
    let relationship_name = relationship.as_relationship_name().to_string();
    let collection = holon.related_holons(&relationship)?;
    let members = collection
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();

    match members.as_slice() {
        [single] => Ok(single.clone()),
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
        }),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
            count: many.len(),
        }),
    }
}

// TODO: move optional single-related cardinality helpers onto `ReadableHolon`
// (or an adjacent shared reference-layer surface) instead of keeping them
// local here.
#[allow(dead_code)]
fn single_related_opt(
    holon: &crate::reference_layer::HolonReference,
    relationship: CoreRelationshipTypeName,
) -> Result<Option<crate::reference_layer::HolonReference>, HolonError> {
    let relationship_name = relationship.as_relationship_name().to_string();
    let collection = holon.related_holons(&relationship)?;
    let members = collection
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();

    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
            count: many.len(),
        }),
    }
}
