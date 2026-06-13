use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{DanceDescriptor, DanceResponseDescriptor, Descriptor, HolonDescriptor};
use crate::dances::{DanceInvocationReference, DanceResponseReference};
use crate::reference_layer::{ReadableHolon, WritableHolon};

/// Shared execution seam for canonical `DanceV2` ingress.
///
/// PR4 keeps the implementation static and descriptor-backed. The executor
/// validates the typed invocation boundary, request shape, target affordances,
/// implementation cardinality, and response descriptor shape before minting a
/// new typed response reference.
pub async fn execute_dance_v2(
    context: &Arc<TransactionContext>,
    invocation: DanceInvocationReference,
) -> Result<DanceResponseReference, HolonError> {
    let invocation_holon = invocation.as_holon_reference();
    let dance_descriptor_ref = single_related(invocation_holon, CoreRelationshipTypeName::InvokesDance)?;
    let dance_descriptor = DanceDescriptor::from_holon(dance_descriptor_ref.clone());
    let request_type = dance_descriptor.request_type()?;
    let response_descriptor = dance_descriptor.response()?;

    validate_request_shape(invocation_holon, request_type.as_ref())?;

    if let Some(target) = single_related_opt(invocation_holon, CoreRelationshipTypeName::Target)? {
        validate_target_affords_dance(&target, dance_descriptor.header().type_name()?)?;
    }

    let implementations = dance_descriptor_ref
        .related_holons(&CoreRelationshipTypeName::ForDance)?
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .len();

    if implementations == 0 {
        return Err(HolonError::DescriptorDeclarationNotFound {
            kind: "dance implementation".to_string(),
            name: dance_descriptor.header().type_name()?.to_string(),
            descriptor: dance_descriptor_ref.summarize()?,
        });
    }

    if implementations > 1 {
        return Err(HolonError::DuplicateInheritedDeclaration {
            kind: "dance implementation".to_string(),
            name: dance_descriptor.header().type_name()?.to_string(),
            descriptor: dance_descriptor_ref.summarize()?,
        });
    }

    validate_response_descriptor(&response_descriptor)?;

    let mut response = context
        .mutation()
        .new_holon(Some(MapString("dance-response".to_string())))?;
    response.with_descriptor(response_descriptor.holon().clone())?;

    DanceResponseReference::new(response.into())
}

fn validate_request_shape(
    invocation_holon: &crate::reference_layer::HolonReference,
    request_type: Option<&HolonDescriptor>,
) -> Result<(), HolonError> {
    let request = single_related_opt(invocation_holon, CoreRelationshipTypeName::Request)?;
    match (request_type, request) {
        (Some(_), None) => Err(HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: invocation_holon.summarize()?,
        }),
        (None, Some(_)) => Err(HolonError::InvalidRelationship(
            "Request".to_string(),
            invocation_holon.summarize()?,
        )),
        _ => Ok(()),
    }
}

fn validate_target_affords_dance(
    target: &crate::reference_layer::HolonReference,
    dance_name: MapString,
) -> Result<(), HolonError> {
    let target_descriptor = target.holon_descriptor()?;
    target_descriptor.get_dance_by_name(dance_name)?;
    Ok(())
}

fn validate_response_descriptor(
    response_descriptor: &DanceResponseDescriptor,
) -> Result<(), HolonError> {
    let _ = response_descriptor.response_body()?;
    Ok(())
}

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
