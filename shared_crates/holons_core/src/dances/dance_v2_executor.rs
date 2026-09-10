use std::sync::Arc;

use base_types::MapString;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::{DanceImplementation, DanceInvocation, DanceResponseReference};
use crate::descriptors::{DanceDescriptor, DanceResponseDescriptor, Descriptor};
use crate::reference_layer::{ReadableHolon, WritableHolon};

/// A DanceV2 invocation after its semantic contract and currently available
/// implementation have been resolved.
pub struct ResolvedDanceV2Invocation {
    bound_invocation: crate::dances::BoundDanceInvocation,
    implementation: DanceImplementation,
    response_descriptor: DanceResponseDescriptor,
}

impl ResolvedDanceV2Invocation {
    /// Returns the invocation bound to its descriptor-backed contract.
    pub fn bound_invocation(&self) -> &crate::dances::BoundDanceInvocation {
        &self.bound_invocation
    }

    /// Returns the one currently available implementation selected by policy.
    pub fn implementation(&self) -> &DanceImplementation {
        &self.implementation
    }

    /// Returns the descriptor that governs response construction.
    pub fn response_descriptor(&self) -> &DanceResponseDescriptor {
        &self.response_descriptor
    }
}

/// Executes a descriptor-driven dance invocation and returns a typed response
/// reference.
///
/// The executor acts as a choreographer. It binds the invocation contract,
/// validates it, selects one implementation under the current static policy,
/// invokes it in the current container, and mints a response holon described
/// by the Dance's declared response type. An implementation that requires a
/// host-authoritative or space-authoritative capability delegates that concern
/// to its service or routing boundary.
pub async fn execute_dance_v2(
    context: &Arc<TransactionContext>,
    invocation: DanceInvocation,
) -> Result<DanceResponseReference, HolonError> {
    let resolved = resolve_dance_v2_invocation(invocation)?;
    let response_body = resolved.implementation.invoke(context, &resolved.bound_invocation)?;
    build_resolved_dance_v2_response(context, &resolved, response_body)
}

/// Binds, validates, and resolves the currently available implementation for a
/// DanceV2 invocation without invoking it.
///
/// This shared pipeline is independent of ingress and execution context. It
/// deliberately does not activate a Dancer, route an invocation, or expose
/// implementation-specific infrastructure.
pub fn resolve_dance_v2_invocation(
    invocation: DanceInvocation,
) -> Result<ResolvedDanceV2Invocation, HolonError> {
    let bound_invocation = invocation.bind()?;
    validate_bound_invocation(&bound_invocation)?;
    let implementation = resolve_implementation(&bound_invocation)?;
    let response_descriptor = bound_invocation.response_type()?;
    Ok(ResolvedDanceV2Invocation { bound_invocation, implementation, response_descriptor })
}

/// Constructs the descriptor-governed response for an already invoked Dance.
pub fn build_resolved_dance_v2_response(
    context: &Arc<TransactionContext>,
    resolved: &ResolvedDanceV2Invocation,
    body: Option<crate::reference_layer::HolonReference>,
) -> Result<DanceResponseReference, HolonError> {
    build_response_reference(context, resolved.response_descriptor(), body)
}

/// Resolves an implementation that is currently available for this invocation.
///
/// The invocation is deliberately the resolver input rather than only the
/// Dance descriptor. Dancer-aware resolution may later need its affording
/// Holon, execution context, or routing requirements. This PR retains the
/// existing static policy and does not activate a Dancer during execution.
fn resolve_implementation(
    bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<DanceImplementation, HolonError> {
    resolve_static_implementation(bound_invocation.dance_descriptor())
}

/// Applies the current bundled/static implementation-resolution policy.
///
/// Exactly one `ForDance` candidate must be available. This is deliberately a
/// policy of the current static resolver, not a permanent semantic invariant
/// of a Dance.
fn resolve_static_implementation(
    dance_descriptor: &DanceDescriptor,
) -> Result<DanceImplementation, HolonError> {
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
    let mut response =
        context.mutation().new_holon(Some(MapString("dance-response".to_string())))?;
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
        (None, Some(_)) => Ok(()),
        (Some(expected_type), Some(request_holon)) => {
            if let Some(request_descriptor_ref) = request_holon.get_descriptor()? {
                let request_descriptor =
                    crate::descriptors::HolonDescriptor::from_holon(request_descriptor_ref);
                if request_descriptor.header().type_name()? != expected_type.header().type_name()? {
                    return Err(HolonError::WrongDescriptorKind {
                        expected: expected_type.header().type_name()?.to_string(),
                        found: request_descriptor.header().type_name()?.to_string(),
                        descriptor: request_descriptor.header().type_name()?.to_string(),
                    });
                }
            }

            // TODO: validate declared Projection properties when schema-backed
            // input contracts are ready; descriptor absence is not invalid.
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

fn validate_affording_holon_contract(
    bound_invocation: &crate::dances::BoundDanceInvocation,
) -> Result<(), HolonError> {
    if let Some(affording_descriptor) = bound_invocation.affording_holon_descriptor() {
        affording_descriptor
            .get_dance_by_name(bound_invocation.dance_descriptor().dance_name()?)?;
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
