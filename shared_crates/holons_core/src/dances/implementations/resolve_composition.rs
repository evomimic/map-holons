//! One canonical guest operation for the selected owner's requested composition.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::dahn::composition::CompositionRequest;
use crate::dahn::defaults::{optional, related};
use crate::dances::{BoundDanceInvocation, DanceInvocation, RequiredExecutionContext};
use crate::{HolonReference, WritableHolon};
use core_types::HolonError;
use std::sync::Arc;
use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName as C, DahnHolonTypeName,
    DahnRelationshipTypeName as R, ToRelationshipName,
};
fn one(source: &HolonReference, name: R) -> Result<HolonReference, HolonError> {
    match related(source, name)?.as_slice() {
        [value] => Ok(value.clone()),
        _ => Err(HolonError::InvalidParameter(format!("Expected one {name:?}"))),
    }
}
pub fn invocation(
    context: &Arc<TransactionContext>,
    request: CompositionRequest,
) -> Result<DanceInvocation, HolonError> {
    let descriptor = context
        .lookup()
        .get_saved_holon_by_key(&DahnHolonTypeName::ResolveCompositionRequest.descriptor_key())?;
    let mut body = context.mutation().new_holon(None)?;
    body.add_related_holons(C::DescribedBy, vec![descriptor.into()])?;
    body.add_related_holons(R::CompositionSubject.to_relationship_name(), vec![request.subject])?;
    body.add_related_holons(R::CompositionTheme.to_relationship_name(), vec![request.theme])?;
    if let Some(slot) = request.demand_slot {
        body.add_related_holons(R::RequestedCompositionSlot.to_relationship_name(), vec![slot])?;
    }
    if let Some(source) = request.collection_source {
        body.add_related_holons(R::SelectedCollectionSource.to_relationship_name(), vec![source])?;
    }
    let descriptor = context.lookup().get_saved_holon_by_key(
        &CoreHolonTypeName::DanceInvocation.descriptor_key(&CoreHolonTypeName::HolonType),
    )?;
    let mut invocation = context.mutation().new_holon(None)?;
    invocation.add_related_holons(C::DescribedBy, vec![descriptor.into()])?;
    if let Some(space) = context.get_space_holon()? {
        invocation.add_related_holons(C::OwnedBy, vec![space.clone()])?;
        body.add_related_holons(C::OwnedBy, vec![space])?;
    }
    invocation.with_property_value(CorePropertyTypeName::DanceName, "ResolveComposition")?;
    invocation.add_related_holons(C::AffordingHolon, vec![request.owner])?;
    invocation.add_related_holons(C::Request, vec![body.into()])?;
    DanceInvocation::new(invocation.into())
}
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    if bound.required_execution_context() != RequiredExecutionContext::SpaceAuthoritative {
        return Err(HolonError::InvalidParameter(
            "ResolveComposition requires Space authority".into(),
        ));
    }
    let owner = bound
        .affording_holon()
        .cloned()
        .ok_or_else(|| HolonError::InvalidParameter("Missing composition owner".into()))?;
    let request = bound
        .request()
        .ok_or_else(|| HolonError::InvalidParameter("Missing composition request".into()))?;
    context
        .resolve_composition(CompositionRequest {
            owner,
            subject: one(request, R::CompositionSubject)?,
            theme: one(request, R::CompositionTheme)?,
            demand_slot: optional(request, R::RequestedCompositionSlot)?,
            collection_source: optional(request, R::SelectedCollectionSource)?,
        })
        .map(Some)
}
