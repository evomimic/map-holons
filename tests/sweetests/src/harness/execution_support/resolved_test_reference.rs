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
use core_types::LocalId;
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;

#[derive(Clone, Debug)]
pub struct ResolvedTestReference {
    /// Fixture-declared identity + intent of the source holon.
    pub source_token: TestReference,
    /// Runtime handle produced by executing the step.
    pub resulting_reference: HolonReference,
}

impl ResolvedTestReference {
    /// Build from a fixture token and the resulting runtime handle.
    pub fn from_reference_parts(
        source_token: TestReference,
        resulting_reference: HolonReference,
    ) -> Self {
        Self { source_token, resulting_reference }
    }

    /// True if the resulting handle is a staged holon in committed state.
    pub fn result_is_committed(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<bool, HolonError> {
        Ok(matches!(
            &self.resulting_reference,
            HolonReference::Staged(staged) if staged.is_in_state(context, StagedState::Committed(LocalId(Vec::new())))?))
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

        if expected_content == actual_content {
            Ok(())
        } else {
            Err(HolonError::Misc(format!(
                "Essential content mismatch.\nExpected: {:#?}\nActual:   {:#?}",
                expected_content, actual_content
            )))
        }
    }
}
