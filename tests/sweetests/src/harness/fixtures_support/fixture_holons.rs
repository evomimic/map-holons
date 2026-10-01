use crate::harness::fixtures_support::{TestHolonState, TestReference};
use base_types::MapInteger;
use core_types::{HolonError, TemporaryId};

use crate::{
    ExpectedCommitCandidate, ExpectedDisposition, ExpectedRetryParticipant,
    ResolvedCommitCandidate, SAVED_LOOKUP_STUB_MARKER,
};
use holons_core::WritableHolon;
use holons_core::{
    core_shared_objects::transactions::TransactionContext, HolonReference, TransientReference,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use super::{ExpectedSnapshot, SnapshotId, SourceSnapshot};
use holons_core::ReadableHolon;
use sha2::{Digest, Sha256};
use uuid::{Builder, Uuid};

/// Immutable snapshot-id to current-head snapshot-id lookup for execution-time
/// consumers that cannot depend on mutable fixture-authoring state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixtureHeadIndex {
    by_snapshot_id: BTreeMap<SnapshotId, SnapshotId>,
}

impl FixtureHeadIndex {
    pub fn new(by_snapshot_id: BTreeMap<SnapshotId, SnapshotId>) -> Self {
        Self { by_snapshot_id }
    }

    pub fn get(&self, snapshot_id: &SnapshotId) -> Option<&SnapshotId> {
        self.by_snapshot_id.get(snapshot_id)
    }
}

/// Hashes the TemporaryId of the first source snapshot token minted
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixtureHolonId(pub Uuid);

impl FixtureHolonId {
    pub fn new_from_id(id: TemporaryId) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(id.0.as_bytes());
        let hash = hasher.finalize();

        // Take the first 16 bytes for UUID
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash[..16]);

        // Set UUID variant RFC4122 version Custom
        let uuid = Builder::from_custom_bytes(bytes.clone()).into_uuid();

        FixtureHolonId(uuid)
    }
}
/// Persisted-source provenance established by staging, independent of Commit expectations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagingSource {
    /// A create or independent clone establishes a new root.
    NewRoot,
    /// An update was staged from this logical holon's persisted version.
    Version { source: FixtureHolonId },
}

/// Whether a fixture head owns its persisted node or reuses another holon's identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedIdentity {
    OwnNode,
    AliasOf(FixtureHolonId),
}

///  Represents one logical holon as it evolves across multiple Test Steps during the Fixture Phase.
///  Mutable and internal to the harness.
#[derive(Clone, Debug)]
pub struct FixtureHolon {
    /// Authoritative snapshot representing the fixture’s current expectation
    /// after the most recent step. Used for chaining and validation.
    head_snapshot: ExpectedSnapshot,

    /// Most recent non-deleted snapshot usable as a source for future steps.
    /// Used when the head snapshot represents a Deleted holon.
    last_live_snapshot: ExpectedSnapshot,

    pub staging_source: StagingSource,
    pub saved_identity: Option<SavedIdentity>,
}

impl FixtureHolon {
    /// Conversion mechanism called by adders that determines which snapshot can be used as the new source and then performs the conversion.
    fn resolve_snapshot_as_source(&self) -> SourceSnapshot {
        if self.head_snapshot.state() == TestHolonState::Deleted {
            self.last_live_snapshot.as_source()
        } else {
            self.head_snapshot.as_source()
        }
    }

    pub fn state(&self) -> TestHolonState {
        self.head_snapshot.state()
    }
}

/// Fixture-time factory + registry for [`TestReference`]s.
///
/// - **Only** `FixtureHolons` can mint tokens (it calls `TestReference::new`, which is `pub(crate)`).
/// - `commit()` advances declared candidates with disposition-specific persisted expectations.
///
///  Each token maps to an ExecutionHolon -- the expected runtime resolution.
#[derive(Clone)]
pub struct FixtureHolons {
    /// Fixture authoring owns one transaction. Expected snapshots are cloned
    /// through it so the harness follows the same transaction-owned clone
    /// policy as runtime code while keeping that context out of `DancesTestCase`.
    fixture_context: Arc<TransactionContext>,
    /// Append-only ledger of all TestReferences minted during fixture authoring,
    /// including tokens not returned to TestCase authors (e.g. commit-minted tokens).
    ///
    /// Used for commit enumeration, validation, and traceability.
    /// Never used for identity resolution or execution-time lookup.
    pub tokens: Vec<TestReference>,
    /// Authoritative registry of logical holons, keyed by stable fixture-time identity.
    ///
    /// This is the single source of truth for logical holon lifecycle state
    /// and head snapshot tracking.
    pub holons: BTreeMap<FixtureHolonId, FixtureHolon>,
    /// Maps snapshot identifiers to their owning logical holon.
    ///
    /// Consulted exclusively when resolving SourceSnapshots at execution time.
    /// ExpectedSnapshots are registered here only to enable future chaining.
    pub snapshot_to_fixture_holon: BTreeMap<SnapshotId, FixtureHolonId>, // keyed index
}

impl FixtureHolons {
    /// Create an empty fixture registry bound to its authoring transaction.
    pub fn new(fixture_context: Arc<TransactionContext>) -> Self {
        Self {
            fixture_context,
            tokens: Vec::new(),
            holons: BTreeMap::new(),
            snapshot_to_fixture_holon: BTreeMap::new(),
        }
    }

    /// Clones a transient fixture snapshot through the destination transaction.
    /// Undescribed and incomplete fixture state is preserved for later validation.
    pub fn copy_fixture_snapshot(
        &self,
        source: &TransientReference,
    ) -> Result<TransientReference, HolonError> {
        self.fixture_context.clone_holon(&source.into())
    }

