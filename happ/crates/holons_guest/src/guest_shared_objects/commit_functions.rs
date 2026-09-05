use hdk::prelude::*;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use crate::persistence_layer::{
    expand_from_source_by_key, get_holon, persist_holon, put_smartlink_cached,
    SmartLinkWriteContext,
};
use core_types::{HolonWriteRequest, PreparedSmartLink, PutSmartLinkOutcome};

use holons_core::{
    core_shared_objects::{
        holon::state::{AccessType, StagedState},
        Holon, HolonCollection, ReadableHolonState, StagedHolon, WriteableHolonState,
    },
    descriptors::{resolve_inverse_relationship_name, TargetBinding},
    reference_layer::ReadableHolon,
    HolonReference, SmartReference, StagedReference, WritableHolon,
};

use base_types::MapString;
use core_types::{CanonicalKey, HolonError, HolonId, KeyMatch, OccurrenceId};
use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle,
};
use holons_core::reference_layer::TransientReference;
use integrity_core_types::{short_hex, LocalId, PropertyMap, RelationshipName};
use sha2::{Digest, Sha256};
pub use type_names::CorePropertyTypeName::{CommitRequestStatus, CommitsAttempted};
pub use type_names::CoreRelationshipTypeName::{AbandonedHolons, SavedHolons};
pub use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName,
    ToPropertyName, ToRelationshipName,
};

/// Represents the result of attempting to commit a staged holon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    /// The holon was successfully persisted (created or updated).
    Saved {
        /// Whether this commit published a new immutable version that may establish a key.
        establishes_key_index_entry: bool,
    },
    /// The holon was explicitly marked as abandoned and skipped.
    Abandoned,
    /// No persistence action was required (already committed or unchanged).
    NoAction,
}

type RelationshipCollectionSnapshot = Vec<(RelationshipName, Arc<RwLock<HolonCollection>>)>;

/// Temporary aggregate timings for locating relationship-commit hot spots.
///
/// These are intentionally local to one commit invocation: guest state must not
/// outlive the Wasm call, and the measurements must not alter commit behavior.
#[derive(Default)]
struct CommitPerformanceMetrics {
    inverse_resolution_count: usize,
    inverse_resolution_micros: i64,
    smartlink_persist_count: usize,
    smartlink_persist_micros: i64,
}

/// Tracks `(lineage root, key)` entries already observed during this commit.
///
/// This avoids a repeated DHT query when a transaction creates several versions of
/// one lineage, while the exact keyed lookup below preserves idempotence across
/// transactions and concurrent writers.
#[derive(Default)]
struct KeyIndexMaintenance {
    observed_entries: HashSet<(LocalId, MapString)>,
}

fn performance_timestamp_micros() -> Option<i64> {
    sys_time().ok().map(|timestamp| timestamp.as_micros())
}

fn record_elapsed_micros(started_at: Option<i64>, total: &mut i64) {
    if let (Some(started_at), Some(completed_at)) = (started_at, performance_timestamp_micros()) {
        *total += completed_at.saturating_sub(started_at);
    }
}

/// Captures the committed source selected for relationship persistence.
///
/// Relationship SmartLinks are anchored to the committed source selected in Pass 1.
///
/// New creates and version-producing updates commit to a new node id. Graph-only
/// updates commit to the existing persisted source id carried by the staged holon.
struct RelationshipCommitSource {
    source_local_id: LocalId,
    source_key: Option<MapString>,
    source_reference: HolonReference,
}

impl RelationshipCommitSource {
    fn from_committed_staged_holon(
        staged_reference: &StagedReference,
        staged_holon: &StagedHolon,
    ) -> Option<Self> {
        match staged_holon.get_staged_state() {
            StagedState::Committed(local_id) => Some(Self {
                source_local_id: local_id,
                source_key: staged_holon.key(),
                source_reference: staged_reference.into(),
            }),
            _ => None,
        }
    }

    fn source_local_id(&self) -> &LocalId {
        &self.source_local_id
    }
}

