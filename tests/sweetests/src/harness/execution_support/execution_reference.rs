//! Execution-time realization of a fixture token.
//!
//! A [`ExecutionReference`] pairs the fixture-declared **expected snapshot**
//! (what the fixture expected at this point in the flow) with the **runtime
//! handle** actually produced by executing a step.
//!
//! - `expected_snapshot`: the [`ExpectedSnapshot`] declared by the fixture. Its
//!   `TestHolonState` describes the lifecycle of the *mapping* holon
//!   (Transient, Staged, or Saved).
//! - `execution_reference`: the [`HolonReference`] created at runtime
//!   (often a `StagedReference`; if committed, represents “Saved”).
//!
//! ⚠ Important: **Do not confuse intent and result.**
//! The expected snapshot that comes from the exectuor input token is intent; the resulting reference is 'DHT' reality.

use crate::{ExpectedSnapshot, TestReference};
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_prelude::prelude::*;
use pretty_assertions::assert_eq;

#[derive(Clone, Debug)]
pub struct ExecutionReference {
    /// Fixture-declared intent of the expected snapshot, which includes expected content.
    pub expected_snapshot: ExpectedSnapshot,
    /// Runtime handle produced by executing the step.
    pub execution_handle: ExecutionHandle,
}

#[derive(Clone, Debug)]
pub enum ExecutionHandle {
    LiveReference(HolonReference),
    Deleted,
}

impl ExecutionHandle {
    pub fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        match self {
            Self::LiveReference(holon_reference) => holon_reference.essential_content(context),
            Self::Deleted => Err(HolonError::InvalidParameter(
                "Holon is marked as deleted, there is no content to compare".to_string(),
            )),
        }
    }

    pub fn get_holon_reference(&self) -> Result<HolonReference, HolonError> {
        match self {
            Self::LiveReference(holon_reference) => Ok(holon_reference.clone()),
            Self::Deleted => Err(HolonError::InvalidParameter(
                "Holon is marked as deleted, there is no associated HolonReference".to_string(),
            )),
        }
    }
}

impl From<HolonReference> for ExecutionHandle {
    fn from(reference: HolonReference) -> Self {
        Self::LiveReference(reference)
    }
}

impl ExecutionReference {
    /// Canonical constructor for executors.
    ///
    /// Binds fixture intent (via TestReference) to the execution-time handle
    /// produced by running a step.
    ///
    /// Executors MUST use this method.
    pub fn from_token_execution(
        token: &TestReference,
        execution_handle: ExecutionHandle,
    ) -> Self {
        Self {
            expected_snapshot: token.expected_snapshot(),
            execution_handle,
        }
    }

    /// Assert that execution-time state matches fixture-declared expectations.
    ///
    /// This is a **test assertion helper**, not a fallible API:
    /// - Panics if expected content cannot be read
    /// - Panics if execution-time content cannot be read
    /// - Panics if the two do not match
    ///
    /// Intended for use by test executors to enforce fixture invariants.
    /// A mismatch indicates a test failure, not a recoverable error.
    pub fn assert_essential_content_eq(
        &self,
        context: &dyn HolonsContextBehavior,
    ) {
        let expected_content = self
            .expected_snapshot
            .essential_content(context)
            .expect("failed to read expected snapshot content");

        let actual_content = self
            .execution_handle
            .essential_content(context)
            .expect("failed to read execution-time content");

        assert_eq!(expected_content, actual_content);
    }
}
