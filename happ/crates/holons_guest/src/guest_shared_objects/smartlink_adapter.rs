//! Temporary coordinator-facing SmartLink facade.
//!
//! Storage SL1 Part 2 (#594) moved the real read/write/delete logic into
//! `persistence_layer::smartlink`. These functions are thin, behavior-preserving
//! shims kept until the facade is retired in SL4. They exist only so existing
//! callers keep compiling against the same surface.

use core_types::{HolonError, HolonId, PreparedSmartLink, PutSmartLinkOutcome};
use hdk::prelude::*;
use holons_guest_integrity::type_conversions::*;
use holons_integrity::LinkTypes;
use integrity_core_types::{LocalId, RelationshipName};

use crate::persistence_layer::{expand_all_from_source, expand_from_source, put_smartlink};

/// The canonical decoded SmartLink now lives in `core_types`; re-export it so the
/// pre-#594 `guest_shared_objects::SmartLink` path keeps resolving.
pub use core_types::SmartLink;

// This link query defaults on all fields; `GetStrategy::default()` performs a network fetch.
pub fn fetch_links_to_all_holons() -> Result<Vec<HolonId>, HolonError> {
    let path = Path::from("all_holon_nodes");
    let base_address = path.path_entry_hash().map_err(holon_error_from_wasm_error)?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::AllHolonNodes)
        .map_err(holon_error_from_wasm_error)?;
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let mut holon_ids = Vec::new();
    info!(
        "Retrieved {:?} links for 'all_holon_nodes' path, converting to SmartLinks..",
        links.len()
    );

    for link in links {
        let holon_id = HolonId::Local(local_id_from_action_hash(
            link.target.clone().into_action_hash().ok_or(HolonError::HashConversion(
                "Source/Base".to_string(),
                "ActionHash".to_string(),
            ))?,
        ));
        holon_ids.push(holon_id);
    }

    Ok(holon_ids)
}

/// Facade shim: all SmartLinks from a source across every relationship.
/// Delegates to the storage-layer expansion API.
pub fn get_all_relationship_links(local_source_id: &LocalId) -> Result<Vec<SmartLink>, HolonError> {
    expand_all_from_source(local_source_id)
}

/// Facade shim: SmartLinks for a specific relationship from this source.
/// Delegates to the storage-layer expansion API.
pub fn get_relationship_links(
    source_action_hash: ActionHash,
    relationship_name: &RelationshipName,
) -> Result<Vec<SmartLink>, HolonError> {
    expand_from_source(&local_id_from_action_hash(source_action_hash), relationship_name)
}

/// Facade shim: persists a SmartLink and returns its create-link action hash.
///
/// Delegates to the storage-layer `put_smartlink`. `Inserted` / `AlreadyPresent`
/// return the physical id (preserving the pre-#594 `ActionHash` return contract). A
/// `Conflict` is surfaced as a **hard error**: a live link already shares this
/// insertion identity but differs in canonical key or authoritative relationship
/// properties, so the requested SmartLink was *not* persisted. Returning success
/// there would let commit Pass 2 continue as if it had been — and legacy DHT rows
/// (the old dedup matched only exact tag bytes) can genuinely be in that state.
pub fn save_smartlink(prepared: PreparedSmartLink) -> Result<ActionHash, HolonError> {
    let source_id = prepared.source_id.clone();
    let target_id = prepared.target_id.clone();
    let relationship_name = prepared.relationship_name.clone();
    let smartlink_id = match put_smartlink(prepared)? {
        PutSmartLinkOutcome::Inserted(id) | PutSmartLinkOutcome::AlreadyPresent(id) => id,
        PutSmartLinkOutcome::Conflict(existing) => {
            return Err(HolonError::CommitFailure(format!(
                "SmartLink conflict: a live link {existing:?} already shares the insertion identity \
                 (source={source_id:?}, target={target_id:?}, relationship={relationship_name:?}) \
                 but differs in canonical key or relationship properties; requested link not persisted"
            )));
        }
    };
    try_action_hash_from_local_id(&smartlink_id.0)
}