    /// Creates and adds a new FixtureHolon from the given Expected snapshot.
    /// Only takes Transient or Staged.
    /// Errors if FixtureHolonId already exists, as this should never happen due to a unique TransientReference
    /// for the snapshot being passed since it should have been created from cloning the source.
    pub fn create_fixture_holon(&mut self, snapshot: ExpectedSnapshot) -> Result<(), HolonError> {
        if matches!(
            snapshot.state(),
            TestHolonState::Saved
                | TestHolonState::SavedLookup
                | TestHolonState::Abandoned
                | TestHolonState::Deleted
        ) {
            return Err(HolonError::InvalidParameter(
                "Can only create a FixtureHolon from Transient or Staged".to_string(),
            ));
        }
        self.register_fixture_holon(snapshot, StagingSource::NewRoot)
    }

    /// Registers an update candidate with the persisted source established by staging.
    pub fn create_versioned_fixture_holon(
        &mut self,
        snapshot: ExpectedSnapshot,
        source_token: &TestReference,
    ) -> Result<(), HolonError> {
        let source = self.fixture_id_for_token(source_token)?;
        let source_state = self.holons[&source].state();
        if snapshot.state() != TestHolonState::Staged
            || !matches!(source_state, TestHolonState::Saved | TestHolonState::SavedLookup)
        {
            return Err(HolonError::InvalidParameter(format!(
                "Version staging requires a Staged snapshot and a saved source: {source_token}, source state {source_state}"
            )));
        }
        self.register_fixture_holon(snapshot, StagingSource::Version { source })
    }

    /// Creates and adds a new FixtureHolon for a saved-lookup stub: a key-only
    /// snapshot standing in for a holon committed outside the fixture's ledger
    /// (e.g. by a schema load). Only takes `SavedLookup`.
    ///
    /// Lookup stubs participate in token chaining and execution-time resolution
    /// like any other FixtureHolon, but contribute to no fixture counts and are
    /// never advanced by `commit()`.
    pub fn create_saved_lookup_fixture_holon(
        &mut self,
        snapshot: ExpectedSnapshot,
    ) -> Result<(), HolonError> {
        if snapshot.state() != TestHolonState::SavedLookup {
            return Err(HolonError::InvalidParameter(
                "Can only create a saved-lookup FixtureHolon from SavedLookup".to_string(),
            ));
        }
        self.register_fixture_holon(snapshot, StagingSource::NewRoot)
    }

    /// Shared registration body for new FixtureHolons.
    fn register_fixture_holon(
        &mut self,
        snapshot: ExpectedSnapshot,
        staging_source: StagingSource,
    ) -> Result<(), HolonError> {
        let snapshot_id = snapshot.id();
        // Create and insert FixtureHolon
        let fixture_holon_id = FixtureHolonId::new_from_id(snapshot_id.clone()); // unique id constructor
        let holon = FixtureHolon {
            head_snapshot: snapshot.clone(),
            last_live_snapshot: snapshot,
            staging_source,
            saved_identity: None,
        }; // last live is the same for first creations
        if self.holons.contains_key(&fixture_holon_id) {
            return Err(HolonError::Misc("Something went wrong in logic.. duplicate ids for fixture holons should never happen".to_string()));
        }
        self.holons.insert(fixture_holon_id.clone(), holon);
        // Update keyed index
        self.snapshot_to_fixture_holon.insert(snapshot_id, fixture_holon_id);

        Ok(())
    }

    /// Advances the head_snapshot of the FixtureHolon associated with the given SnapshotId, replacing it with the given new_snapshot TransientReference.
    /// and updates the snapshot_to_fixture_holon keyed index with the id of the new one.
    pub fn advance_head(
        &mut self,
        old_snapshot: &SnapshotId,
        new_snapshot: ExpectedSnapshot,
    ) -> Result<(), HolonError> {
        if let Some(holon_id) = self.snapshot_to_fixture_holon.get(old_snapshot) {
            if let Some(holon) = self.holons.get_mut(holon_id) {
                // Update keyed index unless the snapshot represents a deleted Holon
                self.snapshot_to_fixture_holon.insert(new_snapshot.id(), holon_id.clone());
                if holon.head_snapshot.state() != TestHolonState::Deleted {
                    holon.last_live_snapshot = holon.head_snapshot.clone();
                }
                holon.head_snapshot = new_snapshot;
                Ok(())
            } else {
                Err(HolonError::InvalidParameter(
                    "No FixtureHolon is keyed by the given SnapshotId".to_string(),
                ))
            }
        } else {
            Err(HolonError::InvalidParameter(
                "FixtureHolon not found for FixtureHolonId".to_string(),
            ))
        }
    }

    /// Public helper for adders to derive the next source snapshot to be (potentially) used for the subsequent step.
    /// Extracts the TemporaryId of the ExpectedSnapshot to get the associated fixture holon and uses that to call a private helper
    /// for resolving the appropriate live/head converted as a SourceSnapshot.
    pub fn derive_next_source(
        &mut self,
        token: &TestReference,
    ) -> Result<SourceSnapshot, HolonError> {
        let id = token.expected_id();
        let fixture_holon = self.get_fixture_holon_by_snapshot(&id)?;
        let new_source = fixture_holon.resolve_snapshot_as_source();

        Ok(new_source)
    }

    /// Resolves the expected fixture snapshot to embed as a relationship target.
    ///
    /// Relationship adders use this to avoid freezing stale target snapshots
    /// into expected relationship graphs when callers pass an older token for a
    /// holon whose head has advanced.
    pub fn resolve_expected_relationship_target(
        &self,
        token: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        let id = token.expected_id();
        let fixture_holon = self.get_fixture_holon_by_snapshot(&id)?;
        Ok(fixture_holon.head_snapshot.snapshot().into())
    }

