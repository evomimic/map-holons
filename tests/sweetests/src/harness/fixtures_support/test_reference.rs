//! Fixture-time tokens for referring to holons in test cases.
//!
//! # Overview
//! - [`IntendedResolvedState`] expresses the **intended lifecycle** of a holon at a
//!   specific point in a test case: `Transient`, `Staged`, or `Saved`.
//! - [`TestReference`] is an **opaque fixture-time token** that contains a
//!   portable [`TransientReference`] plus an [`IntendedResolvedState`].
//!
//! ## Why a token?
//! Fixtures declare *intent* but must not couple themselves to runtime handles
//! (`StagedReference`, smart references, etc.). A `TestReference` lets fixtures
//! pass “what this should be by the time this step runs” without exposing or
//! depending on execution-time objects. The actual handles are produced and
//! tracked during execution (see `execution_support`).
//!
//! ## Construction and visibility
//! - Fixtures **cannot** construct `TestReference` directly.
//! - Tokens are minted **only** by `FixtureHolons` (the factory/registry) via pub(crate) exposure only.
//! - All fields are private.
//! - Tokens are immutable, representing a frozen "snapshot".

use holons_core::reference_layer::TransientReference;

/// Declarative intent for a test-scoped reference.
///
/// - `Transient`: the holon is expected to be a transient snapshot at this point in the flow.
/// - `Staged`: the holon is expected to be staged (pre-commit).
/// - `Saved`: the holon is expected to be committed (post-commit).
///
/// Notes:
/// - A new token is minted with a unique id for each snapshot representation of a state change.
/// - FixtureHolons::commit() will mint a saved-intent (ie saved state) token for each staged-intent token who's previous snapshot is not either Abandoned or Saved.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum IntendedResolvedState {
    Transient,
    Staged,
    Saved,
    Abandoned,
    Deleted,
}

/// An **opaque fixture token** that identifies a holon by [`TransientReference`]
/// and expresses its intended lifecycle via [`IntendedResolvedState`].
///
/// From a fixture’s perspective, this is just a *token*:
/// - No direct construction or mutation (use [`FixtureHolons`](super::fixture_holons::FixtureHolons)
///   to obtain tokens).
/// - Passed into `add_*_step` functions to declare which holon a step should act on
///   and what state it is expected to be in when that step executes.
///
/// Internally, harness code can access identity and intent in order to:
/// - Resolve the token into a `HolonReference`.
/// - Verify and assert state transitions.
/// - Update intent by minting a new token that points back to the previous snapshot token_id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestReference {
    previous: TransientReference, // back pointer to previous snapshot
    intended_resolved_state: IntendedResolvedState, // Transient | Staged | Saved | Abandoned | Deleted
    token_id: TransientReference, // carries the TemporaryId used to resolve the ExecutionHolon and expected essential content
}

impl TestReference {
    /// Crate-internal constructor. Only [`FixtureHolons`] may mint tokens.
    pub fn new(
        previous: TransientReference,
        intended_resolved_state: IntendedResolvedState,
        token_id: TransientReference,
    ) -> Self {
        Self { previous, intended_resolved_state, token_id }
    }

    pub fn previous(&self) -> TransientReference {
        self.previous.clone()
    }

    pub fn intended_resolved_state(&self) -> IntendedResolvedState {
        self.intended_resolved_state
    }

    pub fn token_id(&self) -> TransientReference {
        self.token_id.clone()
    }
}