/// `commit`
///
/// Executes a two-pass commit of all staged holons and their relationships,
/// returning a **`TransientReference` to a CommitResponseType holon** that
/// reports the outcome of the operation.
///
/// ## What This Function Produces
///
/// Instead of returning a Rust `CommitResponse` struct (the old behavior),
/// this function now **constructs a CommitResponse holon instance** using the
/// holon operations API.
///
/// The returned `TransientReference` points to a `CommitResponseType` holon with:
/// - **Property** `CommitRequestStatus` = `"Complete"` or `"Incomplete"`
/// - **Property** `CommitsAttempted` = number of staged holons
/// - **Relationship** `SavedHolons` → all successfully committed holons
/// - **Relationship** `AbandonedHolons` → all holons skipped or failed
///
/// This holon is a first-class MAP holon and can be returned directly as a
/// dance response or inspected by follow-up guest or client processes.
///
/// ## Process Overview
///
/// The commit proceeds in **two passes**:
///
/// ### **Pass 1 — Commit staged holons**
///
/// Each staged holon is processed according to its `StagedState`:
///
/// - **ForCreate**
///   Persist as a new HolonNode; update the staged holon to `Committed(saved_id)`
///
/// - **ForUpdateGraphOnly**
///   Do not write a node; update to `Committed(existing_source_id)` so Pass 2
///   anchors relationship SmartLinks to the existing persisted node.
///
/// - **ForUpdateNewVersion**
///   Persist as a new HolonNode, stage `Predecessor` from the new version to
///   the prior persisted version, and update to `Committed(new_saved_id)`.
///
/// - **Abandoned**
///   Skipped and added to the `AbandonedHolons` relationship
///
/// - **ForUpdate / Already Committed**
///   No-op
///
/// Any holon that fails to commit:
/// - keeps its original staged state
/// - receives its error in the holon’s internal error list
/// - causes the CommitResponse holon’s `CommitRequestStatus` to be set to `"Incomplete"`
///
/// If **any** holon fails in pass 1, the function returns immediately after
/// building the response holon. **Pass 2 is skipped.**
///
/// ### **Pass 2 — Commit relationships**
///
/// Only executed if pass 1 completes with no failures.
///
/// For each staged holon in `Committed(saved_id)` state:
/// - iterate all staged relationship collections
/// - create SmartLinks for each member (if the target holon has a LocalId)
/// - record any errors into the source staged holon
/// - update the CommitResponse holon to `"Incomplete"` if any error occurs
///
/// If all relationships succeed, the overall result remains `"Complete"`.
///
/// ## Return Value
///
/// Returns:
/// Ok(TransientReference)
/// pointing to the CommitResponse holon created during the process.
///
/// This holon is always returned—even if the commit is partially successful—
/// and contains a complete summary of saved and abandoned items.
///
/// ## Clearing Staged Holons
///
/// If both passes succeed (no `Incomplete` status), the guest holon service
/// is responsible for clearing the staged holon pool afterward.
pub fn commit(
    context: &Arc<TransactionContext>,
    staged_references: &[StagedReference],
) -> Result<TransientReference, HolonError> {
    info!("Entering commit...");

    // Number of staged holons is derived from the provided references.
    let stage_count = staged_references.len() as i64;

    let mut response_reference =
        context.mutation().new_holon(Some(MapString("Commit Response".to_string())))?;
    response_reference
        .with_property_value(CommitRequestStatus, "Complete")?
        .with_property_value(CommitsAttempted, stage_count)?;

    if stage_count < 1 {
        info!("Stage empty, nothing to commit!");
        return Ok(response_reference);
    }

    // Outcome tallies, kept beyond the Pass 1 scope so the commit response can be summarized at
    // every exit — including the incomplete early return, which is the case most worth seeing.
    let mut saved_ids: Vec<LocalId> = Vec::new();
    let mut key_index_source_ids: HashSet<LocalId> = HashSet::new();
    let mut abandoned_count = 0_usize;
    let mut failed_count = 0_usize;

    // === FIRST PASS: Commit Staged Holons ===
    {
        info!("\n\nStarting FIRST PASS... commit staged holons...");

        let mut saved_holons: Vec<HolonReference> = Vec::new();
        let mut abandoned_holons: Vec<HolonReference> = Vec::new();
        let transaction_handle = TransactionContextHandle::new(Arc::clone(context));

        for staged_reference in staged_references {
            staged_reference.is_accessible(AccessType::Commit)?;

            trace!("Committing {:?}", staged_reference.temporary_id());

            match commit_holon(staged_reference, context) {
                Ok(CommitOutcome::Saved { establishes_key_index_entry }) => {
                    let holon_id = staged_reference.holon_id()?;
                    let key_string: MapString = staged_reference.key()?.ok_or_else(|| {
                        HolonError::HolonNotFound("Committed holon has no key".into())
                    })?;
                    saved_ids.push(holon_id.local_id().clone());
                    if establishes_key_index_entry {
                        key_index_source_ids.insert(holon_id.local_id().clone());
                    }
                    let saved_reference = HolonReference::smart_with_key(
                        transaction_handle.clone(),
                        holon_id,
                        key_string,
                    );
                    saved_holons.push(saved_reference);
                }
                Ok(CommitOutcome::Abandoned) => {
                    abandoned_count += 1;
                    // StagedReference → HolonReference via From<&StagedReference>
                    abandoned_holons.push(staged_reference.into());
                }
                Ok(CommitOutcome::NoAction) => {
                    trace!("No action required for {:?}", staged_reference.temporary_id());
                }
                Err(error) => {
                    response_reference.with_property_value(CommitRequestStatus, "Incomplete")?;
                    // A failed holon joins the abandoned collection, so it is counted there to
                    // keep the log summary consistent with the response holon's shape.
                    abandoned_count += 1;
                    failed_count += 1;
                    abandoned_holons.push(staged_reference.into());
                    warn!("Commit failed for {:?}: {:?}", staged_reference.temporary_id(), error);
                }
            }
        }

        // Attach results to the CommitResponse holon
        response_reference.add_related_holons(SavedHolons, saved_holons)?;
        response_reference.add_related_holons(AbandonedHolons, abandoned_holons)?;
    }

    // Check if Pass 1 ended with an incomplete status
    if let Some(status_value) = response_reference.property_value(CommitRequestStatus)? {
        let status_string: String = (&status_value).into();
        if status_string == "Incomplete" {
            info!("Commit Pass 1 incomplete — skipping Pass 2.");
            log_commit_response(
                &status_string,
                stage_count,
                &saved_ids,
                abandoned_count,
                failed_count,
            );
            return Ok(response_reference);
        }
    }

    // === SECOND PASS: Commit relationships ===
    //
    // Snapshot the committed LocalId + relationship collections under a short-lived read lock,
    // then drop the lock before resolving targets / computing keys / saving smartlinks.
    // This avoids re-entrant locking when a relationship includes a self-edge.
    // This context is deliberately narrower than TransactionContext: it represents only the
    // bounded relationship write set for this commit invocation.
    let mut smartlink_write_context = SmartLinkWriteContext::default();
    let mut performance_metrics = CommitPerformanceMetrics::default();
    let owns_index_space_id = resolve_owns_index_space_id(context, staged_references)?;
    let mut key_index_maintenance = KeyIndexMaintenance::default();

    for staged_reference in staged_references {
        let rc_holon = staged_reference.get_holon_to_commit(context)?;

        // 1) Snapshot what we need while holding only a read lock.
        //
        // NOTE: This must NOT early-return from `commit()`; non-staged holons are simply skipped
        // in Pass 2 (relationship persistence is only for staged holons in Committed state).
        let snapshot: Option<(RelationshipCommitSource, RelationshipCollectionSnapshot)> = {
            let holon_read = rc_holon.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon during relationship commit snapshot: {}",
                    e
                ))
            })?;

            match &*holon_read {
                Holon::Staged(staged_holon) => {
                    // Select source anchor before cloning staged relationship work.
                    if let Some(source) = RelationshipCommitSource::from_committed_staged_holon(
                        staged_reference,
                        staged_holon,
                    ) {
                        // Snapshot the commit-eligible relationship collections while cloning only Arcs.
                        let pairs = staged_holon.relationship_collections_for_commit()?;
                        Some((source, pairs))
                    } else {
                        // Only committed holons participate in relationship persistence.
                        None
                    }
                }
                other => {
                    trace!(
                        "Skipping relationship commit for {:?} (not a staged holon: {:?}).",
                        staged_reference.temporary_id(),
                        other
                    );
                    None
                }
            }
        };

        let Some((relationship_source, relationship_collections)) = snapshot else {
            continue;
        };

        // 2) Commit relationships with NO source-holon lock held.
        //
        // We stop at the first relationship error for a given source holon, record it on the holon,
        // and mark the overall response incomplete. This keeps failure handling bounded and avoids
        // cascading secondary errors.
        let mut first_error: Option<HolonError> = None;

        for (name, holon_collection_rc) in relationship_collections {
            debug!("COMMITTING {:#?} relationship", name.0.clone());

            // Resolve descriptor metadata before locking the staged collection.
            // `holon_descriptor()` reads `DescribedBy`, which may be the same
            // collection currently being committed.
            let inverse_resolution_started_at = performance_timestamp_micros();
            let inverse_name = match resolve_inverse_relationship_name(
                &relationship_source.source_reference,
                &name,
            ) {
                Ok(inverse_name) => inverse_name,
                Err(error) => {
                    first_error = Some(error);
                    break;
                }
            };
            performance_metrics.inverse_resolution_count += 1;
            record_elapsed_micros(
                inverse_resolution_started_at,
                &mut performance_metrics.inverse_resolution_micros,
            );

            let holon_collection = holon_collection_rc.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on relationship collection for {}: {}",
                    name.0 .0, e
                ))
            })?;

            if let Err(err) = commit_relationship(
                &relationship_source,
                name.clone(),
                inverse_name,
                &holon_collection,
                &mut smartlink_write_context,
                &mut performance_metrics,
            ) {
                first_error = Some(err);
                break;
            }
        }

        if first_error.is_none()
            && key_index_source_ids.contains(relationship_source.source_local_id())
        {
            if let Err(error) = maintain_owns_key_index(
                &relationship_source,
                &owns_index_space_id,
                &mut key_index_maintenance,
                &mut smartlink_write_context,
                &mut performance_metrics,
            ) {
                first_error = Some(error);
            }
        }

        // 3) If anything failed, re-lock only to attach the error and mark the response incomplete.
        if let Some(error) = first_error {
            let mut holon_write = rc_holon.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on staged holon for relationship error writeback: {}",
                    e
                ))
            })?;

            if let Holon::Staged(staged_holon) = &mut *holon_write {
                staged_holon.add_error(error.clone())?;
            }

            response_reference.with_property_value(CommitRequestStatus, "Incomplete")?;

            warn!(
                "Attempt to commit relationships failed for {:?}: {:?}",
                staged_reference.temporary_id(),
                error
            );
        }
    }

    // Pass 2 can downgrade the status, so read it back rather than assuming "Complete".
    let final_status = match response_reference.property_value(CommitRequestStatus)? {
        Some(status_value) => (&status_value).into(),
        None => "Unknown".to_string(),
    };

    info!("Commit completed: all staged holons processed and commit response constructed.");
    info!(
        "[PERF-674] relationship commit: inverse_resolutions={} inverse_resolution_ms={} smartlink_persists={} smartlink_persist_ms={}",
        performance_metrics.inverse_resolution_count,
        performance_metrics.inverse_resolution_micros / 1_000,
        performance_metrics.smartlink_persist_count,
        performance_metrics.smartlink_persist_micros / 1_000,
    );
    log_commit_response(&final_status, stage_count, &saved_ids, abandoned_count, failed_count);

    // Done — return the CommitResponse holon reference
    Ok(response_reference)
}

