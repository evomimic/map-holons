//! Execution-time realization of a fixture token.
//!
//! A [`ExecutionReference`] pairs the fixture-declared **mapping token**
//! (what the fixture expected at this point in the flow) with the **runtime
//! handle** actually produced by executing a step.
//!
//! - `fixture_token`: the [`TestReference`] declared by the fixture. Its
//!   `TestHolonState` describes the lifecycle of the *mapping* holon
//!   (Transient, Staged, or Saved).
//! - `resulting_reference`: the [`HolonReference`] created at runtime
//!   (often a `StagedReference`; if committed, represents “Saved”).
//!
//! ⚠ Important: **Do not confuse intent and result.**
//! A “Staged” token may resolve to a *new* staged holon, not the one
//! embedded in the token. The token is intent; the result is reality.

use crate::ExpectedSnapshot;
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_prelude::prelude::*;
use pretty_assertions::assert_eq;

#[derive(Clone, Debug)]
pub struct ExecutionReference {
    /// Fixture-declared intent of the expected snapshot, which includes expected content.
    pub expected_snapshot: ExpectedSnapshot,
    /// Runtime handle produced by executing the step.
    pub resulting_reference: ResultingReference,
}

#[derive(Clone, Debug)]
pub enum ResultingReference {
    LiveReference(HolonReference),
    Deleted,
}

impl ResultingReference {
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

impl From<HolonReference> for ResultingReference {
    fn from(reference: HolonReference) -> Self {
        Self::LiveReference(reference)
    }
}

impl ExecutionReference {
    /// Build from a fixture token and the resulting runtime handle.
    pub fn from_reference_parts(
        expected_snapshot: ExpectedSnapshot,
        resulting_reference: ResultingReference,
    ) -> Self {
        Self { expected_snapshot, resulting_reference }
    }

    /// Assert that the essential content of the fixture-declared source
    /// matches the essential content of the runtime result.
    ///
    /// This reconstructs the expected_token 'snapshot', compares it
    /// against the actual `resulting_reference`, and errors if they differ.
    pub fn assert_essential_content_eq(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        let expected_content = self.expected_snapshot.essential_content(context)?;
        let actual_content = self.resulting_reference.essential_content(context)?;

        // TODO: find a way to compare relationships

        assert_eq!(expected_content, actual_content);

        Ok(())
    }
}
