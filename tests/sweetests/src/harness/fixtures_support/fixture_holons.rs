use core_types::HolonError;
use holons_core::{reference_layer::TransientReference, HolonsContextBehavior, ReadableHolon};
use tracing::debug;
// use tracing::warn;

use crate::harness::fixtures_support::{IntendedResolvedState, TestReference};

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
            match token.intended_resolved_state() {
                IntendedResolvedState::Staged => {
                    // check to make sure the token_id is not associated with an abandoned or commit step
                    // or is not the result of a mint from a modification step on a staged
                    let skip = self.tokens.iter().any(|tr| {
                        tr.previous() == token.token_id()
                            && matches!(
                                tr.intended_resolved_state(),
                                IntendedResolvedState::Saved
                                    | IntendedResolvedState::Abandoned
                                    | IntendedResolvedState::Staged
                            )
                    });
                    if skip {
                        debug!("Skipping commit on Holon :{:#?}, where previous was either Abandoned or Saved", token);
                    } else {
                        // Cloning source in order to create a new fixture holon
                        let token_id = token.token_id().clone_holon(context)?;
                        // Mint saved
                        let saved_token = TestReference::new(
                            token.token_id(),
                            IntendedResolvedState::Saved,
                            token_id,
                        );
                        // Return tokens for passing to executor used for building ResolvedTestReference
                        saved_tokens.push(saved_token);
                    }
                }
                IntendedResolvedState::Abandoned => {
                    debug!("Skipping commit on Abandoned Holon: {:#?}", token);
                }
                IntendedResolvedState::Transient => {
                    debug!(
                        "Latest state is not staged, skipping commit on Transient : {:#?}",
                        token
                    );
                }
                IntendedResolvedState::Saved => {
                    debug!("Holon already saved : {:#?}", token);
                }
                IntendedResolvedState::Deleted => {
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
    /// - `token_id` is used to identify the token and contains a frozen snapshot of the fixture holon's essential content
    /// - `intended_resolved_state` is the lifecycle intent after the step
    ///
    /// Returns the newly created TestReference.
    pub fn mint_snapshot(
        &mut self,
        previous: TransientReference,
        intended_resolved_state: IntendedResolvedState,
        token_id: TransientReference,
    ) -> TestReference {
        let token = TestReference::new(previous, intended_resolved_state, token_id);
        self.tokens.push(token.clone());

        token
    }

    /// Mint an IntendedResolvedState::Abandoned cloned from given TestReference (must be intended_resolved_state Staged)
    pub fn abandon_staged(
        &mut self,
        source_token: &TestReference,
        new_id: TransientReference,
    ) -> Result<TestReference, HolonError> {
        match source_token.intended_resolved_state() {
            IntendedResolvedState::Staged => {
                let abandoned_token = self.mint_snapshot(
                    source_token.token_id(),
                    IntendedResolvedState::Abandoned,
                    new_id,
                );
                Ok(abandoned_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only abandon tokens in IntendedResolvedState::Staged, got {:?}",
                other
            ))),
        }
    }

    /// Mint an IntendedResolvedState::Deleted cloned from given TestReference (must be intended_resolved_state Saved)
    pub fn delete_saved(
        &mut self,
        saved_token: &TestReference,
        new_id: TransientReference,
    ) -> Result<TestReference, HolonError> {
        match saved_token.intended_resolved_state() {
            IntendedResolvedState::Saved => {
                let deleted_token = self.mint_snapshot(
                    saved_token.token_id(),
                    IntendedResolvedState::Deleted,
                    new_id,
                );
                Ok(deleted_token)
            }
            other => Err(HolonError::InvalidTransition(format!(
                "Can only delete tokens in IntendedResolvedState::Saved, got {:?}",
                other
            ))),
        }
    }

    // ---------- Create tokens  ----------

    // Mints a new token based on matching source type
    pub fn add_token(
        &mut self,
        previous_snapshot: TransientReference,
        intended_resolved_state: IntendedResolvedState,
        next_snapshot: TransientReference,
    ) -> Result<TestReference, HolonError> {
        match intended_resolved_state {
            IntendedResolvedState::Transient => {
                Ok(self.add_transient(previous_snapshot, next_snapshot))
            }
            IntendedResolvedState::Staged => Ok(self.add_staged(previous_snapshot, next_snapshot)),
            _ => Err(HolonError::InvalidParameter(
                "Can only add a Transient or Staged token".to_string(),
            )),
        }
    }

    /// Create and retain a **Transient** token from a `TransientReference`.
    pub fn add_transient(
        &mut self,
        previous: TransientReference,
        token_id: TransientReference,
    ) -> TestReference {
        self.mint_snapshot(previous, IntendedResolvedState::Transient, token_id)
    }

    /// Create and retain a **Staged** token from a `TransientReference`.
    pub fn add_staged(
        &mut self,
        previous: TransientReference,
        token_id: TransientReference,
    ) -> TestReference {
        self.mint_snapshot(previous, IntendedResolvedState::Staged, token_id)
    }

    // ---- HELPERS ---- //

    // Gets number of Holons per type of IntendedResolvedState in FixtureHolons
    pub fn counts(&self) -> FixtureHolonCounts {
        let mut counts = FixtureHolonCounts::default();
        for token in &self.tokens {
            let intended_resolved_state = token.intended_resolved_state();
            match intended_resolved_state {
                IntendedResolvedState::Transient => counts.transient += 1,
                IntendedResolvedState::Staged => counts.staged += 1,
                IntendedResolvedState::Saved => counts.saved += 1,
                IntendedResolvedState::Abandoned => counts.staged -= 1,
                IntendedResolvedState::Deleted => counts.saved -= 1,
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