/// Logs a one-line summary of the commit response, plus the saved ids behind `debug!`.
///
/// Emitted at every exit from `commit`, so an incomplete commit is never reported more thinly
/// than a successful one. Only ids, counts, and statuses are logged: a commit response can carry
/// user content, and property values have no place in a log.
fn log_commit_response(
    status: &str,
    attempted: i64,
    saved_ids: &[LocalId],
    abandoned_count: usize,
    failed_count: usize,
) {
    info!(
        "Commit response: status={} attempted={} saved={} abandoned={} (of which failed={})",
        status,
        attempted,
        saved_ids.len(),
        abandoned_count,
        failed_count
    );

    if !saved_ids.is_empty() {
        let rendered: Vec<String> = saved_ids.iter().map(|id| short_hex(id, 8)).collect();
        debug!("Commit response saved holons: [{}]", rendered.join(", "));
    }
}

/// Attempts to persist the holon referenced by the given [`StagedReference`].
///
/// This low-level persistence routine determines the holon's current [`StagedState`]
/// and performs the corresponding create or update operation in the DHT. The holon
/// is mutated in place to reflect its new committed state.
///
/// Returns:
/// * `Ok(Saved)` – Holon successfully created or updated.
/// * `Ok(Abandoned)` – Holon was explicitly marked abandoned and skipped.
/// * `Ok(NoAction)` – Holon required no persistence action.
/// * `Err(HolonError)` – Persistence failure or invalid state.
///
/// This function is only invoked from within the guest environment. It is safe and
/// idempotent to call repeatedly; holons already committed or abandoned are skipped.
fn commit_holon(
    staged_reference: &StagedReference,
    context: &Arc<TransactionContext>,
) -> Result<CommitOutcome, HolonError> {
    // Resolve the staged holon from the pool
    let rc_holon = staged_reference.get_holon_to_commit(context)?;
    let mut holon_write = rc_holon.write().map_err(|e| {
        HolonError::FailedToAcquireLock(format!(
            "Failed to acquire write lock on staged holon during commit: {}",
            e
        ))
    })?;

    if let Holon::Staged(staged_holon) = &mut *holon_write {
        let staged_state = staged_holon.get_staged_state();

        match staged_state {
            // === CREATE NEW NODE ============================================================
            StagedState::ForCreate => {
                trace!("StagedState::ForCreate — publishing a new HolonNode lineage");
                staged_holon.prepare_full_relationship_commit_scope()?;
                let stored = persist_holon(HolonWriteRequest::PublishRoot {
                    holon_node: staged_holon.into_node_model(),
                })?;

                info!(
                    "Committed root (Create): version_id={} lineage=self",
                    short_hex(&stored.version_metadata.version_id, 8)
                );

                staged_holon.to_committed(stored.version_metadata.version_id)?;
                Ok(CommitOutcome::Saved { establishes_key_index_entry: true })
            }

            // === GRAPH-ONLY UPDATE ==========================================================
            StagedState::ForUpdateGraphOnly => {
                let source_id = staged_holon.get_versioned_source_id()?;
                staged_holon.prepare_touched_relationship_commit_scope()?;

                info!(
                    "Committed graph-only edit (no action): reusing source anchor {}",
                    short_hex(&source_id, 8)
                );

                staged_holon.to_committed(source_id)?;
                Ok(CommitOutcome::Saved { establishes_key_index_entry: false })
            }

            // === VERSION-PRODUCING UPDATE ==================================================
            StagedState::ForUpdateNewVersion => {
                trace!("StagedState::ForUpdateNewVersion — publishing the next HolonNode version");
                let predecessor_id = staged_holon.get_versioned_source_id()?;
                staged_holon.prepare_full_relationship_commit_scope()?;
                // Storage decides how a version is written: it resolves the lineage this
                // predecessor belongs to and roots the new version there. Immediate-predecessor
                // ordering is not carried by the node — it is staged below as a SmartLink.
                let stored = persist_holon(HolonWriteRequest::PublishVersion {
                    holon_node: staged_holon.into_node_model(),
                    predecessor_ids: vec![predecessor_id.clone()],
                })?;

                // The lineage is logged alongside the predecessor precisely because they differ
                // once a lineage is more than one version deep: the new version is rooted at the
                // lineage, not at the holon it supersedes.
                info!(
                    "Committed version (Update): version_id={} lineage_id={} predecessor={}",
                    short_hex(&stored.version_metadata.version_id, 8),
                    stored.version_metadata.lineage_root(),
                    short_hex(&predecessor_id, 8)
                );

                let new_local_id = stored.version_metadata.version_id;

                if let Err(error) =
                    stage_predecessor_relationship(staged_holon, context, predecessor_id)
                {
                    staged_holon.add_error(error.clone())?;
                    return Err(error);
                }

                staged_holon.to_committed(new_local_id)?;
                Ok(CommitOutcome::Saved { establishes_key_index_entry: true })
            }

            // === ABANDONED HOLON ============================================================
            StagedState::Abandoned => {
                debug!("Skipping commit for Abandoned holon.");
                Ok(CommitOutcome::Abandoned)
            }

            // === ALREADY COMMITTED OR NO-OP ================================================
            StagedState::Committed(_) | StagedState::ForUpdate => {
                debug!(
                    "Skipping commit for holon in {:?} state (no action required)",
                    staged_state
                );
                Ok(CommitOutcome::NoAction)
            }
        }
    } else {
        Err(HolonError::InvalidParameter(format!(
            "Can only commit staged holons, attempted to commit: {:?}",
            holon_write
        )))
    }
}