    /// Resolves a relationship-target token to its logical holon's current head token.
    ///
    /// Relationship adders use this so execution steps carry the target's current
    /// head (e.g. the post-commit Saved snapshot) instead of a stale author-supplied
    /// snapshot whose runtime realization may be bound to an earlier transaction.
    /// Tokens whose expected snapshot already is the head — including same-transaction
    /// staged targets and SavedLookup stubs — are returned unchanged.
    pub fn resolve_target_token_to_head(
        &self,
        token: &TestReference,
    ) -> Result<TestReference, HolonError> {
        let fixture_holon = self.get_fixture_holon_by_snapshot(&token.expected_id())?;
        let head = &fixture_holon.head_snapshot;
        if head.id() == token.expected_id() {
            return Ok(token.clone());
        }
        // The head token is deliberately not pushed onto the `tokens` ledger,
        // mirroring commit-minted tokens: it re-labels an already-registered head
        // snapshot rather than introducing a new one.
        Ok(TestReference::new(head.as_source(), head.clone()))
    }

    /// Returns the current head snapshot id for the logical holon that owns
    /// `snapshot_id`, or `None` when the id is not tracked by fixture state.
    pub fn head_snapshot_id_for(&self, snapshot_id: &SnapshotId) -> Option<SnapshotId> {
        let fixture_id = self.snapshot_to_fixture_holon.get(snapshot_id)?;
        Some(self.holons.get(fixture_id)?.head_snapshot.id())
    }

    /// Freezes fixture head redirection for execution-time consumers.
    ///
    /// Head advancement is complete by fixture finalization, so this index is
    /// stable for the duration of execution.
    pub fn head_snapshot_index(&self) -> FixtureHeadIndex {
        FixtureHeadIndex::new(
            self.snapshot_to_fixture_holon
                .keys()
                .filter_map(|id| self.head_snapshot_id_for(id).map(|head_id| (id.clone(), head_id)))
                .collect(),
        )
    }

    /// Removes relationships from staged head snapshots that target the supplied
    /// abandoned fixture snapshot. This keeps expected commit results aligned with
    /// persisted graph semantics after an abandon.
    pub fn remove_relationship_targets_for_staged_holons(
        &mut self,
        abandoned_reference: &TransientReference,
    ) -> Result<(), HolonError> {
        let abandoned_temp_id = abandoned_reference.temporary_id();
        let fixture_ids: Vec<_> = self.holons.keys().cloned().collect();

        for fixture_id in fixture_ids {
            let Some(existing_holon) = self.holons.get(&fixture_id).cloned() else {
                continue;
            };

            if existing_holon.head_snapshot.state() != TestHolonState::Staged {
                continue;
            }

            if existing_holon.head_snapshot.snapshot().temporary_id() == abandoned_temp_id {
                continue;
            }

            let mut updated_snapshot =
                self.copy_fixture_snapshot(existing_holon.head_snapshot.snapshot())?;
            let relationship_map = match updated_snapshot.all_related_holons() {
                Ok(map) => map,
                Err(HolonError::NotImplemented(_)) => continue,
                Err(e) => return Err(e),
            };

            let mut changed = false;
            for (relationship_name, collection_arc) in relationship_map.iter() {
                let existing_members = collection_arc
                    .read()
                    .map_err(|e| {
                        HolonError::FailedToAcquireLock(format!(
                            "Failed to read relationship collection while updating abandon expectations: {}",
                            e
                        ))
                    })?
                    .get_members()
                    .clone();

                let contains_abandoned_target = existing_members.iter().any(|reference| {
                    Self::references_same_temporary_id(reference, &abandoned_temp_id)
                });

                if contains_abandoned_target {
                    updated_snapshot.remove_related_holons(&relationship_name, existing_members)?;
                    changed = true;
                }
            }

            if changed {
                let updated_expected =
                    ExpectedSnapshot::new(updated_snapshot, existing_holon.head_snapshot.state());
                let holon = self
                    .holons
                    .get_mut(&fixture_id)
                    .expect("fixture id collected from self.holons must still exist");
                self.snapshot_to_fixture_holon.insert(updated_expected.id(), fixture_id.clone());
                holon.head_snapshot = updated_expected;
            }
        }

        Ok(())
    }

    /// Retrieves the FixtureHolon that is keyed by the given SnapshotId.
    fn get_fixture_holon_by_snapshot(&self, id: &SnapshotId) -> Result<&FixtureHolon, HolonError> {
        let fixture_id =
            self.snapshot_to_fixture_holon.get(&id).ok_or(HolonError::InvalidParameter(
                "No FixtureHolon is keyed by the given SnapshotId".to_string(),
            ))?;
        let holon = self.holons.get(fixture_id).ok_or(HolonError::InvalidParameter(
            "FixtureHolon not found for FixtureHolonId".to_string(),
        ))?;

        Ok(holon)
    }

    fn references_same_temporary_id(
        reference: &HolonReference,
        temporary_id: &TemporaryId,
    ) -> bool {
        match reference {
            HolonReference::Transient(transient) => transient.temporary_id() == *temporary_id,
            HolonReference::Staged(staged) => staged.temporary_id() == *temporary_id,
            HolonReference::Smart(_) => false,
        }
    }

    // =====  COMMIT  ======  //

    fn fixture_id_for_token(&self, token: &TestReference) -> Result<FixtureHolonId, HolonError> {
        self.snapshot_to_fixture_holon
            .get(&token.expected_id())
            .cloned()
            .ok_or_else(|| HolonError::InvalidParameter(format!("Untracked Commit token: {token}")))
    }

