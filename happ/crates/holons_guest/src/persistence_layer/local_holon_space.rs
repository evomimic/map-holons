//! Designation and read-only discovery of the local HolonSpace lineage root.
//!
//! This is an internal storage operation, not a coordinator API: the canonical path, link type, and
//! empty tag are fixed here so no caller can select infrastructure topology.
//!
//! Ordinary whole-space discovery is represented by semantic `Owns` SmartLinks.

use core_types::HolonError;
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, LOCAL_HOLON_SPACE_PATH};
use holons_integrity::LinkTypes;
use integrity_core_types::LocalId;

/// Anchors the canonical LocalHolonSpace path to a persisted lineage-root HolonNode.
///
/// The caller supplies only a semantic `LocalId`; Integrity independently requires that it resolve
/// to a HolonNode lineage-root `Create`.
pub fn index_local_holon_space(lineage_root_id: &LocalId) -> Result<(), HolonError> {
    let base = Path::from(LOCAL_HOLON_SPACE_PATH)
        .path_entry_hash()
        .map_err(holon_error_from_wasm_error)?;
    let target = try_action_hash_from_local_id(lineage_root_id)?;

    create_link(base, target, LinkTypes::LocalHolonSpace, ())
        .map_err(holon_error_from_wasm_error)?;

    Ok(())
}

/// Reads only the executing cell author's local designation; never picks a competing target.
/// A missing or unreadable indexed node is corruption, not permission to provision a replacement.
pub fn discover_local_holon_space_id() -> Result<Option<LocalId>, HolonError> {
    let base = Path::from(LOCAL_HOLON_SPACE_PATH)
        .path_entry_hash()
        .map_err(holon_error_from_wasm_error)?;
    let link_query = LinkQuery::try_new(base, LinkTypes::LocalHolonSpace)
        .map_err(holon_error_from_wasm_error)?;
    let author = agent_info().map_err(holon_error_from_wasm_error)?.agent_initial_pubkey;
    let links = get_links(link_query, GetStrategy::Local).map_err(holon_error_from_wasm_error)?;
    let mut targets = std::collections::HashSet::new();
    for link in links.into_iter().filter(|link| link.author == author) {
        let target = link.target.into_action_hash().ok_or_else(|| {
            HolonError::InvalidState("Local Space index contains a non-action target".into())
        })?;
        targets.insert(target);
    }
    if targets.len() > 1 {
        return Err(HolonError::InvalidState("Local Space index has competing lineages".into()));
    }
    let Some(target) = targets.into_iter().next() else {
        // An absent designation is not proof of a fresh installation after a
        // failed/legacy bootstrap. Inspect authored metadata only, without a DHT
        // enumeration or schema revalidation, before permitting first provisioning.
        let authored = query(ChainQueryFilter::new().action_type(ActionType::Create))
            .map_err(holon_error_from_wasm_error)?;
        if authored
            .iter()
            .any(|record| matches!(record.action().entry_type(), Some(EntryType::App(_))))
        {
            return Err(HolonError::InvalidState(
                "Authored application state exists without a local Space designation".into(),
            ));
        }
        return Ok(None);
    };
    let id = local_id_from_action_hash(target);
    if super::get_holon(&id)?.is_none() {
        return Err(HolonError::InvalidState("Indexed local Space is unavailable".into()));
    }
    if super::has_visible_holon_delete(&id)? != Some(false) {
        return Err(HolonError::InvalidState("Indexed local Space has been deleted".into()));
    }
    Ok(Some(id))
}
