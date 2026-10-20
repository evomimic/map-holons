//! Isolated authoring seams for live Integrity tests and controlled storage benchmarks.
//!
//! # This is not part of the storage API
//!
//! These probes bypass canonical persistence behavior to author only the operations that typed
//! production APIs deliberately cannot construct. Nothing in production may call them, and every
//! authoring probe must remain narrowly fixed to its isolated test workload. This zome
//! is the complete, grep-able inventory of unsupported ingress and is built as a loose test
//! artifact; production DNA and hApp manifests do not package it.
//!
//! # Why these probes exist
//!
//! Some Integrity rules reject operations that canonical coordinator APIs make unreachable. A
//! focused raw-authoring seam is therefore required to prove through a real conductor that those
//! rules are wired into peer validation. The rejection is the assertion; a negative-purpose probe
//! whose call succeeds has failed its purpose. Performance probes instead compare equivalent
//! decoded results while measuring forced retrieval strategies on bounded valid workloads.
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
//! surface in this test artifact. Retrieval benchmarks compile the storage source module directly
//! to share its implementation without importing production externs.

use hdk::prelude::*;
use holons_guest_integrity::{
    local_id_from_action_hash, try_action_hash_from_local_id, HolonNode, LOCAL_HOLON_SPACE_PATH,
};
use holons_integrity::{EntryTypes, LinkTypes};
use integrity_core_types::{HolonError, HolonNodeModel, LocalId};

/// Projects shared conversion errors at this zome's guest boundary without creating a dependency
/// on the production coordinator crate.
fn to_wasm(error: HolonError) -> WasmError {
    wasm_error!(WasmErrorInner::Guest(error.to_string()))
}

/// Observes authoring, not graph traversal: semantic rejection must append neither
/// node actions nor link actions, even if a later read could hide a partial write.
/// Counting all entry and link creates is deliberately stronger than counting
/// only the requested Book and its SmartLinks.
#[hdk_extern]
pub fn commit_write_counts_for_test(_: ()) -> ExternResult<(u32, u32)> {
    let records = query(ChainQueryFilter::new())?;
    let mut entries = 0;
    let mut links = 0;
    for record in records {
        match record.action() {
            Action::Create(_) | Action::Update(_) => entries += 1,
            Action::CreateLink(_) => links += 1,
            _ => {}
        }
    }
    Ok((entries, links))
}

/// Root-oriented infrastructure link types whose rejected shapes require raw authoring.
///
/// `SmartLink` uses its canonical storage API and therefore cannot be selected through this enum.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum RootIndexLinkType {
    LocalHolonSpace,
}