    fn candidate_label(&self, token: &TestReference) -> String {
        format!("{token} (key {:?})", token.expected_reference().key().ok().flatten())
    }

    /// Checks candidate coverage and provenance before any fixture heads advance.
    pub(crate) fn validate_commit_expectations(
        &self,
        declarations: &[ExpectedCommitCandidate],
        retry_participants: &[ExpectedRetryParticipant],
    ) -> Result<(), HolonError> {
        let mut declared = BTreeSet::new();
        for declaration in declarations {
            let id = self.fixture_id_for_token(&declaration.token)?;
            let holon = &self.holons[&id];
            let label = self.candidate_label(&declaration.token);
            if holon.state() != TestHolonState::Staged {
                return Err(HolonError::InvalidParameter(format!(
                    "Commit candidate {label} has state {}, expected Staged",
                    holon.state()
                )));
            }
            if !declared.insert(id) {
                return Err(HolonError::InvalidParameter(format!(
                    "Duplicate Commit candidate {label}"
                )));
            }
            let compatible = matches!(
                (&holon.staging_source, declaration.disposition),
                (StagingSource::NewRoot, ExpectedDisposition::NewRoot)
                    | (
                        StagingSource::Version { .. },
                        ExpectedDisposition::NoAction
                            | ExpectedDisposition::GraphOnly
                            | ExpectedDisposition::NewVersion
                    )
            );
            if !compatible {
                return Err(HolonError::InvalidParameter(format!("Commit candidate {label}: disposition {} incompatible with staging source {:?}", declaration.disposition, holon.staging_source)));
            }
        }
        for (id, holon) in &self.holons {
            if holon.state() == TestHolonState::Staged && !declared.contains(id) {
                let token = TestReference::new(
                    holon.head_snapshot.as_source(),
                    holon.head_snapshot.clone(),
                );
                return Err(HolonError::InvalidParameter(format!(
                    "Missing Commit disposition for {}",
                    self.candidate_label(&token)
                )));
            }
        }
        let mut retries = BTreeSet::new();
        for participant in retry_participants {
            let id = self.fixture_id_for_token(&participant.token)?;
            let holon = &self.holons[&id];
            if declared.contains(&id)
                || !retries.insert(id)
                || !matches!(holon.state(), TestHolonState::Saved | TestHolonState::SavedLookup)
                || holon.saved_identity.is_none()
            {
                return Err(HolonError::InvalidParameter(format!("Invalid or duplicate Commit retry participant {}: state {}, saved identity {:?}", self.candidate_label(&participant.token), holon.state(), holon.saved_identity)));
            }
        }
        Ok(())
    }

    /// Derives declarations only when staging unambiguously established new roots.
    pub fn derive_create_only_declarations(
        &self,
    ) -> Result<Vec<ExpectedCommitCandidate>, HolonError> {
        self.holons.values().filter(|holon| holon.state() == TestHolonState::Staged).map(|holon| {
            let token = TestReference::new(holon.head_snapshot.as_source(), holon.head_snapshot.clone());
            if holon.staging_source != StagingSource::NewRoot {
                return Err(HolonError::InvalidParameter(format!("Commit candidate {} has update provenance; declare its disposition explicitly", self.candidate_label(&token))));
            }
            Ok(ExpectedCommitCandidate::new(token, ExpectedDisposition::NewRoot))
        }).collect()
    }

    /// Mints one result token per declaration, including source aliases for no-action updates.
    /// Declarations retain author order and never derive policy from fixture mutations.
    pub fn commit(
        &mut self,
        declarations: &[ExpectedCommitCandidate],
        retry_participants: &[ExpectedRetryParticipant],
    ) -> Result<Vec<ResolvedCommitCandidate>, HolonError> {
        self.validate_commit_expectations(declarations, retry_participants)?;
        // Prepare against unchanged heads before mutating the registry, so a preparation
        // error leaves every head intact. Version sources are saved when registered;
        // live Commit candidates must be staged, keeping source heads distinct from them.
        let mut prepared = Vec::new();
        for declaration in declarations {
            let id = self.fixture_id_for_token(&declaration.token)?;
            let holon = &self.holons[&id];
            let staged_token = self.resolve_target_token_to_head(&declaration.token)?;
            let mut snapshot = self.copy_fixture_snapshot(holon.head_snapshot.snapshot())?;
            let saved_identity = match (&holon.staging_source, declaration.disposition) {
                (StagingSource::NewRoot, ExpectedDisposition::NewRoot) => {
                    snapshot.with_predecessor(None)?;
                    SavedIdentity::OwnNode
                }
                (
                    StagingSource::Version { source },
                    ExpectedDisposition::NoAction | ExpectedDisposition::GraphOnly,
                ) => SavedIdentity::AliasOf(source.clone()),
                (StagingSource::Version { source }, ExpectedDisposition::NewVersion) => {
                    let source_head = &self.holons[source].head_snapshot;
                    let source_token =
                        TestReference::new(source_head.as_source(), source_head.clone());
                    snapshot.with_predecessor(Some(
                        self.resolve_expected_relationship_target(&source_token)?,
                    ))?;
                    SavedIdentity::OwnNode
                }
                _ => unreachable!("provenance validated before preparing snapshots"),
            };
            let state = if snapshot.property_value(SAVED_LOOKUP_STUB_MARKER)?.is_some() {
                TestHolonState::SavedLookup
            } else {
                TestHolonState::Saved
            };
            let expected = ExpectedSnapshot::new(snapshot, state);
            let result_token = TestReference::new(holon.head_snapshot.as_source(), expected);
            prepared.push((
                id,
                saved_identity,
                ResolvedCommitCandidate {
                    staged_token,
                    disposition: declaration.disposition,
                    result_token,
                    expected_new_errors: declaration.expected_new_errors.clone(),
                },
            ));
        }
        let mut resolved = Vec::new();
        for (id, saved_identity, candidate) in prepared {
            let old_id = self.holons[&id].head_snapshot.id();
            self.advance_head(&old_id, candidate.result_token.expected_snapshot())?;
            self.holons.get_mut(&id).expect("registered fixture").saved_identity =
                Some(saved_identity);
            resolved.push(candidate);
        }
        Ok(resolved)
    }

