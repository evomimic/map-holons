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

/// Stable identity for a fixture-time snapshot, used as the key for snapshot ownership and resolution.
/// Alias for the TemporaryId extracted from the snapshot's TransientReference.
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

/// An **immutable, opaque fixture token** that is safe to pass and reuse, as the sole artifact executors receive.
/// and expresses its intended lifecycle via [`TestHolonState`].
/// Conceptually, a TestReference captures two things:
/// Starting point — what the step should operate on
/// Expected result — what the step should produce
///
/// These two roles travel together as a single immutable contract, where one or multiple are used for each TestStep.
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

    pub fn expected_id(&self) -> SnapshotId {
        self.expected.id()
    }

    pub fn expected_reference(&self) -> &TransientReference {
        &self.expected.snapshot
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
#[derive(new, Clone, Debug, Eq, PartialEq)]
pub struct ExpectedSnapshot {
    snapshot: TransientReference, // Carries the expected content, which can be mutated, except for Deleted state its an ID only.
    state: TestHolonState,        // Transient | Staged | Saved | Abandoned | Deleted
}

impl ExpectedSnapshot {

    // Conversion helper when using an expected snapshot as the new source.
    pub fn as_source(&self) -> SourceSnapshot {
        SourceSnapshot::new(self.snapshot.clone(), self.state)
    }

    pub fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.snapshot.essential_content(context)
    }

    pub fn id(&self) -> SnapshotId{
        self.snapshot.temporary_id().into()
    }

    pub fn snapshot(&self) -> &TransientReference {
        &self.snapshot
    }

    pub fn state(&self) -> TestHolonState {
        self.state
    }
}
