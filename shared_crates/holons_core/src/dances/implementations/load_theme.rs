//! Theme-afforded canonical loading with execution-context service delegation.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::dances::{BoundDanceInvocation, DanceInvocation, RequiredExecutionContext};
use crate::{HolonReference, WritableHolon};
use core_types::HolonError;
use std::sync::Arc;
use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, ThemeTypeName,
};

/// Construct fixed invocation metadata without reading any Theme graph.
pub fn invocation(
    context: &Arc<TransactionContext>,
    theme: HolonReference,
) -> Result<DanceInvocation, HolonError> {
    let descriptor = context.lookup().get_saved_holon_by_key(
        &CoreHolonTypeName::DanceInvocation.descriptor_key(&CoreHolonTypeName::HolonType),
    )?;
    let mut invocation = context.mutation().new_holon(None)?;
    invocation
        .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor.into()])?;
    if let Some(space) = context.get_space_holon()? {
        invocation.add_related_holons(CoreRelationshipTypeName::OwnedBy, vec![space])?;
    }
    invocation
        .with_property_value(CorePropertyTypeName::DanceName, ThemeTypeName::LoadTheme.name())?;
    invocation.add_related_holons(CoreRelationshipTypeName::AffordingHolon, vec![theme])?;
    DanceInvocation::new(invocation.into())
}
/// Host services forward this operation; only the guest service resolves its graph.
pub fn invoke(
    context: &Arc<TransactionContext>,
    bound: &BoundDanceInvocation,
) -> Result<Option<HolonReference>, HolonError> {
    if bound.required_execution_context() != RequiredExecutionContext::SpaceAuthoritative {
        return Err(HolonError::InvalidParameter("LoadTheme requires Space authority".into()));
    }
    let theme = bound.affording_holon().ok_or_else(|| {
        HolonError::InvalidParameter("LoadTheme requires an affording Theme".into())
    })?;
    context.load_theme(theme).map(Some)
}
