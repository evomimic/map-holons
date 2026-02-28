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
use std::fmt;
use holons_core::{
    ReadableHolon, core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference
};

/// Stable identity for a fixture snapshot across execution.
/// Used as the unifying key for:
/// - execution-time resolution
/// - chaining between steps
/// - end-of-test validation (e.g. match_db_content)
pub type SnapshotId = TemporaryId;

/// Declarative intent for a test-scoped reference.
///
/// - `Transient`: the holon is expected to be a transient snapshot at this point in the flow.
/// - `Staged`: the holon is expected to be staged (pre-commit).
/// - `Saved`: the holon is expected to be committed (post-commit).
///
/// Notes:
/// - A new token is minted with a unique id for each snapshot representation of a state change.
/// - FixtureHolons::commit() will mint a saved-intent (ie saved state) token for each staged-intent token whose previous snapshot is not either Abandoned or Saved.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TestHolonState {
    Transient,
    Staged,
    Saved,
    Abandoned,
    Deleted,
}

impl fmt::Display for TestHolonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestHolonState::Transient => write!(f, "Transient"),
            TestHolonState::Staged => write!(f, "Staged"),
            TestHolonState::Saved => write!(f, "Saved"),
            TestHolonState::Abandoned => write!(f, "Abandoned"),
            TestHolonState::Deleted => write!(f, "Deleted"),
        }
    }
}

/// An **immutable, opaque fixture token** that is safe to pass and reuse, as the sole artifact executors receive.
/// and expresses its intended lifecycle via [`TestHolonState`].
/// A TestReference bundles two phase-separated roles:
/// - **Source snapshot** — used only to resolve runtime input before a step executes
/// - **Expected snapshot** — used only to validate outcomes after execution
///
/// These roles are never used simultaneously and are bundled only for fixture authoring convenience.
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

impl fmt::Display for TestReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestReference(source:{}@{}, expected:{}@{})",
            self.source.id(),
            self.source.state(),
            self.expected.id(),
            self.expected.state()
        )
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
    snapshot: TransientReference, // Carries the expected content, which can be mutated, except for Deleted state it's an ID only.
    state: TestHolonState,        // Transient | Staged | Saved | Abandoned | Deleted
}

impl ExpectedSnapshot {

    /// Converts this expected snapshot into the source snapshot for the *next* execution step.
    ///
    /// This encodes the core chaining invariant:
    /// the expected outcome of one step becomes the input to the next.
    pub fn as_source(&self) -> SourceSnapshot {
        SourceSnapshot::new(self.snapshot.clone(), self.state)
    }

    pub fn essential_content(
        &self,
        
    ) -> Result<EssentialHolonContent, HolonError> {
        self.snapshot.essential_content()
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