impl RootIndexLinkType {
    fn canonical_path(self) -> &'static str {
        match self {
            Self::LocalHolonSpace => LOCAL_HOLON_SPACE_PATH,
        }
    }

    fn link_type(self) -> LinkTypes {
        match self {
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

/// Authors a `LocalHolonSpace` link from a noncanonical path base.
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

// Compile the storage implementation directly, without linking holons_guest's externs.
// The benchmark therefore exercises the same retrieval and decoding as production.
#[allow(dead_code)]
#[path = "../../../../crates/holons_guest/src/persistence_layer/smartlink.rs"]
mod benchmark_smartlink;

/// Measures one forced retrieval, excluding transport and result serialization.
/// Returns elapsed microseconds, backend calls, queries, returned links, and exact buckets.
#[hdk_extern]
pub fn smartlink_measure_retrieval_for_test(
    input: (LocalId, Vec<integrity_core_types::RelationshipName>, String),
) -> ExternResult<(u64, u64, u64, u64, Vec<Vec<core_types::SmartLink>>)> {
    use benchmark_smartlink::RelationshipRetrievalStrategy as Strategy;
    let (source, names, strategy) = input;
    let strategy = match strategy.as_str() {
        "batched_prefixes" => Strategy::BatchedPrefixes,
        "consolidated" => Strategy::Consolidated,
        "separate_prefixes" => Strategy::SeparatePrefixes,
        _ => return Err(wasm_error!(WasmErrorInner::Guest("Unknown retrieval strategy".into()))),
    };
    let before = benchmark_smartlink::relationship_read_metrics();
    let started = sys_time()?;
    let buckets =
        benchmark_smartlink::expand_relationship_set_with_strategy(&source, &names, strategy)
            .map_err(to_wasm)?;
    let elapsed = sys_time()?.as_micros().saturating_sub(started.as_micros()) as u64;
    let after = benchmark_smartlink::relationship_read_metrics();
    Ok((elapsed, after.0 - before.0, after.1 - before.1, after.2 - before.2, buckets))
}

/// Seeds bounded, valid storage rows outside every timed retrieval.
#[hdk_extern]
pub fn smartlink_seed_retrieval_for_test(
    input: (LocalId, LocalId, Vec<(integrity_core_types::RelationshipName, u32)>),
) -> ExternResult<u32> {
    use core_types::{
        encode_smartlink_tag, CanonicalKey, HolonId, OccurrenceId, SmartLinkTagInput,
    };
    let (source, target, distribution) = input;
    let total: u64 = distribution.iter().map(|(_, count)| u64::from(*count)).sum();
    if total > 2048 || distribution.len() > 32 {
        return Err(wasm_error!(WasmErrorInner::Guest("Benchmark workload exceeds limit".into())));
    }
    let source_hash = try_action_hash_from_local_id(&source).map_err(to_wasm)?;
    let target_hash = try_action_hash_from_local_id(&target).map_err(to_wasm)?;
    let mut sequence = 0u128;
    for (relationship_name, count) in distribution {
        for _ in 0..count {
            sequence += 1;
            let tag = encode_smartlink_tag(&SmartLinkTagInput {
                target_id: HolonId::Local(target.clone()),
                relationship_name: relationship_name.clone(),
                canonical_key: CanonicalKey::new("benchmark-target").map_err(to_wasm)?,
                occurrence_id: Some(OccurrenceId(sequence.to_le_bytes())),
                relationship_property_values: Default::default(),
                target_property_cache_candidates: Vec::new(),
            })
            .map_err(|error| wasm_error!(WasmErrorInner::Guest(error.to_string())))?;
            create_link(
                source_hash.clone(),
                target_hash.clone(),
                LinkTypes::SmartLink,
                LinkTag(tag),
            )?;
        }
    }
    Ok(total as u32)
}

// The shared storage source's Commit read timers stay inactive in storage-only probes.
#[allow(dead_code)]
#[path = "../../../../crates/holons_guest/src/read_cost.rs"]
mod read_cost;

/// Explicit HDK strategy experiment on an already populated disposable conductor.
/// Times only get_links, not query construction, result comparison, or the zome roundtrip.
#[hdk_extern]
pub fn smartlink_measure_get_strategy_for_test(
    input: (LocalId, integrity_core_types::RelationshipName, u32),
) -> ExternResult<(Vec<(u32, String, u64, usize)>, Vec<String>)> {
    let (source, relationship, samples) = input;
    if !(100..=1000).contains(&samples) {
        return Err(wasm_error!(WasmErrorInner::Guest("Use 100..1000 samples".into())));
    }
    let prefix = core_types::smartlink_relationship_prefix(&relationship)
        .map_err(|error| wasm_error!(WasmErrorInner::Guest(error.to_string())))?;
    let query = LinkQuery::try_new(
        try_action_hash_from_local_id(&source).map_err(to_wasm)?,
        LinkTypes::SmartLink,
    )?
    .tag_prefix(LinkTag(prefix));
    let mut rows = Vec::new();
    let mut expected = None;
    for sample in 0..=samples {
        for position in 0..2 {
            let local = (sample + position) % 2 == 1;
            let strategy = if local { GetStrategy::Local } else { GetStrategy::default() };
            let prepared = query.clone();
            let started = sys_time()?;
            let links = get_links(prepared, strategy)?;
            let elapsed = sys_time()?.as_micros().saturating_sub(started.as_micros()) as u64;
            let mut canonical: Vec<_> = links.iter().map(|link| format!("{link:?}")).collect();
            canonical.sort();
            if let Some(expected) = &expected {
                if *expected != canonical {
                    return Err(wasm_error!(WasmErrorInner::Guest(
                        "Strategy results differ".into()
                    )));
                }
            } else {
                expected = Some(canonical);
            }
            rows.push((
                sample,
                if local { "local" } else { "default" }.into(),
                elapsed,
                links.len(),
            ));
        }
    }
    for sample in 1..=samples {
        let prepared = query.clone();
        let started = sys_time()?;
        let links = get_links(prepared, GetStrategy::Local)?;
        let elapsed = sys_time()?.as_micros().saturating_sub(started.as_micros()) as u64;
        let mut canonical: Vec<_> = links.iter().map(|link| format!("{link:?}")).collect();
        canonical.sort();
        if expected.as_ref() != Some(&canonical) {
            return Err(wasm_error!(WasmErrorInner::Guest("Repeated Local results differ".into())));
        }
        rows.push((sample, "local_repeated".into(), elapsed, links.len()));
    }
    Ok((rows, expected.unwrap_or_default()))
}

/// Rotates an empty link query and trivial host calls within one guest invocation.
#[hdk_extern]
pub fn smartlink_measure_host_baseline_for_test(
    input: (LocalId, integrity_core_types::RelationshipName, u32),
) -> ExternResult<Vec<(u32, String, u64)>> {
    let (source, relationship, samples) = input;
    if !(100..=1000).contains(&samples) {
        return Err(wasm_error!(WasmErrorInner::Guest("Use 100..1000 samples".into())));
    }
    let prefix = core_types::smartlink_relationship_prefix(&relationship)
        .map_err(|error| wasm_error!(WasmErrorInner::Guest(error.to_string())))?;
    let query = LinkQuery::try_new(
        try_action_hash_from_local_id(&source).map_err(to_wasm)?,
        LinkTypes::SmartLink,
    )?
    .tag_prefix(LinkTag(prefix));
    let operations = ["clock_pair", "sys_time", "zome_info", "empty_get_links_local"];
    let mut rows = Vec::new();
    for sample in 0..=samples {
        for offset in 0..4 {
            let operation = operations[((sample + offset) % 4) as usize];
            let prepared = query.clone();
            let started = sys_time()?;
            match operation {
                "sys_time" => {
                    let _ = sys_time()?;
                }
                "zome_info" => {
                    let _ = zome_info()?;
                }
                "empty_get_links_local" => {
                    let links = get_links(prepared, GetStrategy::Local)?;
                    if !links.is_empty() {
                        return Err(wasm_error!(WasmErrorInner::Guest(
                            "Expected an empty link bucket".into()
                        )));
                    }
                }
                _ => {}
            }
            let elapsed = sys_time()?.as_micros().saturating_sub(started.as_micros()) as u64;
            rows.push((sample, operation.into(), elapsed));
        }
    }
    Ok(rows)
}
