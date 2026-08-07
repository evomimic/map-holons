//! Deliberately unsupported authoring seams for live Integrity tests.
//!
//! # This is not part of the storage API
//!
//! These probes bypass canonical persistence behavior to author only the operations that typed
//! production APIs deliberately cannot construct. Nothing in production may call them, and every
//! probe must remain narrowly fixed to the Integrity rule its sweettest needs to reach. This zome
//! is the complete, grep-able inventory of unsupported ingress and is built as a loose test
//! artifact; production DNA and hApp manifests do not package it.
//!
//! # Why these probes exist
//!
//! Some Integrity rules reject operations that canonical coordinator APIs make unreachable. A
//! focused raw-authoring seam is therefore required to prove through a real conductor that those
//! rules are wired into peer validation. The rejection is the assertion; a negative-purpose probe
//! whose call succeeds has failed its purpose.
//!
//! # Artifact exclusion and isolated augmentation
//!
//! Production builds package only `holons`; this zome is a distinct loose WASM and is absent from
//! every production DNA and hApp manifest by construction. Probe-dependent tests first install and
//! inspect the unchanged production DNA, then append only this zome to an isolated conductor and
//! verify that DNA identity, Integrity definitions, and the active production coordinator remain
//! unchanged.
//!
//! Keeping this crate independent of `holons_guest` is essential: HDK extern symbols from an rlib
//! dependency survive WASM linking and would otherwise reproduce the entire production coordinator
//! surface in this test artifact.

use hdk::prelude::*;
use holons_guest_integrity::{local_id_from_action_hash, try_action_hash_from_local_id, HolonNode};
use holons_integrity::{EntryTypes, LinkTypes};
use integrity_core_types::{HolonError, HolonNodeModel, LocalId};

/// Projects shared conversion errors at this zome's guest boundary without creating a dependency
/// on the production coordinator crate.
fn to_wasm(error: HolonError) -> WasmError {
    wasm_error!(WasmErrorInner::Guest(error.to_string()))
}

/// Authors an `Update` against an exact action hash, with no lineage resolution.
///
/// Used by the sweettest that proves integrity rejects an update aimed at another update rather
/// than at a lineage-root `Create`, and by its companion that proves the same seam is *accepted*
/// when aimed at a root — together showing the rejection is about topology, not about this
/// function.
///
/// Not a supported write path.
#[hdk_extern]
pub fn holon_storage_author_update_for_test(
    input: (LocalId, HolonNodeModel),
) -> ExternResult<LocalId> {
    let (target_id, holon_node) = input;
    let target_hash = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;

    let action_hash =
        update_entry(target_hash, &EntryTypes::HolonNode(HolonNode::from(holon_node)))?;

    Ok(local_id_from_action_hash(action_hash))
}

/// Authors a raw `HolonNode` create without coordinator preflight.
///
/// This probe exists only to prove through a live conductor that Integrity independently rejects
/// a structurally invalid HolonNode after the canonical production boundary begins rejecting the
/// same model before authoring. It fixes the entry type and exposes no generic raw-entry ingress.
///
/// Not a supported write path.
#[hdk_extern]
pub fn holon_storage_author_create_for_test(holon_node: HolonNodeModel) -> ExternResult<LocalId> {
    let action_hash = create_entry(&EntryTypes::HolonNode(HolonNode::from(holon_node)))?;
    Ok(local_id_from_action_hash(action_hash))
}

/// Authors caller-supplied Tag v1 bytes under the fixed `SmartLink` type.
///
/// This is the minimum seam needed to prove the Integrity adapter rejects malformed peer-authored
/// bytes that the canonical storage encoder will never produce. Not a supported write path.
#[hdk_extern]
pub fn smartlink_author_raw_tag_for_test(
    input: (LocalId, LocalId, Vec<u8>),
) -> ExternResult<LocalId> {
    let (source_id, target_id, raw_tag) = input;
    let source = try_action_hash_from_local_id(&source_id).map_err(to_wasm)?;
    let target = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;
    let action_hash = create_link(source, target, LinkTypes::SmartLink, LinkTag(raw_tag))?;
    Ok(local_id_from_action_hash(action_hash))
}

/// Deletes only a resolved `AllHolonNodes` create-link action.
///
/// The probe refuses every other action and link type, preserving the narrowest possible ingress
/// for the conductor rejection test. Not a supported write path.
#[hdk_extern]
pub fn all_holon_nodes_delete_for_test(create_link_id: LocalId) -> ExternResult<LocalId> {
    let create_link_hash = try_action_hash_from_local_id(&create_link_id).map_err(to_wasm)?;
    let record = get(create_link_hash.clone(), GetOptions::default())?.ok_or_else(|| {
        wasm_error!(WasmErrorInner::Guest("AllHolonNodes test delete target does not exist".into()))
    })?;
    let Action::CreateLink(create_link) = record.action() else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "AllHolonNodes test delete target is not a CreateLink".into()
        )));
    };
    let Some(LinkTypes::AllHolonNodes) =
        LinkTypes::from_type(create_link.zome_index, create_link.link_type)?
    else {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "AllHolonNodes test delete target has another link type".into()
        )));
    };

    let delete_hash = delete_link(create_link_hash, GetOptions::default())?;
    Ok(local_id_from_action_hash(delete_hash))
}
