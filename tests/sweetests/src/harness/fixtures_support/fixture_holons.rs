use std::collections::BTreeMap;

use base_types::MapString;
use core_types::HolonError;
use holons_core::reference_layer::TransientReference;

use crate::harness::fixtures_support::{ExpectedState, TestReference};

/// Fixture-time factory + registry for [`TestReference`]s.
///
/// - **Only** `FixtureHolons` can mint tokens (it calls `TestReference::new`, which is `pub(crate)`).
/// - Optional lookup by **key** (`MapString`) for fixtures that stage well-known holons
///   and want to retrieve their tokens later by key.
/// - `commit()` flips all *Staged* intents to *Saved* for **expectation** purposes only.
#[derive(Clone, Debug, Default)]
pub struct FixtureHolons {
    test_references: Vec<TestReference>,
    by_key: BTreeMap<MapString, TestReference>,
}

impl FixtureHolons {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    // ---------- Create tokens (no key indexing) ----------

    /// Create and retain a **Transient** token from a `TransientReference`.
    pub fn add_transient(&mut self, transient_reference: &TransientReference) -> TestReference {
        let token = TestReference::new(transient_reference.clone(), ExpectedState::Transient);
        self.test_references.push(token.clone());
        token
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(&mut self, transient_reference: &TransientReference) -> TestReference {
        let token = TestReference::new(transient_reference.clone(), ExpectedState::Staged);
        self.test_references.push(token.clone());
        token
    }

    // ---------- Create tokens AND index by key (preferred when you know the key) ----------

    /// Create a **Transient** token and index it by the holon’s key.
    pub fn add_transient_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
    ) -> Result<TestReference, HolonError> {
        let token = self.add_transient(transient_reference);
        self.index_by_key(token.clone(), key)?;
        Ok(token)
    }

    /// Create a **Staged** token and index it by the holon’s key.
    pub fn add_staged_with_key(
        &mut self,
        transient_reference: &TransientReference,
        key: MapString,
    ) -> Result<TestReference, HolonError> {
        let token = self.add_staged(transient_reference);
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
        self.by_key.insert(key, test_reference);
        Ok(())
    }

    /// Upsert variant: replace any existing mapping for `key`.
    /// Prefer `index_by_key` unless you *intend* to overwrite.
    pub fn upsert_by_key(&mut self, test_reference: TestReference, key: MapString) {
        self.by_key.insert(key, test_reference);
    }

    // ---------- Retrieval by key ----------

    /// Retrieve a token by key (clone returned for convenience).
    pub fn get_by_key(&self, key: &MapString) -> Option<TestReference> {
        self.by_key.get(key).cloned()
    }

    /// Retrieve multiple tokens by keys, returning the first missing key as an error.
    pub fn get_many_by_keys(&self, keys: &[MapString]) -> Result<Vec<TestReference>, HolonError> {
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            let token = self.by_key.get(key).cloned().ok_or_else(|| {
                HolonError::DuplicateError(
                    "Keys".to_string(),
                    format!("FixtureHolon with key: {:?}", key),
                )
            })?;
            result.push(token);
        }
        Ok(result)
    }

    // ---------- Commit + counts + views ----------

    /// Bulk flip: convert all **Staged** tokens currently in this container to **Saved**.
    /// Returns the number of tokens flipped.
    pub fn commit(&mut self) -> usize {
        let mut flipped = 0usize;
        for token in &mut self.test_references {
            if matches!(token.expected_state(), ExpectedState::Staged) {
                token.set_expected_state(ExpectedState::Saved);
                flipped += 1;
            }
        }
        flipped
    }

    pub fn counts(&self) -> FixtureHolonCounts {
        let mut counts = FixtureHolonCounts::default();
        for token in &self.test_references {
            match token.expected_state() {
                ExpectedState::Transient => counts.transient += 1,
                ExpectedState::Staged => counts.staged += 1,
                ExpectedState::Saved => counts.saved += 1,
            }
        }
        counts
    }

    pub fn count_transient(&self) -> usize {
        self.counts().transient
    }
    pub fn count_staged(&self) -> usize {
        self.counts().staged
    }
    pub fn count_saved(&self) -> usize {
        self.counts().saved
    }

    /// All tokens whose expected state is **Saved** (useful for MatchSavedContent).
    pub fn expected_saved_references(&self) -> Vec<TestReference> {
        self.test_references
            .iter()
            .filter(|t| matches!(t.expected_state(), ExpectedState::Saved))
            .cloned()
            .collect()
    }

    /// Borrow the full list (read-only).
    pub fn all(&self) -> &[TestReference] {
        &self.test_references
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FixtureHolonCounts {
    pub transient: usize,
    pub staged: usize,
    pub saved: usize,
}
