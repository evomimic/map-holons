//! Execution-time realization of a fixture token.
//!
//! A [`ResolvedTestReference`] pairs the fixture-declared **source token**
//! (what the fixture expected at this point in the flow) with the **runtime
//! handle** actually produced by executing a step.
//!
//! - `source_token`: the [`TestReference`] declared by the fixture. Its
//!   `ExpectedState` describes the lifecycle of the *source* holon
//!   (Transient, Staged, or Saved).
//! - `resulting_reference`: the [`HolonReference`] created at runtime
//!   (often a `StagedReference`; if committed, represents “Saved”).
//!
//! ⚠ Important: **Do not confuse source and result.**
//! A “Staged” token may resolve to a *new* staged holon, not the one
//! embedded in the token. The token is intent; the result is reality.
use crate::harness::fixtures_support::TestReference;
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_prelude::prelude::*;
use pretty_assertions::assert_eq;

#[derive(Clone, Debug)]
pub struct ResolvedTestReference {
    /// Fixture-declared identity + intent of the source holon.
    pub source_token: TestReference,
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

// #[derive(Clone, Debug)]
// pub enum ResultingReference {
//     Transient(HolonReference),
//     Staged(HolonReference),
//     Saved(HolonReference),
//     Abandoned(HolonReference), // Still a StagedReference but marked as 'Abandoned'
//     Deleted,
// }

impl ResolvedTestReference {
    /// Build from a fixture token and the resulting runtime handle.
    pub fn from_reference_parts(
        source_token: TestReference,
        resulting_reference: ResultingReference,
    ) -> Self {
        Self { source_token, resulting_reference }
    }

    /// Assert that the essential content of the fixture-declared source
    /// matches the essential content of the runtime result.
    ///
    /// This reconstructs the transient from the source token, compares it
    /// against the actual `resulting_reference`, and errors if they differ.
    pub fn assert_essential_content_eq(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        let expected_ref = HolonReference::Transient(self.source_token.transient().clone());
        let expected_content = expected_ref.essential_content(context)?;
        let actual_content = self.resulting_reference.essential_content(context)?;

        assert_eq!(expected_content, actual_content);
        Ok(())
    }
}
