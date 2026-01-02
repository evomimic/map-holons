use std::collections::BTreeMap;

use base_types::MapString;
use core_types::{HolonError, TemporaryId};
use holons_core::{
    core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference,
    HolonsContextBehavior, ReadableHolon,
};
use tracing::debug;
// use tracing::warn;

use crate::harness::fixtures_support::{ExpectedState, TestReference};

/// Fixture-time factory + registry for [`TestReference`]s.
///
/// - **Only** `FixtureHolons` can mint tokens (it calls `TestReference::new`, which is `pub(crate)`).
/// - Optional lookup by **key** (`MapString`) for fixtures that stage well-known holons
///   and want to retrieve their tokens later by key.
/// - `commit()` flips all *Staged* intents to *Saved* for **expectation** purposes only.
/// 
///  A new root only gets created when a new HolonReference type is created: Transient -> Staged -> Smart
///  otherwise just a "next snapshot" token gets minted, each with its own unique TransientReference identifier serving to hold
///  the "frozen" expected content.
/// 
///  Each root maps to an ExecutionHolon, where the last snapshot token is the expected runtime resolution.
#[derive(Clone, Debug, Default)]
pub struct FixtureHolons {
    lineage: BTreeMap<TemporaryId, Vec<TestReference>>,
    by_key: BTreeMap<MapString, Vec<TemporaryId>>, // Base Key, Lineage Roots
}

impl FixtureHolons {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    // =====  COMMIT  ======  //

    /// Mint token with expected state Saved
    pub fn commit(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Vec<TestReference>, HolonError> {
        let mut saved_tokens = Vec::new();

        for (_id, tokens) in self.lineage.iter_mut() {
            // Get the current (latest) token for each lineage
            if let Some(latest_token) = tokens.last() {
                match latest_token.expected_state() {
                    ExpectedState::Staged => {
                        // Cloning source in order to create a new fixture holon, and a new root
                        let new_root =
                            latest_token.expected_content().clone_holon(context)?;

                        let saved_token = TestReference::new(
                            new_root.clone(),
                            ExpectedState::Saved,
                            new_root,
                        );
                        // Update lineage
                        tokens.push(saved_token.clone());
                        // Return tokens for passing to executor used for building ResolvedTestReference
                        saved_tokens.push(saved_token);
                    }
                    ExpectedState::Abandoned => {
                        debug!("Skipping commit on Abandoned Holon: {:#?}", latest_token)
                    }
                    ExpectedState::Transient => {
                        debug!(
                            "Latest state is not staged, skipping commit on Transient : {:#?}",
                            latest_token
                        );
                    }
                    ExpectedState::Saved => {
                        debug!("Holon already saved : {:#?}", latest_token);
                    }
                    ExpectedState::Deleted => {
                        debug!("Holon marked as deleted : {:#?}", latest_token);
                    }
                }
            } else {
                return Err(HolonError::InvalidParameter(
                    "TestReferences in lineage cannot be empty".to_string(),
                ));
            }
        }
        Ok(saved_tokens)
    }

    // // ==== MINTING ==== // //

    /// Mint a new TestReference snapshot and push it onto the lineage.
    ///
    /// - `root` is the runtime pointer (identifies lineage)
    /// - `expected_content` is a frozen snapshot from the next fixture holon within the lineage
    /// - `expected_state` is the lifecycle intent after the step
    ///
    /// Returns the newly created TestReference.
    pub fn mint_snapshot(
        &mut self,
        root: &TransientReference,
        expected_state: ExpectedState,
        expected_content: TransientReference,
    ) -> TestReference {
        let token = TestReference::new(root.clone(), expected_state, expected_content);
        self.push_snapshot(root.temporary_id(), token.clone());

        token
    }

    /// Mint an ExpectedState::Abandoned cloned from given TestReference (must be expected_state Staged)
    pub fn abandon_staged(
        &mut self,
        staged_token: &TestReference,
    ) -> Result<TestReference, HolonError> {
        match staged_token.expected_state() {
            ExpectedState::Staged => {
                let abandoned_token = self.mint_snapshot(
                    staged_token.root(),
                    ExpectedState::Abandoned,
                    staged_token.expected_content().clone(),
                );
                Ok(abandoned_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only abandon tokens in ExpectedState::Staged, got {:?}",
                other
            ))),
        }
    }