    // // ==== MINTING ==== // //

    /// Mint a new TestReference token from the frozen snapshots and push it onto FixtureHolons.tokens.
    ///
    /// Returns the newly created TestReference to be used as input for the next step.
    pub fn mint_test_reference(
        &mut self,
        source: SourceSnapshot,
        expected: ExpectedSnapshot,
    ) -> TestReference {
        let token = TestReference::new(source, expected);
        self.tokens.push(token.clone());

        token
    }

    // ---- HELPERS ---- //

    // Gets number of Holons per type of TestHolonState in FixtureHolons
    pub fn counts(&self) -> FixtureHolonCounts {
        let mut counts = FixtureHolonCounts::default();
        for holon in self.holons.values() {
            let state = holon.head_snapshot.state();
            match state {
                TestHolonState::Transient => counts.transient += 1,
                TestHolonState::Staged => counts.staged += 1,
                TestHolonState::Abandoned => counts.staged -= 1,
                // Saved totals follow node ownership, including aliases and deleted heads.
                // External lookup stubs own no node; versions of partial stubs can own one.
                TestHolonState::Saved | TestHolonState::SavedLookup | TestHolonState::Deleted => {}
            }
        }
        counts.saved = self.count_saved().0;
        counts
    }

    /// Counts fixture-owned persisted nodes; aliases and lookup stubs add no nodes.
    pub fn count_saved(&self) -> MapInteger {
        MapInteger(
            self.holons
                .values()
                .filter(|holon| {
                    holon.saved_identity == Some(SavedIdentity::OwnNode)
                        && holon.state() != TestHolonState::Deleted
                })
                .count() as i64,
        )
    }

    pub fn count_transient(&self) -> MapInteger {
        MapInteger(self.counts().transient)
    }
    pub fn count_staged(&self) -> MapInteger {
        MapInteger(self.counts().staged)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureHolonCounts {
    pub transient: i64,
    pub staged: i64,
    pub saved: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_fixture_context;
    use base_types::MapString;
    use holons_core::core_shared_objects::transactions::TransactionContext;
    use std::sync::Arc;

    /// Registers a new Staged FixtureHolon and mints its authoring token,
    /// mirroring the snapshot flow used by `add_stage_holon_step`.
    fn mint_staged_token(
        context: &Arc<TransactionContext>,
        fixture_holons: &mut FixtureHolons,
        key: &str,
    ) -> TestReference {
        let transient = context
            .mutation()
            .new_holon(Some(MapString(key.to_string())))
            .expect("new_holon should succeed");
        let staged = ExpectedSnapshot::new(
            fixture_holons
                .copy_fixture_snapshot(&transient)
                .expect("copy_fixture_snapshot should succeed"),
            TestHolonState::Staged,
        );
        fixture_holons.create_fixture_holon(staged.clone()).expect("create_fixture_holon");
        fixture_holons
            .mint_test_reference(SourceSnapshot::new(transient, TestHolonState::Transient), staged)
    }

    #[test]
    fn rejected_commit_step_preserves_heads_for_corrected_retry() {
        use crate::{DanceTestStep, DancesTestCase, ExpectedCommitStatus};

        let context = init_fixture_context();
        let mut fixture_holons = FixtureHolons::new(context.clone());
        let staged_token = mint_staged_token(&context, &mut fixture_holons, "rejected-book");
        let mut test_case = DancesTestCase::default();

        test_case
            .add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)
            .unwrap();
        let head = fixture_holons.resolve_target_token_to_head(&staged_token).unwrap();
        assert_eq!(head.expected_id(), staged_token.expected_id());
        assert_eq!(head.expected_snapshot().state(), TestHolonState::Staged);
        assert!(
            matches!(test_case.steps.last(), Some(DanceTestStep::Commit { saved_tokens, .. }) if saved_tokens.is_empty())
        );

        // The same token can feed a correction step; a subsequent accepted Commit advances it.
        assert_eq!(
            fixture_holons.derive_next_source(&staged_token).unwrap().state(),
            TestHolonState::Staged
        );
        test_case
            .add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)
            .unwrap();
        let head = fixture_holons.resolve_target_token_to_head(&staged_token).unwrap();
        assert_eq!(head.expected_snapshot().state(), TestHolonState::Saved);
        assert_ne!(head.expected_id(), staged_token.expected_id());
    }

    #[test]
    fn head_advanced_token_resolves_to_saved_head() {
        let context = init_fixture_context();
        let mut fixture_holons = FixtureHolons::new(context.clone());
        let staged_token = mint_staged_token(&context, &mut fixture_holons, "book-key");

        let declarations = fixture_holons.derive_create_only_declarations().unwrap();
        fixture_holons.commit(&declarations, &[]).expect("commit should advance staged heads");

        let head_token = fixture_holons
            .resolve_target_token_to_head(&staged_token)
            .expect("head resolution should succeed");
        assert_eq!(head_token.expected_snapshot().state(), TestHolonState::Saved);
        assert_ne!(head_token.expected_id(), staged_token.expected_id());
        assert_eq!(
            Some(head_token.expected_id()),
            fixture_holons.head_snapshot_id_for(&staged_token.expected_id())
        );
    }