fn stage_predecessor_relationship(
    staged_holon: &mut StagedHolon,
    context: &Arc<TransactionContext>,
    predecessor_id: LocalId,
) -> Result<(), HolonError> {
    let predecessor_reference = SmartReference::new_from_id(
        TransactionContextHandle::new(Arc::clone(context)),
        HolonId::Local(predecessor_id),
    );

    staged_holon.add_related_holons_with_keys(
        CoreRelationshipTypeName::Predecessor.as_relationship_name(),
        vec![(predecessor_reference.into(), None)],
    )?;

    Ok(())
}

/// Persists one declared relationship collection and its resolved inverse.
fn commit_relationship(
    source: &RelationshipCommitSource,
    name: RelationshipName,
    inverse_name: RelationshipName,
    collection: &HolonCollection,
    smartlink_write_context: &mut SmartLinkWriteContext,
    performance_metrics: &mut CommitPerformanceMetrics,
) -> Result<(), HolonError> {
    collection.is_accessible(AccessType::Commit)?;

    save_smartlinks_for_collection(
        source,
        name.clone(),
        inverse_name,
        collection,
        smartlink_write_context,
        performance_metrics,
    )?;

    Ok(())
}

struct ResolvedRelationshipTarget {
    target_local_id: LocalId,
    target_key: Option<MapString>,
}

