//! Fixture-time tokens for referring to holons in test cases.
//!
//! # Overview
//! - [`ExpectedState`] expresses the **intended lifecycle** of a holon at a
//!   specific point in a test case: `Transient`, `Staged`, or `Saved`.
//! - [`TestReference`] is an **opaque fixture-time token** that contains a
//!   portable [`TransientReference`] plus an [`ExpectedState`].
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
//! - Tokens are minted **only** by `FixtureHolons` (the factory/registry).
//! - All fields are private; constructors and accessors are `pub(crate)` so only
//!   harness internals can inspect or mutate them.

use base_types::{MapString, ToBaseValue};
use core_types::TemporaryId;
use holons_core::{
    core_shared_objects::holon::EssentialHolonContent, reference_layer::TransientReference,
    HolonsContextBehavior,
};
use holons_prelude::prelude::ToPropertyName;

/// Declarative intent for a test-scoped reference.
///
/// - `Transient`: the holon is expected to be a transient snapshot at this point in the flow.
/// - `Staged`: the holon is expected to be staged (pre-commit).
/// - `Saved`: the holon is expected to be committed (post-commit).
///
/// Notes:
/// - Fixtures generally create `Transient` or `Staged` intents.
/// - “Saved” is usually derived by a **fixture-time** commit flip (via
///   [`FixtureHolons::commit`](super::fixture_holons::FixtureHolons::commit)),
///   and may also be enforced during execution when resolving tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ExpectedState {
    Transient,
    Staged,
    Saved,
    Abandoned,
    Deleted,
}

/// An **opaque fixture token** that identifies a holon by [`TransientReference`]
/// and expresses its intended lifecycle via [`ExpectedState`].
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
/// - Update intent (e.g., bulk flip staged → saved in fixtures).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestReference {
    root: TransientReference, // carries the TemporaryId used to resolve the ExecutionHolon
    expected_state: ExpectedState, // Transient | Staged | Saved | Abandoned | Deleted
    expected_content: TransientReference, // FixtureHolon pointer with expected essential content, used for comparing expected (fixture) to actual (resolved)
}

impl TestReference {
    /// Crate-internal constructor. Only [`FixtureHolons`] may mint tokens.
    pub(crate) fn new(
        root: TransientReference,
        expected_state: ExpectedState,
        expected_content: TransientReference,
    ) -> Self {
        Self { root, expected_state, expected_content }
    }

    pub fn expected_content(&self) -> &TransientReference {
        &self.expected_content
    }

    pub fn expected_state(&self) -> ExpectedState {
        self.expected_state
    }

    pub fn temporary_id(&self) -> TemporaryId {
        self.expected_content.temporary_id()
    }

    pub fn root(&self) -> &TransientReference {
        &self.root
    }
}
