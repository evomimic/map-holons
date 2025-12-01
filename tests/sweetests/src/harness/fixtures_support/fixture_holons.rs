use std::collections::BTreeMap;

use base_types::MapString;
use core_types::{HolonError, TemporaryId};
use holons_core::{
    core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference,
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
#[derive(Clone, Debug, Default)]
pub struct FixtureHolons {
    lineage: BTreeMap<TemporaryId, Vec<TestReference>>,
    by_key: BTreeMap<MapString, TemporaryId>,
}

impl FixtureHolons {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    // =====  COMMIT  ======  //

    /// Mint token with expected state Saved
    pub fn commit(&mut self) -> Result<Vec<TestReference>, HolonError> {
        let mut saved_tokens = Vec::new();

        for (_id, tokens) in self.lineage.iter_mut() {
            // Get the current (latest) token for each lineage
            if let Some(latest_token) = tokens.last() {
                match latest_token.expected_state() {
                    ExpectedState::Staged => {
                        let saved_token = TestReference::new(
                            latest_token.transient().clone(),
                            ExpectedState::Saved,
                            latest_token.expected_content().clone(),
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
                        return Err(HolonError::CommitFailure(
                            "TestReference to be Saved must be in an ExpectedState::Staged, got: Transient".to_string()
                        ))
                    }
                    ExpectedState::Saved => {debug!("Holon already saved : {:#?}", latest_token)}
                    ExpectedState::Deleted => {debug!("Holon marked as deleted : {:#?}", latest_token)}
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
    /// - `transient_ref` is the runtime pointer (identifies lineage)
    /// - `expected_content` is a frozen snapshot from that transient holon
    /// - `expected_state` is the lifecycle intent after the step
    ///
    /// Returns the newly created TestReference.
    pub fn mint_snapshot(
        &mut self,
        transient_ref: &TransientReference,
        expected_state: ExpectedState,
        expected_content: &EssentialHolonContent,
    ) -> TestReference {
        let token =
            TestReference::new(transient_ref.clone(), expected_state, expected_content.clone());
        self.push_snapshot(transient_ref.get_temporary_id(), token.clone());

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
                    staged_token.transient(),
                    ExpectedState::Abandoned,
                    staged_token.expected_content(),
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
                    saved_token.transient(),
                    ExpectedState::Deleted,
                    saved_token.expected_content(),
                );
                Ok(deleted_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only delete tokens in ExpectedState::Saved, got {:?}",
                other
            ))),
        }
    }

    // ---------- Create tokens based on matching conditions ----------

    pub fn add_token(
        &mut self,
        token: TestReference,
        key: Option<MapString>,
    ) -> Result<TestReference, HolonError> {
        match token.expected_state() {
            ExpectedState::Transient => {
                if let Some(key) = key {
                    // Mint a transient-intent token indexed by key.
                    Ok(self.add_transient_with_key(
                        token.transient(),
                        key,
                        token.expected_content(),
                    )?)
                } else {
                    // Mint a transient-intent token without a key.
                    Ok(self.add_transient(token.transient(), token.expected_content()))
                }
            }
            ExpectedState::Staged => {
                if let Some(key) = key {
                    // Mint a staged-intent token indexed by key.
                    Ok(self.add_staged_with_key(
                        token.transient(),
                        key,
                        token.expected_content(),
                    )?)
                } else {
                    // Mint a staged-intent token without a key.
                    Ok(self.add_staged(token.transient(), token.expected_content()))
                }
            }
            _ => Err(HolonError::InvalidParameter(
                "Can only add a Transient or Staged token".to_string(),
            )),
        }
    }

    // ---------- Create tokens (no key indexing) ----------

    /// Create and retain a **Transient** token from a `TransientReference`.
    pub fn add_transient(
        &mut self,
        transient_reference: &TransientReference,
        expected_content: &EssentialHolonContent,
    ) -> TestReference {
        self.mint_snapshot(transient_reference, ExpectedState::Transient, expected_content)
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(
        &mut self,
        transient_reference: &TransientReference,
        expected_content: &EssentialHolonContent,
    ) -> TestReference {
        self.mint_snapshot(transient_reference, ExpectedState::Staged, expected_content)
    }

    /// Create and retain a **Saved** token from a `TransientReference`.
    pub fn add_saved(
        &mut self,
        transient_ref: &TransientReference,
        expected_content: &EssentialHolonContent,
    ) -> TestReference {
        self.mint_snapshot(transient_ref, ExpectedState::Saved, expected_content)
    }

    // ---------- Create tokens AND index by key (preferred when you know the key) ----------

    /// Create a **Transient** token and index it by the holon’s key.
    pub fn add_transient_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
        expected_content: &EssentialHolonContent,
    ) -> Result<TestReference, HolonError> {
        self.index_by_key(key, transient_reference.get_temporary_id())?;
        let token = self.add_transient(transient_reference, expected_content);

        Ok(token)
    }

    /// Create a **Staged** token and index it by the holon’s key.
    pub fn add_staged_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
        expected_content: &EssentialHolonContent,
    ) -> Result<TestReference, HolonError> {
        self.index_by_key(key, transient_reference.get_temporary_id())?;
        let token = self.add_staged(transient_reference, expected_content);

        Ok(token)
    }

    /// Index an existing token by key. Errors if the key is present and change is attempted for the corresponding TemporaryId .
    fn index_by_key(&mut self, key: MapString, given_id: TemporaryId) -> Result<(), HolonError> {
        if let Some(current_id) = self.by_key.get(&key) {
            if current_id != &given_id {
                Err(HolonError::InvalidUpdate(format!(
                    "upsert_by_key, since a TemporaryId already exists for key: {:?}, the lineage cannot be changed by this function call.",
                    key
                )))
            } else {
                Ok(())
            }
        } else {
            self.by_key.insert(key, given_id.clone());
            Ok(())
        }
    }

    //  ======  INDEXING  ======  //

    /// Use with Caution...
    /// Upsert variant: replace any existing mapping for `key`.
    /// Prefer `index_by_key` unless you *intend* to overwrite.
    pub fn upsert_by_key(&mut self, key: MapString, id: TemporaryId) {
        self.by_key.insert(key, id);
    }

    // ---------- Retrieval ----------

    /// Retrieve the TemporaryId associated with the given key
    pub fn get_id_by_key(&self, key: &MapString) -> Option<&TemporaryId> {
        self.by_key.get(key)
    }

    /// Retrieve tokens by id
    pub fn get_tokens_by_id(&self, id: &TemporaryId) -> Result<&Vec<TestReference>, HolonError> {
        if let Some(tokens) = self.lineage.get(id) {
            Ok(tokens)
        } else {
            Err(HolonError::InvalidParameter("Lineage not found for id".to_string()))
        }
    }

    /// Retrieve current token for key
    pub fn get_latest_by_key(&self, key: &MapString) -> Result<TestReference, HolonError> {
        let id = if let Some(id) = self.get_id_by_key(key) {
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
        for (_id, token) in &self.latest_snapshots() {
            match token.expected_state() {
                ExpectedState::Transient => counts.transient += 1,
                ExpectedState::Staged => counts.staged += 1,
                ExpectedState::Saved => counts.saved += 1,
                ExpectedState::Abandoned => counts.staged -= 1,
                ExpectedState::Deleted => {}
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
