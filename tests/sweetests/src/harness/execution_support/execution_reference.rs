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
use std::collections::HashSet;

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
        let expected_root = HolonReference::from(self.expected_snapshot.snapshot());
        let actual_root = self
            .execution_handle
            .get_holon_reference()
            .expect("Failed to get HolonReference for execution_handle");
        let mut visited_pairs: HashSet<(String, String)> = HashSet::new();
        Self::compare_holon_graph_eq(&expected_root, &actual_root, &mut visited_pairs)
            .unwrap_or_else(|message| panic!("{}", message));
    }

    fn compare_holon_graph_eq(
        expected: &HolonReference,
        actual: &HolonReference,
        visited_pairs: &mut HashSet<(String, String)>,
    ) -> Result<(), String> {
        let pair_key = Self::pair_key(expected, actual);
        if !visited_pairs.insert(pair_key) {
            return Ok(());
        }

        let expected_content = expected.essential_content().map_err(|e| {
            format!(
                "failed to read expected holon content {}:{}: {:?}",
                expected.reference_kind_string(),
                expected.reference_id_string(),
                e
            )
        })?;
        let actual_content = actual.essential_content().map_err(|e| {
            format!(
                "failed to read actual holon content {}:{}: {:?}",
                actual.reference_kind_string(),
                actual.reference_id_string(),
                e
            )
        })?;
        if expected_content != actual_content {
            return Err(format!(
                "essential content mismatch for expected {}:{} vs actual {}:{}\nexpected: {:#?}\nactual: {:#?}",
                expected.reference_kind_string(),
                expected.reference_id_string(),
                actual.reference_kind_string(),
                actual.reference_id_string(),
                expected_content,
                actual_content
            ));
        }

        let expected_relationship_map =
            match expected.all_related_holons() {
                Ok(map) => Some(map),
                Err(HolonError::NotImplemented(_)) => None,
                Err(e) => {
                    return Err(format!("Failed to get expected all_related_holons: {:?}", e));
                }
            };
        let actual_relationship_map =
            match actual.all_related_holons() {
                Ok(map) => Some(map),
                Err(HolonError::NotImplemented(_)) => None,
                Err(e) => {
                    return Err(format!("Failed to get actual all_related_holons: {:?}", e));
                }
            };

        // Some references (notably SmartReference on client side) cannot fetch
        // related holons yet. In that case, compare essential content only.
        if expected_relationship_map.is_none() || actual_relationship_map.is_none() {
            return Ok(());
        }

        let expected_relationship_map = expected_relationship_map
            .expect("checked above: expected_relationship_map is Some");
        let actual_relationship_map =
            actual_relationship_map.expect("checked above: actual_relationship_map is Some");

        if expected_relationship_map.count() != actual_relationship_map.count() {
            return Err(format!(
                "relationship count mismatch for expected {}:{} vs actual {}:{} (expected {}, actual {})",
                expected.reference_kind_string(),
                expected.reference_id_string(),
                actual.reference_kind_string(),
                actual.reference_id_string(),
                expected_relationship_map.count(),
                actual_relationship_map.count()
            ));
        }

        for (relationship_name, expected_collection_arc) in expected_relationship_map.iter() {
            let expected_collection = expected_collection_arc
                .read()
                .map_err(|e| format!("Failed to acquire read lock for expected collection: {}", e))?;
            let expected_members = expected_collection.get_members().clone();

            let actual_collection_arc = actual_relationship_map
                .get_collection_for_relationship(&relationship_name)
                .ok_or_else(|| {
                    format!(
                        "actual relationship map is missing relationship {:?}",
                        relationship_name
                    )
                })?;
            let actual_collection = actual_collection_arc
                .read()
                .map_err(|e| format!("Failed to acquire read lock for actual collection: {}", e))?;
            let mut unmatched_actual = actual_collection.get_members().clone();

            if expected_members.len() != unmatched_actual.len() {
                return Err(format!(
                    "relationship {:?} member count mismatch (expected {}, actual {})",
                    relationship_name,
                    expected_members.len(),
                    unmatched_actual.len()
                ));
            }

            for expected_member in expected_members {
                let mut matched_index: Option<usize> = None;
                let mut matched_visited_state: Option<HashSet<(String, String)>> = None;
                let mut last_error: Option<String> = None;

                for (index, actual_member) in unmatched_actual.iter().enumerate() {
                    let mut candidate_visited = visited_pairs.clone();
                    match Self::compare_holon_graph_eq(
                        &expected_member,
                        actual_member,
                        &mut candidate_visited,
                    ) {
                        Ok(()) => {
                            matched_index = Some(index);
                            matched_visited_state = Some(candidate_visited);
                            break;
                        }
                        Err(err) => {
                            last_error = Some(err);
                        }
                    }
                }

                match matched_index {
                    Some(index) => {
                        unmatched_actual.remove(index);
                        if let Some(new_visited) = matched_visited_state {
                            *visited_pairs = new_visited;
                        }
                    }
                    None => {
                        let expected_member_content = expected_member.essential_content().map_err(
                            |e| {
                                format!(
                                    "Failed to read expected member content for relationship {:?}: {:?}",
                                    relationship_name, e
                                )
                            },
                        )?;
                        return Err(format!(
                            "No matching member found for relationship {:?} and expected member content {:#?}. Last mismatch: {}",
                            relationship_name,
                            expected_member_content,
                            last_error.unwrap_or_else(|| "no candidate mismatch details available".to_string())
                        ));
                    }
                }
            }

            if !unmatched_actual.is_empty() {
                return Err(format!(
                    "relationship {:?} has unmatched actual members after exhaustive comparison: {:?}",
                    relationship_name, unmatched_actual
                ));
            }
        }

        Ok(())
    }

    fn pair_key(expected: &HolonReference, actual: &HolonReference) -> (String, String) {
        (
            format!(
                "{}:{}",
                expected.reference_kind_string(),
                expected.reference_id_string()
            ),
            format!(
                "{}:{}",
                actual.reference_kind_string(),
                actual.reference_id_string()
            ),
        )
    }
}
