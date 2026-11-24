//! Common ingress policy for typed session-registry participants.
use super::SessionStateWire;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use std::sync::Arc;

/// Keep private registry pools separate from public transactions and other spaces.
/// Each participant subsequently validates and indexes its own root and payload.
pub(super) fn validate_registry_owner(
    owner: &SessionStateWire,
    agent: &Arc<TransactionContext>,
) -> Result<TxId, HolonError> {
    if !owner.is_restricted_cache_read()
        || owner.is_bootstrap_provisioning()
        || !owner.get_staged_holons().holons.is_empty()
        || owner
            .get_transient_holons()
            .holons
            .values()
            .any(|holon| !matches!(holon, crate::context_binding::HolonWire::Transient(_)))
    {
        return Err(HolonError::InvalidParameter(
            "Registry owner must be restricted, non-provisioning and unstaged".into(),
        ));
    }
    if owner.get_local_space_holon_wire()
        != agent.get_space_holon()?.map(crate::HolonReferenceWire::from)
    {
        return Err(HolonError::InvalidParameter(
            "Registry owner belongs to a different HolonSpace".into(),
        ));
    }
    let id = owner.get_tx_id().ok_or_else(|| {
        HolonError::InvalidParameter("Registry owner is missing its transaction identity".into())
    })?;
    if id == agent.tx_id() {
        return Err(HolonError::InvalidParameter(
            "Registry owner must be separate from public transaction".into(),
        ));
    }
    Ok(id)
}
