use std::collections::BTreeMap;

use base_types::MapString;
use core_types::{HolonError, TemporaryId};
use holons_core::{
    core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference,
};

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
    by_key: BTreeMap<MapString, Vec<TestReference>>,
}

impl FixtureHolons {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn abandon_staged(
        &mut self,
        staged_token: &TestReference,
    ) -> Result<TestReference, HolonError> {
        match staged_token.expected_state() {
            ExpectedState::Staged => {
                let mut abandoned_token = staged_token.clone();
                abandoned_token.set_expected_state(ExpectedState::Abandoned);
                // self.test_references.remove(index)
                self.lineage
                    .entry(staged_token.transient().get_temporary_id())
                    .and_modify(|v| v.push(abandoned_token.clone()))
                    .or_insert(vec![abandoned_token.clone()]);
                Ok(abandoned_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only abandon tokens in ExpectedState::Staged, got {:?}",
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
                    Ok(self.add_transient_with_key(token.transient(), key, token.expected_content())?)
                } else {
                    // Mint a transient-intent token without a key.
                    Ok(self.add_transient(token.transient(), token.expected_content()))
                }
            }
            ExpectedState::Staged => {
                if let Some(key) = key {
                    // Mint a staged-intent token indexed by key.
                    Ok(self.add_staged_with_key(token.transient(), key, token.expected_content())?)
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
        let token = TestReference::new(
            transient_reference.clone(),
            ExpectedState::Transient,
            expected_content.clone(),
        );
        self.lineage
            .entry(transient_reference.get_temporary_id())
            .and_modify(|v| v.push(token.clone()))
            .or_insert(vec![token.clone()]);
        token
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(
        &mut self,
        transient_reference: &TransientReference,
        expected_content: &EssentialHolonContent,
    ) -> TestReference {
        let token = TestReference::new(
            transient_reference.clone(),
            ExpectedState::Staged,
            expected_content.clone(),
        );
        self.lineage
            .entry(transient_reference.get_temporary_id())
            .and_modify(|v| v.push(token.clone()))
            .or_insert(vec![token.clone()]);
        token
    }

    // ---------- Create tokens AND index by key (preferred when you know the key) ----------

    /// Create a **Transient** token and index it by the holon’s key.
    pub fn add_transient_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
        expected_content: &EssentialHolonContent,
    ) -> Result<TestReference, HolonError> {
        let token = self.add_transient(transient_reference, expected_content);
        self.index_by_key(token.clone(), key)?;
        Ok(token)
    }

    /// Create a **Staged** token and index it by the holon’s key.
    pub fn add_staged_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
        expected_content: &EssentialHolonContent,
    ) -> Result<TestReference, HolonError> {
        let token = self.add_staged(transient_reference, expected_content);
        self.index_by_key(token.clone(), key)?;
        Ok(token)
    }

    // ---------- Index management ----------

    /// Index an existing token by key. Errors if the key already exists.
    pub fn index_by_key(
        &mut self,
        test_reference: TestReference,
        key: MapString,
    ) -> Result<(), HolonError> {
        if self.by_key.contains_key(&key) {
            return Err(HolonError::DuplicateError(
                "Keys".to_string(),
                format!("FixtureHolon with key: {:?}", key),
            ));
        }
        self.by_key
            .entry(key)
            .and_modify(|v| v.push(test_reference.clone()))
            .or_insert(vec![test_reference]);
        Ok(())
    }

    /// Upsert variant: replace any existing mapping for `key`.
    /// Prefer `index_by_key` unless you *intend* to overwrite.
    pub fn upsert_by_key(&mut self, test_reference: TestReference, key: MapString) {
        self.by_key
            .entry(key)
            .and_modify(|v| v.push(test_reference.clone()))
            .or_insert(vec![test_reference]);
    }

    // ---------- Retrieval by key ----------

    /// Retrieve tokens by key (clone returned for convenience).
    pub fn get_by_key(&self, key: &MapString) -> Option<Vec<TestReference>> {
        self.by_key.get(key).cloned()
    }

    // ---------- Commit + counts + views ----------

    /// Mint token with expected state Saved
    pub fn commit(
        &mut self,
        staged_tokens: Vec<TestReference>,
    ) -> Result<Vec<TestReference>, HolonError> {
        let mut saved_tokens = Vec::new();
        for token in staged_tokens {
            let transient = token.transient();
            match token.expected_state() {
                ExpectedState::Staged => {
                    let saved_token = TestReference::new(
                        transient.clone(),
                        ExpectedState::Saved,
                        token.expected_content().clone(),
                    );
                    self.lineage
                        .entry(transient.get_temporary_id())
                        .and_modify(|v| v.push(saved_token.clone()))
                        .or_insert(vec![saved_token.clone()]);
                    saved_tokens.push(saved_token);
                }
                _ => {
                    return Err(HolonError::CommitFailure(
                        "TestReference to be Saved must be in an ExpectedState::Staged".to_string(),
                    ))
                }
            }
        }
        Ok(saved_tokens)
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FixtureHolonCounts {
    pub transient: i64,
    pub staged: i64,
    pub saved: i64,
}
