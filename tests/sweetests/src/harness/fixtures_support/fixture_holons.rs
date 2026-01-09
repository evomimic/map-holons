use core_types::HolonError;
use holons_core::{reference_layer::TransientReference, HolonsContextBehavior, ReadableHolon};
use tracing::debug;
// use tracing::warn;

use crate::harness::fixtures_support::{ExpectedState, TestReference};

/// Fixture-time factory + registry for [`TestReference`]s.
///
/// - **Only** `FixtureHolons` can mint tokens (it calls `TestReference::new`, which is `pub(crate)`).
/// - `commit()` flips all *Staged* intents to *Saved* for **expectation** purposes only.
///
///  Each token maps to an ExecutionHolon -- the expected runtime resolution.
#[derive(Clone, Debug, Default)]
pub struct FixtureHolons {
    tokens: Vec<TestReference>,
}

impl FixtureHolons {
    /// Create an empty container.
    pub fn new() -> Self {
        Self::default()
    }

    // =====  COMMIT  ======  //

    /// Mint tokens with expected state Saved
    pub fn commit(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Vec<TestReference>, HolonError> {
        let mut saved_tokens = Vec::new();

        for token in self.tokens.iter() {
            match token.expected_state() {
                ExpectedState::Staged => {
                    // Cloning source in order to create a new fixture holon
                    let expected_content = token.expected_content().clone_holon(context)?;
                    // Mint saved
                    let saved_token = TestReference::new(ExpectedState::Saved, expected_content);
                    // Return tokens for passing to executor used for building ResolvedTestReference
                    saved_tokens.push(saved_token);
                }
                ExpectedState::Abandoned => {
                    debug!("Skipping commit on Abandoned Holon: {:#?}", token)
                }
                ExpectedState::Transient => {
                    debug!(
                        "Latest state is not staged, skipping commit on Transient : {:#?}",
                        token
                    );
                }
                ExpectedState::Saved => {
                    debug!("Holon already saved : {:#?}", token);
                }
                ExpectedState::Deleted => {
                    debug!("Holon marked as deleted : {:#?}", token);
                }
            }
        }
        // Update FixtureHolons
        self.tokens.extend(saved_tokens.clone());

        Ok(saved_tokens)
    }

    // // ==== MINTING ==== // //

    /// Mint a new TestReference snapshot and push it onto FixtureHolons
    ///
    /// - `expected_content` is a frozen snapshot of the fixture holon
    /// - `expected_state` is the lifecycle intent after the step
    ///
    /// Returns the newly created TestReference.
    pub fn mint_snapshot(
        &mut self,
        expected_state: ExpectedState,
        expected_content: TransientReference,
    ) -> TestReference {
        let token = TestReference::new(expected_state, expected_content);
        self.tokens.push(token.clone());

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
                let deleted_token = self
                    .mint_snapshot(ExpectedState::Deleted, saved_token.expected_content().clone());
                Ok(deleted_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only delete tokens in ExpectedState::Saved, got {:?}",
                other
            ))),
        }
    }

    // ---------- Create tokens  ----------

    // Mints a new token based on matching source type 
    pub fn add_token(
        &mut self,
        expected_state: ExpectedState,
        next_snapshot: TransientReference,
    ) -> Result<TestReference, HolonError> {
        match expected_state {
            ExpectedState::Transient => Ok(self.add_transient(next_snapshot)),
            ExpectedState::Staged => Ok(self.add_staged(next_snapshot)),
            _ => Err(HolonError::InvalidParameter(
                "Can only add a Transient or Staged token".to_string(),
            )),
        }
    }

    /// Create and retain a **Transient** token from a `TransientReference`.
    pub fn add_transient(&mut self, expected_content: TransientReference) -> TestReference {
        self.mint_snapshot(ExpectedState::Transient, expected_content)
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(&mut self, expected_content: TransientReference) -> TestReference {
        self.mint_snapshot(ExpectedState::Staged, expected_content)
    }

    // ---- HELPERS ---- //

    // Gets number of Holons per type of ExpectedState in FixtureHolons
    pub fn counts(&self) -> FixtureHolonCounts {
        let mut counts = FixtureHolonCounts::default();
        for token in &self.tokens {
            let expected_state = token.expected_state();
            match expected_state {
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
