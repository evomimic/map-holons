use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{reference_layer::TransientReference, ReadableHolon};

use base_types::MapInteger;
use core_types::{HolonError, TemporaryId};
use derive_new::new;
use holons_core::{reference_layer::TransientReference, ReadableHolon};
use tracing::debug;
// use tracing::warn;
use crate::harness::fixtures_support::{TestHolonState, TestReference};
use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use uuid::{Builder, Uuid};

use super::{ExpectedSnapshot, SnapshotId, SourceSnapshot};

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
///  Represents one logical holon as it evolves across multiple Test Steps during the Fixture Phase.
///  Mutable and internal to the harness.
#[derive(new, Clone, Debug)]
pub struct FixtureHolon {
    // Stable (immutable) fixture-time identity
    id: FixtureHolonId,
    /// Authoritative snapshot representing the fixture’s current expectation
    /// after the most recent step. Used for chaining and validation.
    head_snapshot: ExpectedSnapshot,

    /// Most recent non-deleted snapshot usable as a source for future steps.
    /// Used when the head snapshot represents a Deleted holon.
    last_live_snapshot: ExpectedSnapshot,
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
/// - `commit()` advances head with a minted *Saved* expectation for all *Staged* intents.
///
///  Each token maps to an ExecutionHolon -- the expected runtime resolution.
#[derive(Clone, Debug, Default)]
pub struct FixtureHolons {
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
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates and adds a new FixtureHolon from the given Expected snapshot.
    /// Only takes Transient or Staged.
    /// Errors if FixtureHolonId already exists, as this should never happen due to a unique TransientReference
    /// for the snapshot being passed since it should have been created from cloning the source.
    pub fn create_fixture_holon(&mut self, snapshot: ExpectedSnapshot) -> Result<(), HolonError> {
        if matches!(
            snapshot.state(),
            TestHolonState::Saved | TestHolonState::Abandoned | TestHolonState::Deleted
        ) {
            return Err(HolonError::InvalidParameter(
                "Can only create a FixtureHolon from Transient or Staged".to_string(),
            ));
        }
        let snapshot_id = snapshot.id();
        // Create and insert FixtureHolon
        let fixture_holon_id = FixtureHolonId::new_from_id(snapshot_id.clone()); // unique id constructor
        let holon = FixtureHolon::new(fixture_holon_id.clone(), snapshot.clone(), snapshot); // last live is the same for first creations
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

    // =====  COMMIT  ======  //

    /// Mint tokens with expected state Saved.
    /// Returned tokens are *only* used for resolution of expected during the execution, and never passed to an add step.
    pub fn commit(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Vec<TestReference>, HolonError> {
        let mut saved_tokens = Vec::new();

        for holon in self.holons.clone().values() {
            match holon.head_snapshot.state() {
                TestHolonState::Staged => {
                    let snapshot = holon.head_snapshot.snapshot().clone().clone_holon(context)?;
                    let source = holon.head_snapshot.as_source();
                    let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Saved);
                    // Mint saved
                    let saved_token = TestReference::new(source, expected.clone());
                    // Return tokens for passing to executor used for building ExecutionReference
                    saved_tokens.push(saved_token);
                    // Advance head
                    self.advance_head(&holon.head_snapshot.snapshot().temporary_id(), expected)?;
                }
                TestHolonState::Abandoned => {
                    debug!("Skipping commit on Abandoned Holon: {:#?}", holon);
                }
                TestHolonState::Transient => {
                    debug!(
                        "Latest state is not staged, skipping commit on Transient : {:#?}",
                        holon
                    );
                }
                TestHolonState::Saved => {
                    debug!("Holon already saved : {:#?}", holon);
                }
                TestHolonState::Deleted => {
                    debug!("Holon marked as deleted : {:#?}", holon);
                }
            }
        }

        Ok(saved_tokens)
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
                TestHolonState::Saved => counts.saved += 1,
                TestHolonState::Abandoned => counts.staged -= 1,
                TestHolonState::Deleted => counts.saved -= 1,
            }
        }
        counts
    }

    pub fn count_transient(&self) -> MapInteger {
        MapInteger(self.counts().transient)
    }
    pub fn count_staged(&self) -> MapInteger {
        MapInteger(self.counts().staged)
    }
    pub fn count_saved(&self) -> MapInteger {
        MapInteger(self.counts().saved + 1) // Accounts for initial LocalHolonSpace
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureHolonCounts {
    pub transient: i64,
    pub staged: i64,
    pub saved: i64,
}
