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
//! The expected snapshot that comes from the executor input token is intent; the resulting reference is 'DHT' reality.

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
    pub fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        match self {
            Self::LiveReference(holon_reference) => holon_reference.essential_content(),
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
    pub fn from_token_execution(token: &TestReference, execution_handle: ExecutionHandle) -> Self {
        Self { expected_snapshot: token.expected_snapshot(), execution_handle }
    }

    /// Assert that execution-time state matches fixture-declared expectations.
    ///
    /// This is a **test assertion helper**, not a fallible API:
    /// which panics if any of the following occurs:
    /// - expected content cannot be read
    /// - execution-time content cannot be read
    /// - expected vs actual content does not match
    /// in relationship_map of expected vs actual:
    /// - for each relationship_name the length of members in the collection do not match
    /// - in each collection there is not an exhaustive list where a target holon exists whos essential content matches the other
    ///
    /// Intended for use by test executors to enforce fixture invariants.
    /// A mismatch indicates a test failure, not a recoverable error.
    pub fn assert_essential_content_eq(&self) {
        // Content //
        let expected_content = self
            .expected_snapshot
            .essential_content()
            .expect("failed to read expected snapshot content");
        let actual_content = self
            .execution_handle
            .essential_content()
            .expect("failed to read execution-time content");
        assert_eq!(expected_content, actual_content);

        // Relationships //
        let expected_relationship_map = self
            .expected_snapshot
            .snapshot()
            .all_related_holons()
            .expect("Failed to get all related holons");
        let actual_relationship_map = self
            .execution_handle
            .get_holon_reference()
            .expect("Failed to get HolonReference for execution_handle")
            .all_related_holons()
            .expect("Failed to get all related holons");
        assert_eq!(expected_relationship_map.count(), actual_relationship_map.count());
        // Nested loop 'brute force' approach:
        for (name, expected_collection_arc) in expected_relationship_map.iter() {
            let expected_collection =
                expected_collection_arc.write().expect("Failed to Failed to acquire write lock");
            let expected_members = expected_collection.get_members();
            let actual_collection_arc = actual_relationship_map.get_collection_for_relationship(&name).expect(&format!("There must be an associated collection in the actual_relationship_map for {:?} in the expected_relationship_map", name));
            let actual_collection =
                actual_collection_arc.write().expect("Failed to acquire write lock");
            // Clone actual to establish exhaustive list
            let mut actual_members = actual_collection.get_members().clone();
            for expected_holon in expected_members {
                let expected_content = expected_holon
                    .essential_content()
                    .expect("Failed to read expected holon content");
                // Find matching holon
                let matching_holon = actual_collection.get_members().iter().find(|actual_member| {
                    let actual_content = actual_member
                        .essential_content()
                        .expect("Failed to read actual holon content");

                    actual_content == expected_content
                });
                if let Some(holon) = matching_holon {
                    // Remove matched element so it cannot match again
                    actual_members.retain(|h| h != holon);
                } else {
                    panic!(
                            "Expected member with content {:#?} not found in actual collection\n for relationship {:?}",
                            expected_content, name
                        );
                }
            }
            assert!(
                actual_members.is_empty(),
                "Members in actual_collection did not get exhausted"
            );
        }
    }
}
