use hdk::prelude::*;
use holons_guest_integrity::type_conversions::*;
use holons_guest_integrity::HolonNode;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::guest_shared_objects::{save_smartlink, SmartLink};
use crate::persistence_layer::create_holon_node;

use holons_core::{
    core_shared_objects::{
        holon::state::{AccessType, StagedState},
        Holon, HolonCollection, ReadableHolonState, StagedHolon, WriteableHolonState,
    },
    descriptors::resolve_inverse_relationship_name,
    reference_layer::ReadableHolon,
    HolonCollectionApi, HolonReference, SmartReference, StagedReference, WritableHolon,
};

use base_types::{BaseValue, MapString};
use core_types::{HolonError, HolonId};
use holons_core::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle,
};
use holons_core::reference_layer::TransientReference;
use integrity_core_types::{LocalId, PropertyMap, PropertyName, RelationshipName};
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
    Saved,
    /// The holon was explicitly marked as abandoned and skipped.
    Abandoned,
    /// No persistence action was required (already committed or unchanged).
    NoAction,
}

type RelationshipCollectionSnapshot = Vec<(RelationshipName, Arc<RwLock<HolonCollection>>)>;

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

    // === FIRST PASS: Commit Staged Holons ===
    {
        info!("\n\nStarting FIRST PASS... commit staged holons...");

        let mut saved_holons: Vec<HolonReference> = Vec::new();
        let mut abandoned_holons: Vec<HolonReference> = Vec::new();
        let transaction_handle = TransactionContextHandle::new(Arc::clone(context));

        for staged_reference in staged_references {
            staged_reference.is_accessible(AccessType::Commit)?;

            info!(
                "[commit:first-pass] processing {}",
                describe_staged_reference(staged_reference, context)
            );

            trace!("Committing {:?}", staged_reference.temporary_id());

            match commit_holon(staged_reference, context) {
                Ok(CommitOutcome::Saved) => {
                    info!(
                        "[commit:first-pass] saved {}",
                        describe_staged_reference(staged_reference, context)
                    );
                    let holon_id = staged_reference.holon_id()?;
                    let key_string: MapString = staged_reference.key()?.ok_or_else(|| {
                        HolonError::HolonNotFound("Committed holon has no key".into())
                    })?;
                    info!(
                        "[commit:first-pass] saved reference details temp_id={} holon_id={:?} key={}",
                        staged_reference.temporary_id(),
                        holon_id,
                        key_string.0
                    );
                    let saved_reference = HolonReference::smart_with_key(
                        transaction_handle.clone(),
                        holon_id,
                        key_string,
                    );
                    saved_holons.push(saved_reference);
                }
                Ok(CommitOutcome::Abandoned) => {
                    info!(
                        "[commit:first-pass] abandoned {}",
                        describe_staged_reference(staged_reference, context)
                    );
                    // StagedReference → HolonReference via From<&StagedReference>
                    abandoned_holons.push(staged_reference.into());
                }
                Ok(CommitOutcome::NoAction) => {
                    info!(
                        "[commit:first-pass] no-action {}",
                        describe_staged_reference(staged_reference, context)
                    );
                    trace!("No action required for {:?}", staged_reference.temporary_id());
                }
                Err(error) => {
                    response_reference.with_property_value(CommitRequestStatus, "Incomplete")?;
                    abandoned_holons.push(staged_reference.into());
                    warn!("Commit failed for {:?}: {:?}", staged_reference.temporary_id(), error);
                }
            }
        }

        info!(
            "[commit:first-pass] attaching results saved_holons={} abandoned_holons={}",
            saved_holons.len(),
            abandoned_holons.len()
        );

        // Attach results to the CommitResponse holon
        response_reference.add_related_holons(SavedHolons, saved_holons)?;
        info!("[commit:first-pass] attached SavedHolons");
        response_reference.add_related_holons(AbandonedHolons, abandoned_holons)?;
        info!("[commit:first-pass] attached AbandonedHolons");
    }

    // Check if Pass 1 ended with an incomplete status
    info!("[commit:first-pass] reading CommitRequestStatus after attachment");
    if let Some(status_value) = response_reference.property_value(CommitRequestStatus)? {
        let status_string: String = (&status_value).into();
        info!("[commit:first-pass] CommitRequestStatus after attachment = {}", status_string);
        if status_string == "Incomplete" {
            info!("Commit Pass 1 incomplete — skipping Pass 2.");
            return Ok(response_reference);
        }
    }

    // === SECOND PASS: Commit relationships ===
    info!("[commit:second-pass] starting relationship commit pass");
    //
    // Snapshot the committed LocalId + relationship collections under a short-lived read lock,
    // then drop the lock before resolving targets / computing keys / saving smartlinks.
    // This avoids re-entrant locking when a relationship includes a self-edge.
    for staged_reference in staged_references {
        info!(
            "[commit:second-pass] processing {}",
            describe_staged_reference(staged_reference, context)
        );
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
            info!(
                "[commit:second-pass] skipping temp_id={} because no committed relationship snapshot was produced",
                staged_reference.temporary_id()
            );
            continue;
        };

        info!(
            "[commit:second-pass] snapshot ready temp_id={} relationship_collections={}",
            staged_reference.temporary_id(),
            relationship_collections.len()
        );

        // 2) Commit relationships with NO source-holon lock held.
        //
        // We stop at the first relationship error for a given source holon, record it on the holon,
        // and mark the overall response incomplete. This keeps failure handling bounded and avoids
        // cascading secondary errors.
        let mut first_error: Option<HolonError> = None;

        for (name, holon_collection_rc) in relationship_collections {
            debug!("COMMITTING {:#?} relationship", name.0.clone());
            info!(
                "[commit:second-pass] temp_id={} resolving inverse for relationship={}",
                staged_reference.temporary_id(),
                name.0 .0
            );

            // Resolve descriptor metadata before locking the staged collection.
            // `holon_descriptor()` reads `DescribedBy`, which may be the same
            // collection currently being committed.
            let inverse_name = match resolve_inverse_relationship_name(
                &relationship_source.source_reference,
                &name,
                staged_references,
            ) {
                Ok(inverse_name) => inverse_name,
                Err(error) => {
                    info!(
                        "[commit:second-pass] temp_id={} inverse resolution failed for relationship={} error={:?}",
                        staged_reference.temporary_id(),
                        name.0.0,
                        error
                    );
                    first_error = Some(error);
                    break;
                }
            };

            info!(
                "[commit:second-pass] temp_id={} resolved inverse relationship={} inverse={}",
                staged_reference.temporary_id(),
                name.0 .0,
                inverse_name.0 .0
            );

            let holon_collection = holon_collection_rc.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on relationship collection for {}: {}",
                    name.0 .0, e
                ))
            })?;

            info!(
                "[commit:second-pass] temp_id={} committing relationship={} member_count={}",
                staged_reference.temporary_id(),
                name.0 .0,
                holon_collection.get_count().0
            );

            if let Err(err) = commit_relationship(
                &relationship_source,
                name.clone(),
                inverse_name,
                &holon_collection,
            ) {
                info!(
                    "[commit:second-pass] temp_id={} commit_relationship failed for relationship={} error={:?}",
                    staged_reference.temporary_id(),
                    name.0.0,
                    err
                );
                first_error = Some(err);
                break;
            }

            info!(
                "[commit:second-pass] temp_id={} committed relationship={}",
                staged_reference.temporary_id(),
                name.0 .0
            );
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
        } else {
            info!(
                "[commit:second-pass] completed temp_id={} without relationship errors",
                staged_reference.temporary_id()
            );
        }
    }

    //info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");
    // Optionally dump here if you have a helper like `as_json` for references/ids.

    info!("Commit completed: all staged holons processed and commit response constructed.");
    // Done — return the CommitResponse holon reference
    Ok(response_reference)
}