    #[test]
    fn same_head_token_is_returned_unchanged() {
        let context = init_fixture_context();
        let mut fixture_holons = FixtureHolons::new(context.clone());
        let staged_token = mint_staged_token(&context, &mut fixture_holons, "person-key");

        let resolved = fixture_holons
            .resolve_target_token_to_head(&staged_token)
            .expect("head resolution should succeed");
        assert_eq!(resolved, staged_token);
    }

    fn commit_roots(fixture_holons: &mut FixtureHolons) {
        let declarations = fixture_holons.derive_create_only_declarations().unwrap();
        fixture_holons.commit(&declarations, &[]).unwrap();
    }

    fn stage_version(fixture_holons: &mut FixtureHolons, source: &TestReference) -> TestReference {
        crate::DancesTestCase::default()
            .add_stage_new_version_step(
                fixture_holons,
                source.clone(),
                None,
                MapInteger(1),
                None,
                None,
            )
            .unwrap()
    }

    #[test]
    fn no_action_advances_head_without_a_saved_result_and_preserves_error_declaration() {
        use crate::{DanceTestStep, DancesTestCase, ExpectedCommitStatus};
        use integrity_core_types::HolonErrorKind;
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let root = mint_staged_token(&context, &mut fixtures, "unchanged");
        commit_roots(&mut fixtures);
        let update = stage_version(&mut fixtures, &root);
        let mut test_case = DancesTestCase::default();
        test_case
            .add_commit_step_with_dispositions(
                &mut fixtures,
                ExpectedCommitStatus::Complete,
                vec![ExpectedCommitCandidate::new(update.clone(), ExpectedDisposition::NoAction)
                    .with_expected_new_errors(vec![HolonErrorKind::CommitFailure])],
                vec![],
                None,
                None,
            )
            .unwrap();
        let Some(DanceTestStep::Commit { saved_tokens, candidates, .. }) = test_case.steps.last()
        else {
            panic!("expected Commit")
        };
        assert!(saved_tokens.is_empty());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].expected_new_errors, vec![HolonErrorKind::CommitFailure]);
        let head = fixtures.resolve_target_token_to_head(&update).unwrap();
        assert_eq!(head.expected_id(), candidates[0].result_token.expected_id());
        assert_ne!(head.expected_id(), update.expected_id());
        assert_eq!(head.expected_snapshot().state(), TestHolonState::Saved);
        let source_id = fixtures.fixture_id_for_token(&root).unwrap();
        assert_eq!(
            fixtures.get_fixture_holon_by_snapshot(&head.expected_id()).unwrap().saved_identity,
            Some(SavedIdentity::AliasOf(source_id))
        );
        assert_eq!(fixtures.count_saved(), MapInteger(1));
    }

    #[test]
    fn successive_versions_replace_copied_predecessor_without_mutating_the_source() {
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "versioned");
        commit_roots(&mut fixtures);
        let a_head = fixtures.resolve_target_token_to_head(&a).unwrap();
        let b = stage_version(&mut fixtures, &a);
        let b_result = fixtures
            .commit(
                &[ExpectedCommitCandidate::new(b.clone(), ExpectedDisposition::NewVersion)],
                &[],
            )
            .unwrap()
            .remove(0)
            .result_token;
        assert_eq!(
            b_result.expected_reference().predecessor().unwrap(),
            Some(a_head.expected_reference().into())
        );
        let b_predecessor = b_result.expected_reference().predecessor().unwrap();
        let c = stage_version(&mut fixtures, &b);
        let c_result = fixtures
            .commit(&[ExpectedCommitCandidate::new(c, ExpectedDisposition::NewVersion)], &[])
            .unwrap()
            .remove(0)
            .result_token;
        assert_eq!(
            c_result.expected_reference().predecessor().unwrap(),
            Some(b_result.expected_reference().into())
        );
        assert_eq!(b_result.expected_reference().predecessor().unwrap(), b_predecessor);
        assert_eq!(
            fixtures.resolve_target_token_to_head(&b).unwrap().expected_id(),
            b_result.expected_id()
        );
        assert!(a_head.expected_reference().predecessor().unwrap().is_none());
        assert_eq!(fixtures.count_saved(), MapInteger(3));
    }

    #[test]
    fn unchanged_and_graph_only_updates_retain_non_root_source_lineage() {
        for disposition in [ExpectedDisposition::NoAction, ExpectedDisposition::GraphOnly] {
            let context = init_fixture_context();
            let mut fixtures = FixtureHolons::new(context.clone());
            let a = mint_staged_token(&context, &mut fixtures, "lineage-source");
            commit_roots(&mut fixtures);
            let b = stage_version(&mut fixtures, &a);
            let b_result = fixtures
                .commit(
                    &[ExpectedCommitCandidate::new(b.clone(), ExpectedDisposition::NewVersion)],
                    &[],
                )
                .unwrap()
                .remove(0)
                .result_token;
            let predecessor = b_result.expected_reference().predecessor().unwrap();
            let update = stage_version(&mut fixtures, &b);
            let result = fixtures
                .commit(&[ExpectedCommitCandidate::new(update, disposition)], &[])
                .unwrap()
                .remove(0)
                .result_token;
            assert_eq!(result.expected_reference().predecessor().unwrap(), predecessor);
            assert_eq!(b_result.expected_reference().predecessor().unwrap(), predecessor);
            assert_eq!(fixtures.count_saved(), MapInteger(2));
            assert_eq!(fixtures.counts().saved, 2);
        }
    }

    #[test]
    fn independent_clone_commit_clears_inherited_predecessor() {
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "clone-source");
        commit_roots(&mut fixtures);
        let b = stage_version(&mut fixtures, &a);
        let b_result = fixtures
            .commit(&[ExpectedCommitCandidate::new(b, ExpectedDisposition::NewVersion)], &[])
            .unwrap()
            .remove(0)
            .result_token;
        let snapshot = fixtures.copy_fixture_snapshot(b_result.expected_reference()).unwrap();
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        fixtures.create_fixture_holon(expected.clone()).unwrap();
        let clone =
            fixtures.mint_test_reference(b_result.expected_snapshot().as_source(), expected);
        let result = fixtures
            .commit(&[ExpectedCommitCandidate::new(clone, ExpectedDisposition::NewRoot)], &[])
            .unwrap()
            .remove(0)
            .result_token;
        assert!(result.expected_reference().predecessor().unwrap().is_none());
        assert!(b_result.expected_reference().predecessor().unwrap().is_some());
    }

    #[test]
    fn saved_lookup_sources_remain_partial_for_all_update_dispositions() {
        for disposition in [
            ExpectedDisposition::NoAction,
            ExpectedDisposition::GraphOnly,
            ExpectedDisposition::NewVersion,
        ] {
            let context = init_fixture_context();
            let mut fixtures = FixtureHolons::new(context.clone());
            let mut snapshot =
                context.mutation().new_holon(Some(MapString("external".into()))).unwrap();
            snapshot.with_property_value(SAVED_LOOKUP_STUB_MARKER, true).unwrap();
            let expected = ExpectedSnapshot::new(snapshot, TestHolonState::SavedLookup);
            fixtures.create_saved_lookup_fixture_holon(expected.clone()).unwrap();
            let stub = fixtures.mint_test_reference(expected.as_source(), expected);
            let update = stage_version(&mut fixtures, &stub);
            let result = fixtures
                .commit(&[ExpectedCommitCandidate::new(update, disposition)], &[])
                .unwrap()
                .remove(0)
                .result_token;
            assert_eq!(result.expected_snapshot().state(), TestHolonState::SavedLookup);
            assert_eq!(
                fixtures.resolve_target_token_to_head(&stub).unwrap().expected_snapshot().state(),
                TestHolonState::SavedLookup
            );
            assert_eq!(
                fixtures.count_saved(),
                MapInteger(if disposition == ExpectedDisposition::NewVersion { 1 } else { 0 })
            );
        }
    }

    #[test]
    fn create_only_convenience_rejects_update_provenance_without_advancing_heads() {
        use crate::{DancesTestCase, ExpectedCommitStatus};
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "explicit-update");
        commit_roots(&mut fixtures);
        let update = stage_version(&mut fixtures, &a);
        let mut test_case = DancesTestCase::default();
        let error = test_case
            .add_commit_step(&mut fixtures, ExpectedCommitStatus::Complete, None, None)
            .unwrap_err();
        assert!(error.to_string().contains("explicit-update"));
        assert!(test_case.steps.is_empty());
        assert_eq!(fixtures.resolve_target_token_to_head(&update).unwrap(), update);
    }

    #[test]
    fn mixed_declarations_preserve_author_order_and_filter_only_no_action() {
        use crate::{DanceTestStep, DancesTestCase, ExpectedCommitStatus};
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "mixed-a");
        let b = mint_staged_token(&context, &mut fixtures, "mixed-b");
        let c = mint_staged_token(&context, &mut fixtures, "mixed-c");
        commit_roots(&mut fixtures);
        let unchanged = stage_version(&mut fixtures, &a);
        let graph_only = stage_version(&mut fixtures, &b);
        let version = stage_version(&mut fixtures, &c);
        let root = mint_staged_token(&context, &mut fixtures, "mixed-root");
        let declarations = vec![
            ExpectedCommitCandidate::new(version, ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(unchanged, ExpectedDisposition::NoAction),
            ExpectedCommitCandidate::new(root, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(graph_only, ExpectedDisposition::GraphOnly),
        ];
        let mut test_case = DancesTestCase::default();
        test_case
            .add_commit_step_with_dispositions(
                &mut fixtures,
                ExpectedCommitStatus::Complete,
                declarations.clone(),
                vec![],
                None,
                None,
            )
            .unwrap();
        let Some(DanceTestStep::Commit { saved_tokens, candidates, .. }) = test_case.steps.last()
        else {
            panic!("expected Commit")
        };
        assert_eq!(
            candidates.iter().map(|candidate| candidate.staged_token.clone()).collect::<Vec<_>>(),
            declarations.iter().map(|declaration| declaration.token.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            *saved_tokens,
            vec![
                candidates[0].result_token.clone(),
                candidates[2].result_token.clone(),
                candidates[3].result_token.clone()
            ]
        );
        assert_eq!(fixtures.count_saved(), MapInteger(5));
    }

    #[test]
    fn coverage_rejects_missing_duplicate_incompatible_and_non_staged_candidates() {
        use crate::{DancesTestCase, ExpectedCommitStatus};
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "coverage-a");
        let b = mint_staged_token(&context, &mut fixtures, "coverage-b");
        let declaration = ExpectedCommitCandidate::new(a.clone(), ExpectedDisposition::NewRoot);
        let mut test_case = DancesTestCase::default();
        for (declarations, diagnostic) in [
            (vec![declaration.clone()], "coverage-b"),
            (vec![declaration.clone(), declaration.clone()], "Duplicate"),
            (
                vec![ExpectedCommitCandidate::new(a.clone(), ExpectedDisposition::GraphOnly)],
                "incompatible",
            ),
        ] {
            let error = test_case
                .add_commit_step_with_dispositions(
                    &mut fixtures,
                    ExpectedCommitStatus::Complete,
                    declarations,
                    vec![],
                    None,
                    None,
                )
                .unwrap_err();
            assert!(error.to_string().contains(diagnostic), "{error}");
            assert!(test_case.steps.is_empty());
            assert_eq!(fixtures.resolve_target_token_to_head(&a).unwrap(), a);
            assert_eq!(fixtures.resolve_target_token_to_head(&b).unwrap(), b);
        }
        commit_roots(&mut fixtures);
        let error = fixtures.commit(&[declaration], &[]).unwrap_err();
        assert!(error.to_string().contains("state Saved"));
    }

    #[test]
    fn retry_participants_require_saved_identity_and_unique_disjoint_coverage() {
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "retry-source");
        let declaration = ExpectedCommitCandidate::new(a.clone(), ExpectedDisposition::NewRoot);
        let retry = ExpectedRetryParticipant::new(a.clone());
        assert!(fixtures.commit(&[declaration], &[retry.clone()]).is_err());
        commit_roots(&mut fixtures);
        assert!(fixtures.commit(&[], &[retry.clone()]).is_ok());
        assert!(fixtures.commit(&[], &[retry.clone(), retry]).is_err());
        use crate::{DanceTestStep, DancesTestCase, ExpectedCommitStatus};
        let mut test_case = DancesTestCase::default();
        let before = fixtures.resolve_target_token_to_head(&a).unwrap();
        test_case
            .add_commit_step_with_dispositions(
                &mut fixtures,
                ExpectedCommitStatus::Incomplete,
                vec![],
                vec![ExpectedRetryParticipant::new(before.clone())],
                None,
                None,
            )
            .unwrap();
        let Some(DanceTestStep::Commit { saved_tokens, candidates, retry_participants, .. }) =
            test_case.steps.last()
        else {
            panic!("expected Commit")
        };
        assert!(saved_tokens.is_empty());
        assert!(candidates.is_empty());
        assert_eq!(retry_participants.len(), 1);
        assert_eq!(fixtures.resolve_target_token_to_head(&a).unwrap(), before);
    }

    #[test]
    fn command_error_commit_preserves_staged_heads() {
        use crate::{DancesTestCase, ExpectedCommitStatus};
        use integrity_core_types::HolonErrorKind;
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let a = mint_staged_token(&context, &mut fixtures, "command-error");
        let mut test_case = DancesTestCase::default();
        test_case
            .add_commit_step(
                &mut fixtures,
                ExpectedCommitStatus::Complete,
                Some(HolonErrorKind::CommitFailure),
                None,
            )
            .unwrap();
        assert_eq!(fixtures.resolve_target_token_to_head(&a).unwrap(), a);
    }

    #[test]
    fn property_mutations_do_not_rewrite_declared_dispositions() {
        use crate::{DanceTestStep, DancesTestCase, ExpectedCommitStatus};
        use holons_prelude::prelude::*;
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let root = mint_staged_token(&context, &mut fixtures, "stale-declaration");
        commit_roots(&mut fixtures);
        let update = stage_version(&mut fixtures, &root);
        let declaration =
            ExpectedCommitCandidate::new(update.clone(), ExpectedDisposition::GraphOnly);
        let properties = [("Title".to_property_name(), "Changed".to_base_value())].into();
        let mut test_case = DancesTestCase::default();
        let mutated = test_case
            .add_with_properties_step(&mut fixtures, update, properties, None, None)
            .unwrap();
        test_case
            .add_commit_step_with_dispositions(
                &mut fixtures,
                ExpectedCommitStatus::Complete,
                vec![declaration],
                vec![],
                None,
                None,
            )
            .unwrap();
        let Some(DanceTestStep::Commit { candidates, .. }) = test_case.steps.last() else {
            panic!("expected Commit")
        };
        assert_eq!(candidates[0].disposition, ExpectedDisposition::GraphOnly);
        assert_eq!(candidates[0].staged_token.expected_id(), mutated.expected_id());
    }

    #[test]
    fn deleting_an_owned_head_removes_its_saved_count() {
        let context = init_fixture_context();
        let mut fixtures = FixtureHolons::new(context.clone());
        let root = mint_staged_token(&context, &mut fixtures, "deleted-count");
        commit_roots(&mut fixtures);
        let head = fixtures.resolve_target_token_to_head(&root).unwrap();
        let deleted = ExpectedSnapshot::new(
            fixtures.copy_fixture_snapshot(head.expected_reference()).unwrap(),
            TestHolonState::Deleted,
        );
        fixtures.advance_head(&head.expected_id(), deleted).unwrap();
        assert_eq!(fixtures.count_saved(), MapInteger(0));
        assert_eq!(fixtures.counts().saved, 0);
    }

    #[test]
    fn untracked_token_errors() {
        let context = init_fixture_context();
        let mut fixture_holons = FixtureHolons::new(context.clone());
        // Mint a token without registering a FixtureHolon for its snapshot.
        let transient = context
            .mutation()
            .new_holon(Some(MapString("orphan-key".to_string())))
            .expect("new_holon should succeed");
        let expected = ExpectedSnapshot::new(
            fixture_holons
                .copy_fixture_snapshot(&transient)
                .expect("copy_fixture_snapshot should succeed"),
            TestHolonState::Staged,
        );
        let token = fixture_holons.mint_test_reference(
            SourceSnapshot::new(transient, TestHolonState::Transient),
            expected,
        );

        assert!(fixture_holons.resolve_target_token_to_head(&token).is_err());
    }
}