/// Returns the key-index entry established by this committed version.
///
/// A lineage root establishes the set-style `Owns` materialization. A successor
/// establishes an index entry only when its key differs from its predecessor.
fn key_index_entry(
    source: &RelationshipCommitSource,
) -> Result<Option<(MapString, bool)>, HolonError> {
    let Some(key) = source.source_key.clone() else {
        return Ok(None);
    };

    let Some(predecessor) = source.source_reference.predecessor()? else {
        return Ok(Some((key, true)));
    };

    if predecessor.key()? == Some(key.clone()) {
        Ok(None)
    } else {
        Ok(Some((key, false)))
    }
}

/// Resolves the owning space for keyed `Owns` index maintenance.
///
/// Ordinary commits use the persisted local space. Bootstrap is the sole
/// exception: its space becomes local only after Commit succeeds, so the
/// already-committed staged CoreSchemaSpace is used as the anchor source.
fn resolve_owns_index_space_id(
    context: &Arc<TransactionContext>,
    staged_references: &[StagedReference],
) -> Result<LocalId, HolonError> {
    if let Some(space) = context.get_space_holon()? {
        return Ok(space.holon_id()?.local_id().clone());
    }

    if context.is_bootstrap_provisioning() {
        return staged_references
            .iter()
            .find(|reference| {
                reference.key().ok().flatten().is_some_and(|key| key.0 == "MAP.CoreSchemaSpace")
            })
            .ok_or_else(|| {
                HolonError::InvalidState(
                    "Core Schema bootstrap did not commit MAP.CoreSchemaSpace".to_string(),
                )
            })?
            .holon_id()
            .map(|id| id.local_id().clone());
    }

    Err(HolonError::InvalidState(
        "keyed Owns index maintenance requires a persisted HolonSpace".to_string(),
    ))
}