fn describe_staged_reference(
    staged_reference: &StagedReference,
    context: &Arc<TransactionContext>,
) -> String {
    let rc_holon = match staged_reference.get_holon_to_commit(context) {
        Ok(rc_holon) => rc_holon,
        Err(error) => {
            return format!(
                "temp_id={} <failed to resolve staged holon: {:?}>",
                staged_reference.temporary_id(),
                error
            )
        }
    };

    let holon_read = match rc_holon.read() {
        Ok(holon_read) => holon_read,
        Err(error) => {
            return format!(
                "temp_id={} <failed to read staged holon: {}>",
                staged_reference.temporary_id(),
                error
            )
        }
    };

    match &*holon_read {
        Holon::Staged(staged_holon) => format!(
            "temp_id={} state={:?} key={:?} original_id={:?}",
            staged_reference.temporary_id(),
            staged_holon.get_staged_state(),
            staged_holon.key(),
            staged_holon.original_id()
        ),
        other => format!(
            "temp_id={} <unexpected non-staged holon: {:?}>",
            staged_reference.temporary_id(),
            other
        ),
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
        info!(
            "[commit_holon] temp_id={} entering state={:?} key={:?} original_id={:?}",
            staged_reference.temporary_id(),
            staged_state,
            staged_holon.key(),
            staged_holon.original_id()
        );

        match staged_state {
            // === CREATE NEW NODE ============================================================
            StagedState::ForCreate => {
                info!(
                    "[commit_holon] temp_id={} ForCreate -> create_holon_node",
                    staged_reference.temporary_id()
                );
                trace!("StagedState::ForCreate — creating HolonNode in DHT");
                staged_holon.prepare_full_relationship_commit_scope()?;
                let node = staged_holon.into_node_model();
                let record = create_holon_node(HolonNode::from(node))
                    .map_err(holon_error_from_wasm_error)?;

                staged_holon.to_committed(LocalId(record.action_address().clone().into_inner()))?;
                info!(
                    "[commit_holon] temp_id={} ForCreate committed local_id={:?}",
                    staged_reference.temporary_id(),
                    staged_holon.holon_id()
                );
                Ok(CommitOutcome::Saved)
            }

            // === GRAPH-ONLY UPDATE ==========================================================
            StagedState::ForUpdateGraphOnly => {
                info!(
                    "[commit_holon] temp_id={} ForUpdateGraphOnly -> reuse existing source",
                    staged_reference.temporary_id()
                );
                trace!("StagedState::ForUpdateGraphOnly — reusing existing source anchor");
                let source_id = staged_holon.get_versioned_source_id()?;
                staged_holon.prepare_touched_relationship_commit_scope()?;

                staged_holon.to_committed(source_id)?;
                info!(
                    "[commit_holon] temp_id={} ForUpdateGraphOnly committed local_id={:?}",
                    staged_reference.temporary_id(),
                    staged_holon.holon_id()
                );
                Ok(CommitOutcome::Saved)
            }

            // === VERSION-PRODUCING UPDATE ==================================================
            StagedState::ForUpdateNewVersion => {
                info!(
                    "[commit_holon] temp_id={} ForUpdateNewVersion -> create_holon_node",
                    staged_reference.temporary_id()
                );
                trace!("StagedState::ForUpdateNewVersion — creating next HolonNode version");
                let predecessor_id = staged_holon.get_versioned_source_id()?;
                staged_holon.prepare_full_relationship_commit_scope()?;
                let node = staged_holon.into_node_model();
                let record = create_holon_node(HolonNode::from(node))
                    .map_err(holon_error_from_wasm_error)?;
                let new_local_id = LocalId(record.action_address().clone().into_inner());

                if let Err(error) =
                    stage_predecessor_relationship(staged_holon, context, predecessor_id)
                {
                    staged_holon.add_error(error.clone())?;
                    return Err(error);
                }

                staged_holon.to_committed(new_local_id)?;
                info!(
                    "[commit_holon] temp_id={} ForUpdateNewVersion committed local_id={:?}",
                    staged_reference.temporary_id(),
                    staged_holon.holon_id()
                );
                Ok(CommitOutcome::Saved)
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
) -> Result<(), HolonError> {
    collection.is_accessible(AccessType::Commit)?;

    save_smartlinks_for_collection(source, name.clone(), inverse_name, collection)?;

    Ok(())
}

struct ResolvedRelationshipTarget {
    target_local_id: LocalId,
    target_key: Option<MapString>,
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
) -> Result<(), HolonError> {
    let source_id = source.source_local_id();
    let key_prop = CorePropertyTypeName::Key.as_property_name();

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

    let inverse_smart_property_values =
        smart_property_values_from_key(source.source_key.clone(), &key_prop);

    for (target_index, resolved_target) in resolved_targets.iter().enumerate() {
        // Persist both directions from one resolved endpoint pair so the forward
        // and inverse SmartLinks stay anchored to the same committed source.
        let forward_smartlink = SmartLink {
            from_address: source_id.clone(),
            to_address: HolonId::Local(resolved_target.target_local_id.clone()),
            relationship_name: name.clone(),
            smart_property_values: smart_property_values_from_key(
                resolved_target.target_key.clone(),
                &key_prop,
            ),
        };

        debug!(
            "saving smartlink (idx={}): relationship={:?}, source={:?}, target={:?}",
            target_index, name.0 .0, source_id, forward_smartlink.to_address
        );
        save_smartlink(forward_smartlink)?;

        let inverse_smartlink = SmartLink {
            from_address: resolved_target.target_local_id.clone(),
            to_address: HolonId::Local(source_id.clone()),
            relationship_name: inverse_name.clone(),
            smart_property_values: inverse_smart_property_values.clone(),
        };

        debug!(
            "saving inverse smartlink (idx={}): relationship={:?}, source={:?}, target={:?}",
            target_index, inverse_name.0 .0, resolved_target.target_local_id, source_id
        );
        save_smartlink(inverse_smartlink)?;
    }

    Ok(())
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

fn smart_property_values_from_key(
    key_option: Option<MapString>,
    key_property_name: &PropertyName,
) -> Option<PropertyMap> {
    key_option.map(|key| {
        let mut property_values: PropertyMap = BTreeMap::new();
        property_values.insert(key_property_name.clone(), BaseValue::StringValue(key));
        property_values
    })
}
