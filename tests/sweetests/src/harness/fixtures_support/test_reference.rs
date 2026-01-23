//! Fixture-time tokens for referring to holons in test cases.
//!
//! # Overview
//! - [`TestHolonState`] expresses the **intended lifecycle** of a holon at a
//!   specific point in a test case: `Transient`, `Staged`, or `Saved`.
//! - [`TestReference`] is an **opaque fixture-time token** that contains a
//!   portable [`TransientReference`] plus an [`TestHolonState`].
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

use core_types::{HolonError, TemporaryId};
use derive_new::new;
use holons_core::{
    core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference,
    HolonsContextBehavior, ReadableHolon,
};

/// Alias used throughout the harness docs for readability.
///
/// Concretely this is the `TemporaryId` carried by `ExpectedSnapshot.snapshot`
/// (or whatever the harness defines as the "snapshot id" for a TestReference).
pub type SnapshotId = TemporaryId;

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
pub enum TestHolonState {
    Transient,
    Staged,
    Saved,
    Abandoned,
    Deleted,
}

/// An **opaque fixture token** that identifies a holon by [`TransientReference`]
/// and expresses its intended lifecycle via [`TestHolonState`].
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
    source: SourceSnapshot,
    expected: ExpectedSnapshot,
}

impl TestReference {
    /// Crate-internal constructor. Only [`FixtureHolons`] may mint tokens.
    pub(crate) fn new(source: SourceSnapshot, expected: ExpectedSnapshot) -> Self {
        Self { source, expected }
    }

    pub fn source_snapshot(&self) -> SourceSnapshot {
        self.source.clone()
    }

    pub fn source_id(&self) -> SnapshotId {
        self.source.id()
    }

    pub fn source_reference(&self) -> &TransientReference {
        &self.source.snapshot
    }

    pub fn expected_snapshot(&self) -> ExpectedSnapshot {
        self.expected.clone()
    }

    pub fn expected_id(&self) -> Result<SnapshotId, HolonError> {
        self.expected.id()
    }

    pub fn expected_reference(&self) -> &TransientReference {
        &self.source.snapshot
    }
}

/// Input to the execution step.
#[derive(new, Clone, Debug, Eq, PartialEq)]
pub struct SourceSnapshot {
    snapshot: TransientReference, // immutable snapshot of token
    state: TestHolonState,        // Transient | Staged | Saved | Abandoned | Deleted
}

impl SourceSnapshot {
    pub fn id(&self) -> SnapshotId {
        self.snapshot.temporary_id().into()
    }

    pub fn snapshot(&self) -> &TransientReference {
        &self.snapshot
    }

    pub fn state(&self) -> TestHolonState {
        self.state
    }
}

/// Defines what is expected after the execution step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedSnapshot {
    snapshot: Option<TransientReference>, // Carries expected content, None for Deleted state
    state: TestHolonState,                // Transient | Staged | Saved | Abandoned | Deleted
}

impl ExpectedSnapshot {
    // Custom constructor with guards to prevent incorrect construction of object match incompatability.
    pub fn new(
        snapshot: Option<TransientReference>,
        state: TestHolonState,
    ) -> Result<Self, HolonError> {
        if (snapshot.is_some() && state == TestHolonState::Deleted) 
            || (snapshot.is_none() && state != TestHolonState::Deleted)
        {
            return Err(HolonError::InvalidParameter(
                "Construction of ExpectedSnapshot in a deleted state cannot contain a snapshot"
                    .to_string(),
            ));
        }

        Ok(Self { snapshot, state })
    }


    pub fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        if let Some(tr) = &self.snapshot {
            tr.essential_content(context)
        } else {
            Err(HolonError::HolonNotFound(
                "Snapshot is None... cannot call essential_content".to_string(),
            ))
        }
    }

    pub fn id(&self) -> Result<SnapshotId, HolonError> {
        if let Some(tr) = &self.snapshot {
            Ok(tr.temporary_id().into())
        } else {
            Err(HolonError::HolonNotFound("Snapshot is None, there is no id.".to_string()))
        }
    }

    pub fn state(&self) -> TestHolonState {
        self.state
    }
}
