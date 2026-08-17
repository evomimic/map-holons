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
use holons_guest_integrity::{
    local_id_from_action_hash, try_action_hash_from_local_id, HolonNode, ALL_HOLON_NODES_PATH,
    LOCAL_HOLON_SPACE_PATH,
};
use holons_integrity::{EntryTypes, LinkTypes};
use integrity_core_types::{HolonError, HolonNodeModel, LocalId};

/// Projects shared conversion errors at this zome's guest boundary without creating a dependency
/// on the production coordinator crate.
fn to_wasm(error: HolonError) -> WasmError {
    wasm_error!(WasmErrorInner::Guest(error.to_string()))
}

/// Root-oriented infrastructure link types whose rejected shapes require raw authoring.
///
/// `SmartLink` uses its canonical storage API and therefore cannot be selected through this enum.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum RootIndexLinkType {
    AllHolonNodes,
    LocalHolonSpace,
}

impl RootIndexLinkType {
    fn canonical_path(self) -> &'static str {
        match self {
            Self::AllHolonNodes => ALL_HOLON_NODES_PATH,
            Self::LocalHolonSpace => LOCAL_HOLON_SPACE_PATH,
        }
    }

    fn link_type(self) -> LinkTypes {
        match self {
            Self::AllHolonNodes => LinkTypes::AllHolonNodes,
            Self::LocalHolonSpace => LinkTypes::LocalHolonSpace,
        }
    }
}

/// Input for authoring a root-index link from an intentionally noncanonical base.
#[derive(Serialize, Deserialize, Debug)]
pub struct NonCanonicalBaseInput {
    pub link_type: RootIndexLinkType,
    pub path: String,
    pub target_id: LocalId,
}

/// Input for authoring a canonical root-index link to an `Update` target.
#[derive(Serialize, Deserialize, Debug)]
pub struct RootIndexUpdateTargetInput {
    pub link_type: RootIndexLinkType,
    pub target_id: LocalId,
}

/// Mechanical infrastructure-link create shared by the probes below.
///
/// Fixes the empty tag and derives the Holochain target hash from a semantic `LocalId`.
fn author_infrastructure_link(
    base_path: &str,
    link_type: LinkTypes,
    target_id: &LocalId,
) -> ExternResult<LocalId> {
    let base = Path::from(base_path).path_entry_hash()?;
    let target = try_action_hash_from_local_id(target_id).map_err(to_wasm)?;
    let create_hash = create_link(base, target, link_type, ())?;
    Ok(local_id_from_action_hash(create_hash))
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

/// Authors an `AllHolonNodes` or `LocalHolonSpace` link from a noncanonical path base.
///
/// This is the minimum raw authority needed to exercise `NonCanonicalBase`; production storage
/// fixes both canonical bases internally and cannot construct the rejected operation. The probe
/// refuses the canonical path and expects Integrity to reject the authored link with the selected
/// link type's canonical-base message.
///
/// Not a supported write path.
#[hdk_extern]
pub fn infrastructure_author_noncanonical_base_for_test(
    input: NonCanonicalBaseInput,
) -> ExternResult<LocalId> {
    if input.path == input.link_type.canonical_path() {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "Noncanonical-base probe refuses the canonical infrastructure path".into()
        )));
    }

    author_infrastructure_link(&input.path, input.link_type.link_type(), &input.target_id)
}

/// Authors a canonical root-index link whose target is an existing HolonNode `Update`.
///
/// This is the minimum raw authority needed to exercise `NonRootHolonNodeTarget`; production
/// storage accepts lineage-root IDs for these indexes and cannot construct the rejected operation.
/// The probe refuses any target that is not an `Update` and expects Integrity to reject it as a
/// non-root HolonNode target.
///
/// Not a supported write path.
#[hdk_extern]
pub fn infrastructure_author_update_target_for_test(
    input: RootIndexUpdateTargetInput,
) -> ExternResult<LocalId> {
    let target_hash = try_action_hash_from_local_id(&input.target_id).map_err(to_wasm)?;
    let record = get(target_hash, GetOptions::default())?.ok_or_else(|| {
        wasm_error!(WasmErrorInner::Guest(
            "Infrastructure update-target probe target does not exist".into()
        ))
    })?;
    if !matches!(record.action(), Action::Update(_)) {
        return Err(wasm_error!(WasmErrorInner::Guest(
            "Infrastructure update-target probe requires an Update target".into()
        )));
    }

    author_infrastructure_link(
        input.link_type.canonical_path(),
        input.link_type.link_type(),
        &input.target_id,
    )
}

/// Locates and deletes the canonical `AllHolonNodes` link to one persisted lineage root.
///
/// Setup comes from `holon_storage_persist(PublishRoot)`, the canonical production writer. This is
/// the minimum raw authority needed to exercise `AllHolonNodesDelete`: no production API deletes
/// the index link directly, so the probe derives its fixed base and type from a semantic target ID
/// and expects Integrity to reject deletion.
///
/// Not a supported write path.
#[hdk_extern]
pub fn all_holon_nodes_delete_for_test(target_id: LocalId) -> ExternResult<LocalId> {
    let target_hash = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;
    let base = Path::from(ALL_HOLON_NODES_PATH).path_entry_hash()?;
    let links =
        get_links(LinkQuery::try_new(base, LinkTypes::AllHolonNodes)?, GetStrategy::default())?;
    let create_link_hash = links
        .into_iter()
        .find_map(|link| {
            (link.target.into_action_hash().as_ref() == Some(&target_hash))
                .then_some(link.create_link_hash)
        })
        .ok_or_else(|| {
            wasm_error!(WasmErrorInner::Guest(
                "Canonical AllHolonNodes link for test target does not exist".into()
            ))
        })?;

    let delete_hash = delete_link(create_link_hash, GetOptions::default())?;
    Ok(local_id_from_action_hash(delete_hash))
}