    /// Mint an ExpectedState::Deleted cloned from given TestReference (must be expected_state Saved)
    pub fn delete_saved(
        &mut self,
        saved_token: &TestReference,
    ) -> Result<TestReference, HolonError> {
        match saved_token.expected_state() {
            ExpectedState::Saved => {
                let deleted_token = self.mint_snapshot(
                    saved_token.root(),
                    ExpectedState::Deleted,
                    saved_token.expected_content().clone(),
                );
                Ok(deleted_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only delete tokens in ExpectedState::Saved, got {:?}",
                other
            ))),
        }
    }

    // ----- Creates and adds a new token (the chronological next) in lineage without changing root, based on matching source type -----

    pub fn create_next_snapshot(
        &mut self,
        root: &TransientReference,
        expected_state: ExpectedState,
        next_snapshot: TransientReference,
    ) -> Result<TestReference, HolonError> {
        match expected_state {
            ExpectedState::Transient => Ok(self.add_transient(root, next_snapshot)),
            ExpectedState::Staged => Ok(self.add_staged(root, next_snapshot)),
            _ => Err(HolonError::InvalidParameter(
                "Can only add a Transient or Staged token".to_string(),
            )),
        }
    }

    // ---------- Create tokens (no key indexing) ----------

    /// Create and retain a **Transient** token from a `TransientReference`.
    pub fn add_transient(
        &mut self,
        root: &TransientReference,
        expected_content: TransientReference,
    ) -> TestReference {
        self.mint_snapshot(root, ExpectedState::Transient, expected_content)
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(
        &mut self,
        root: &TransientReference,
        expected_content: TransientReference,
    ) -> TestReference {
        self.mint_snapshot(&root, ExpectedState::Staged, expected_content)
    }

    /// Create and retain a **Saved** token from a `TransientReference`.
    pub fn add_saved(
        &mut self,
        root: &TransientReference,
        expected_content: TransientReference,
    ) -> TestReference {
        self.mint_snapshot(root, ExpectedState::Saved, expected_content)
    }

    // ---------- Create tokens AND index by key (preferred when you know the key) ----------

    /// Create a **Transient** token and index it by the holon’s key.
    pub fn add_transient_with_key(
        &mut self,
        source_reference: &TransientReference,
        key: MapString,
        expected_content: TransientReference,
    ) -> TestReference {
        self.index_by_key(key, source_reference.temporary_id());
        let token = self.add_transient(&source_reference, expected_content);

        token
    }

    /// Create a **Staged** token and index it by the holon’s key.
    pub fn add_staged_with_key(
        &mut self,
        source_reference: &TransientReference,
        key: MapString,
        expected_content: TransientReference,
    ) -> TestReference {
        self.index_by_key(key, source_reference.temporary_id());
        let token = self.mint_snapshot(&source_reference, ExpectedState::Staged, expected_content);

        token
    }

    /// Index an existing token by base key.
    fn index_by_key(&mut self, key: MapString, id: TemporaryId) {
        self.by_key.entry(key).or_insert_with(Vec::new).push(id);
    }

    //  ======  INDEXING  ======  //

    // /// Use with Caution...
    // /// Upsert variant: replace any existing mapping for `key`.
    // /// Prefer `index_by_key` unless you *intend* to overwrite.
    // pub fn upsert_by_key(&mut self, key: MapString, id: TemporaryId) {
    //     self.by_key.insert(key, id);
    // }

    // ---------- Retrieval ----------

    /// Retrieve current token for base key
    pub fn get_latest_by_key(&self, key: &MapString) -> Result<TestReference, HolonError> {
        let id = if let Some(id) = self.by_key.get(key).and_then(|ids| ids.last()) {
            id
        } else {
            return Err(HolonError::InvalidParameter(
                "Key did not return an associated id".to_string(),
            ));
        };
        self.get_latest_for_id(id)
    }

    /// Retrieve current token for id
    pub fn get_latest_for_id(&self, id: &TemporaryId) -> Result<TestReference, HolonError> {
        let vec = self.get_tokens_by_id(id)?;
        vec.last()
            .ok_or(HolonError::InvalidParameter(
                "Lineage returned empty for id, something went wrong".to_string(),
            ))
            .cloned()
    }

    /// Retrieve tokens by id
    pub fn get_tokens_by_id(&self, id: &TemporaryId) -> Result<&Vec<TestReference>, HolonError> {
        if let Some(tokens) = self.lineage.get(id) {
            Ok(tokens)
        } else {
            Err(HolonError::InvalidParameter("Lineage not found for id".to_string()))
        }
    }

    // ---- HELPERS ---- //

    /// Insert the newly minted snapshot into the lineage, as the latest token (added to end of Vec)
    pub fn push_snapshot(&mut self, tid: TemporaryId, token: TestReference) {
        self.lineage
            .entry(tid)
            .and_modify(|v| v.push(token.clone()))
            .or_insert(vec![token.clone()]);
    }

    /// Iterate *latest* snapshots only (one per lineage).
    pub fn latest_snapshots(&self) -> Vec<(TemporaryId, TestReference)> {
        self.lineage
            .iter()
            .filter_map(|(tid, vec)| vec.last().map(|tok| (tid.clone(), tok.clone())))
            .collect()
    }

    // Gets number of Holons per type of ExpectedState for current (latest) in lineage
    pub fn counts(&self) -> FixtureHolonCounts {
        let mut counts = FixtureHolonCounts::default();
        for (_id, snapshots) in &self.lineage {
            let latest_token = snapshots.last().expect("Unexpected: lineage for id is empty");
            match latest_token.expected_state() {
                ExpectedState::Transient => counts.transient += 1,
                ExpectedState::Staged => counts.staged += 1,
                ExpectedState::Saved => counts.saved += 1,
                ExpectedState::Abandoned => counts.staged -= 1,
                ExpectedState::Deleted => counts.saved -= 1,
            }
        }
        counts
    }

    pub fn count_transient(&self) -> i64 {
        self.counts().transient
    }
    pub fn count_staged(&self) -> i64 {
        self.counts().staged
    }
    pub fn count_saved(&self) -> i64 {
        self.counts().saved + 1 // Accounts for initial LocalHolonSpace
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureHolonCounts {
    pub transient: i64,
    pub staged: i64,
    pub saved: i64,
}