/// Produces the deterministic occurrence identity of an additional keyed `Owns` index entry.
///
/// The root's ordinary `Owns` materialization remains occurrence-free. Each distinct
/// successor key needs a second physical SmartLink to the same root, so it uses a stable
/// occurrence identity derived solely from that canonical key. Storage remains
/// descriptor-agnostic; this is a coordinator-level indexing decision.
fn owns_key_index_occurrence_id(key: &MapString) -> OccurrenceId {
    let mut hasher = Sha256::new();
    hasher.update(b"map-holons:owns-key-index:v1\0");
    hasher.update(key.0.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    OccurrenceId(bytes)
}

/// Ensures the keyed `Owns` discovery entry for a newly established lineage key.
///
/// This runs after semantic relationship persistence. It intentionally writes only the
/// space-to-root index direction: no inverse relationship is materialized for index metadata.
fn maintain_owns_key_index(
    source: &RelationshipCommitSource,
    space_id: &LocalId,
    maintenance: &mut KeyIndexMaintenance,
    smartlink_write_context: &mut SmartLinkWriteContext,
    performance_metrics: &mut CommitPerformanceMetrics,
) -> Result<(), HolonError> {
    let Some((key, is_lineage_root)) = key_index_entry(source)? else {
        return Ok(());
    };

    let lineage_root =
        materialize_target_binding(source.source_local_id(), TargetBinding::Lineage)?;
    if !maintenance.observed_entries.insert((lineage_root.clone(), key.clone())) {
        return Ok(());
    }

    let canonical_key = CanonicalKey::new(key.0.clone())?;
    let owns = CoreRelationshipTypeName::Owns.as_relationship_name();
    let existing =
        expand_from_source_by_key(space_id, &owns, KeyMatch::Exact(canonical_key.clone()))?;
    if existing.into_iter().any(|link| link.target_id == HolonId::Local(lineage_root.clone())) {
        return Ok(());
    }

    let keyed_owns = PreparedSmartLink {
        source_id: space_id.clone(),
        target_id: HolonId::Local(lineage_root),
        relationship_name: owns,
        canonical_key,
        occurrence_id: (!is_lineage_root).then(|| owns_key_index_occurrence_id(&key)),
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    };
    persist_smartlink(smartlink_write_context, keyed_owns, performance_metrics)
}

/// Resolves the physical SmartLink target selected by a completed descriptor binding.
///
/// Storage remains descriptor-agnostic: relationship commit resolves a lineage root before
/// constructing `PreparedSmartLink`, so a lineage-bound occurrence can never be represented by
/// a descendant-targeted link.
fn materialize_target_binding(
    target_id: &LocalId,
    binding: TargetBinding,
) -> Result<LocalId, HolonError> {
    match binding {
        TargetBinding::Version => Ok(target_id.clone()),
        TargetBinding::Lineage => get_holon(target_id)?
            .ok_or_else(|| HolonError::HolonNotFound(target_id.to_string()))
            .map(|stored| stored.version_metadata.lineage_root().as_local_id().clone()),
    }
}

/// Creates local forward and inverse SmartLinks for each member in `collection`.
///
/// Current behavior is fail-fast at the collection level: if any member cannot
/// resolve a persisted `holon_id` or key metadata, this function returns that
/// error immediately and no later members in the same collection are processed.
/// In practice, an abandoned staged target can therefore prevent otherwise
/// valid sibling links in the same relationship collection from being
/// persisted during this commit pass.
///
/// This guarantees local inverse materialization only. Cross-space inverse
/// propagation is outside this function's responsibility and belongs at the
/// outbound-proxy boundary represented by `cache_request_router.rs`.
fn save_smartlinks_for_collection(
    source: &RelationshipCommitSource,
    name: RelationshipName,
    inverse_name: RelationshipName,
    collection: &HolonCollection,
    smartlink_write_context: &mut SmartLinkWriteContext,
    performance_metrics: &mut CommitPerformanceMetrics,
) -> Result<(), HolonError> {
    let relationship_descriptor = source
        .source_reference
        .holon_descriptor()?
        .get_relationship_by_name(name.clone())?
        .try_into_declared_relationship_descriptor()?;
    let forward_binding = relationship_descriptor.target_binding()?;
    let inverse_binding = relationship_descriptor.required_inverse()?.target_binding()?;
    let source_id = source.source_local_id();
    debug!(
        "Calling commit on each HOLON_REFERENCE in the collection for [source_id {:?}]->{:#?}.",
        source_id,
        name.0 .0.clone()
    );
    trace!(
        "Relationship source ref_kind={} ref_id={} source_key={:?}",
        source.source_reference.reference_kind_string(),
        source.source_reference.reference_id_string(),
        source.source_key
    );

    let members = collection.get_members();
    debug!("Relationship {:?} has {} members to commit", name.0 .0, members.len());

    // Resolve targets first so schema and endpoint errors are reported before
    // any SmartLink write for this relationship collection.
    let mut resolved_targets = Vec::with_capacity(members.len());
    for (target_index, holon_reference) in members.iter().enumerate() {
        // Avoid deep Debug formatting here because runtime-bound references can recurse heavily in wasm.
        debug!(
            "Target index={} ref_kind={} ref_id={}",
            target_index,
            holon_reference.reference_kind_string(),
            holon_reference.reference_id_string()
        );

        let target_id = match holon_reference.holon_id() {
            Ok(id) => {
                debug!("Resolved holon_id for index {}: {:?}", target_index, id);
                id
            }
            Err(err) => {
                warn!(
                    "Failed to get holon_id for relationship {:?} at index {}: {:?}",
                    name.0 .0, target_index, err
                );
                return Err(err);
            }
        };
        let target_local_id = require_local_target(&name, target_index, target_id)?;

        let key_option = match holon_reference.key() {
            Ok(k) => {
                debug!("Resolved key for index {}: {:?}", target_index, k);
                k
            }
            Err(err) => {
                error!(
                    "Error getting key for relationship {:?} at index {}: {:?}",
                    name.0 .0, target_index, err
                );
                return Err(err);
            }
        };

        resolved_targets
            .push(ResolvedRelationshipTarget { target_local_id, target_key: key_option });
    }

    let inverse_canonical_key = canonical_key_from_optional_key(source.source_key.clone())?;

    for (target_index, resolved_target) in resolved_targets.iter().enumerate() {
        // Persist both directions from one resolved endpoint pair so the forward
        // and inverse SmartLinks stay anchored to the same committed source.
        //
        // Both directions pass `occurrence_id: None` because commit processing only
        // emits set-style links today. That is a coordinator choice, not a storage
        // limitation: storage persists supplied occurrences as distinct identities
        // (Storage SL3). Assigning and pairing them is deferred coordinator work.
        let forward_target_id =
            materialize_target_binding(&resolved_target.target_local_id, forward_binding)?;
        let forward_smartlink = PreparedSmartLink {
            source_id: source_id.clone(),
            target_id: HolonId::Local(forward_target_id),
            relationship_name: name.clone(),
            canonical_key: canonical_key_from_optional_key(resolved_target.target_key.clone())?,
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        };

        debug!(
            "saving smartlink (idx={}): relationship={:?}, source={:?}, target={:?}",
            target_index, name.0 .0, source_id, forward_smartlink.target_id
        );
        persist_smartlink(smartlink_write_context, forward_smartlink, performance_metrics)?;

        let inverse_target_id = materialize_target_binding(source_id, inverse_binding)?;
        let inverse_smartlink = PreparedSmartLink {
            source_id: resolved_target.target_local_id.clone(),
            target_id: HolonId::Local(inverse_target_id),
            relationship_name: inverse_name.clone(),
            canonical_key: inverse_canonical_key.clone(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        };

        debug!(
            "saving inverse smartlink (idx={}): relationship={:?}, source={:?}, target={:?}",
            target_index, inverse_name.0 .0, resolved_target.target_local_id, source_id
        );
        persist_smartlink(smartlink_write_context, inverse_smartlink, performance_metrics)?;
    }

    Ok(())
}

/// Persists one SmartLink through the storage API and applies commit's outcome policy.
///
/// `Inserted` and `AlreadyPresent` are both success: the requested link is live either
/// way, and commit needs no identity back from it. A `Conflict` is an error: a live link
/// already shares this insertion identity but differs in canonical key or authoritative
/// relationship properties, so the requested SmartLink was *not* persisted. Pass 2 turns
/// that error into an `Incomplete` commit response rather than a failed commit — the
/// holon itself is already persisted by then. Reporting success instead would claim a
/// relationship was committed when it was not.
///
/// This is deliberately defensive, and the window is narrow. Staging hydrates a holon's
/// persisted relationships, and `add_related_holons` treats an entry whose reference
/// identity already exists as an idempotent no-op, so a link that is live *before*
/// staging never reaches this function at all. A conflict therefore requires the
/// colliding link to appear between staging and commit — a concurrent writer, or a
/// legacy DHT row surfacing under the old dedup, which matched only exact tag bytes.
///
/// Note that no write happens either way: `put_smartlink` declines before authoring. The
/// entire observable effect of this arm is the reported outcome, which is why
/// `commit_conflict_tests.rs` asserts the response status rather than the DHT state.
fn persist_smartlink(
    smartlink_write_context: &mut SmartLinkWriteContext,
    prepared: PreparedSmartLink,
    performance_metrics: &mut CommitPerformanceMetrics,
) -> Result<(), HolonError> {
    let source_id = prepared.source_id.clone();
    let target_id = prepared.target_id.clone();
    let relationship_name = prepared.relationship_name.clone();

    let started_at = performance_timestamp_micros();
    let outcome = put_smartlink_cached(smartlink_write_context, prepared);
    performance_metrics.smartlink_persist_count += 1;
    record_elapsed_micros(started_at, &mut performance_metrics.smartlink_persist_micros);

    match outcome? {
        PutSmartLinkOutcome::Inserted(_) | PutSmartLinkOutcome::AlreadyPresent(_) => Ok(()),
        PutSmartLinkOutcome::Conflict(existing) => Err(HolonError::CommitFailure(format!(
            "SmartLink conflict: a live link {existing:?} already shares the insertion identity \
             (source={source_id:?}, target={target_id:?}, relationship={relationship_name:?}) \
             but differs in canonical key or relationship properties; requested link not persisted"
        ))),
    }
}

/// Requires relationship targets to be local before SmartLink persistence.
///
/// Issue 442 guarantees local inverse SmartLink materialization only. External
/// targets require multi-space resolution and cross-space inverse propagation at
/// the outbound-proxy seam in `cache_request_router.rs`, which is not implemented.
fn require_local_target(
    relationship_name: &RelationshipName,
    target_index: usize,
    target_id: HolonId,
) -> Result<LocalId, HolonError> {
    match target_id {
        HolonId::Local(local_id) => Ok(local_id),
        HolonId::External(external_id) => Err(HolonError::NotImplemented(format!(
            "Multi-space relationship persistence is not implemented for relationship {:?} \
             target index {} ({})",
            relationship_name.0 .0, target_index, external_id
        ))),
    }
}

fn canonical_key_from_optional_key(key: Option<MapString>) -> Result<CanonicalKey, HolonError> {
    CanonicalKey::new(key.map_or_else(String::new, |value| value.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_key_index_occurrence_is_stable_per_key_and_distinct_between_keys() {
        let alpha = MapString("lineage.alpha".to_string());
        let beta = MapString("lineage.beta".to_string());

        assert_eq!(owns_key_index_occurrence_id(&alpha), owns_key_index_occurrence_id(&alpha));
        assert_ne!(owns_key_index_occurrence_id(&alpha), owns_key_index_occurrence_id(&beta));
    }
}
