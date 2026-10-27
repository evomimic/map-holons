//! Space-authoritative construction of a retained transient schema result.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::{BoundDanceInvocation, DanceInvocation, RequiredExecutionContext};
use crate::descriptors::resolve_core_descriptor;
use crate::{HolonReference, ReadableHolon, WritableHolon};
use core_types::HolonError;
use std::sync::Arc;
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

/// Builds the ordinary typed request and invocation in the caller's transaction.
pub fn invocation(
    context: &Arc<TransactionContext>,
    target: HolonReference,
) -> Result<DanceInvocation, HolonError> {
    let space = context.get_space_holon()?.ok_or_else(|| {
        HolonError::InvalidParameter(
            "ConstructEffectiveDescriptor requires a persisted HolonSpace".into(),
        )
    })?;
    let request_type = resolve_core_descriptor(
        context,
        &CoreHolonTypeName::ConstructEffectiveDescriptorRequest
            .descriptor_key(&CoreHolonTypeName::HolonType)
            .0,
    )?;
    let invocation_type = resolve_core_descriptor(
        context,
        &CoreHolonTypeName::DanceInvocation.descriptor_key(&CoreHolonTypeName::HolonType).0,
    )?;
    let mut request = context.mutation().new_holon(None)?;
    request.add_related_references(CoreRelationshipTypeName::DescribedBy, vec![request_type])?;
    request.add_related_references(CoreRelationshipTypeName::OwnedBy, vec![space.clone()])?;
    request.add_related_references(CoreRelationshipTypeName::RequestedHolonType, vec![target])?;
    let mut invocation = context.mutation().new_holon(None)?;
    invocation
        .add_related_references(CoreRelationshipTypeName::DescribedBy, vec![invocation_type])?;
    invocation.add_related_references(CoreRelationshipTypeName::OwnedBy, vec![space.clone()])?;
    invocation.with_property_value(
        CorePropertyTypeName::DanceName,
        CoreHolonTypeName::ConstructEffectiveDescriptor.as_holon_name(),
    )?;
    invocation.add_related_references(CoreRelationshipTypeName::AffordingHolon, vec![space])?;
    invocation.add_related_references(CoreRelationshipTypeName::Request, vec![request.into()])?;
    DanceInvocation::new(invocation.into())
}

/// Executes after ordinary Dance binding, request validation and static selection.
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    if bound.required_execution_context() != RequiredExecutionContext::SpaceAuthoritative {
        return Err(HolonError::InvalidParameter(
            "ConstructEffectiveDescriptor requires Space authority".into(),
        ));
    }
    let space = context
        .get_space_holon()?
        .ok_or_else(|| HolonError::InvalidParameter("Missing HolonSpace".into()))?;
    let affording = bound
        .affording_holon()
        .ok_or_else(|| HolonError::InvalidParameter("Missing affording HolonSpace".into()))?;
    if affording.holon_id()? != space.holon_id()? {
        return Err(HolonError::InvalidParameter(
            "Construction must be afforded by this transaction's HolonSpace".into(),
        ));
    }
    let request = bound
        .request()
        .ok_or_else(|| HolonError::InvalidParameter("Missing construction request".into()))?;
    crate::descriptors::require_holon_type(
        request,
        &CoreHolonTypeName::ConstructEffectiveDescriptorRequest.as_holon_name(),
    )?;
    let collection = request.related_holons(CoreRelationshipTypeName::RequestedHolonType)?;
    let members = collection
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone();
    let [target] = members.as_slice() else {
        return Err(HolonError::InvalidParameter(
            "RequestedHolonType must have exactly one target".into(),
        ));
    };
    Ok(Some(context.construct_effective_descriptor(target.clone())?.holon().clone()))
}
