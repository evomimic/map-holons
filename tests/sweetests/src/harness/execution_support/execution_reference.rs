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

use crate::{ExpectedSnapshot, TestReference, SAVED_LOOKUP_STUB_MARKER};
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_prelude::prelude::*;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelationshipComparisonPolicy {
    ExpectedSubsetAllRelationships,
    ExactDefinitionalRelationships,
}

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
    /// - an expected relationship member has no matching actual member
    ///
    /// Relationship comparison uses **superset semantics** (expected ⊆ actual):
    /// every fixture-expected edge must exist in the saved graph, but extra
    /// actual edges are tolerated. Commit Pass 2 materializes an inverse
    /// SmartLink on the *target* of every committed declared relationship
    /// (issue #442), so saved holons legitimately carry edges the fixture
    /// never staged. Inverse correctness is asserted by dedicated
    /// traversal-verification steps, not by this comparison.
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
        Self::compare_holon_graph_eq(
            &expected_root,
            &actual_root,
            &mut visited_pairs,
            RelationshipComparisonPolicy::ExpectedSubsetAllRelationships,
        )
        .unwrap_or_else(|message| panic!("{}", message));
    }

    /// Assert saved DB content using essential content plus definitional relationships.
    ///
    /// Commit may persist non-definitional navigational edges, such as inverse
    /// SmartLinks, that were never present in fixture snapshots. Saved content
    /// comparison therefore derives the actual holon's definitional
    /// relationship surface from its descriptor, compares that filtered graph
    /// exactly, and matches relationship members by key/identity. Each saved
    /// fixture holon is still compared as its own root, so member content does
    /// not need to be recursively revalidated from every relationship edge.
    pub fn assert_saved_content_eq(&self) {
        let expected_root = HolonReference::from(self.expected_snapshot.snapshot());
        let actual_root = self
            .execution_handle
            .get_holon_reference()
            .expect("Failed to get HolonReference for execution_handle");
        let mut visited_pairs: HashSet<(String, String)> = HashSet::new();
        Self::compare_holon_graph_eq(
            &expected_root,
            &actual_root,
            &mut visited_pairs,
            RelationshipComparisonPolicy::ExactDefinitionalRelationships,
        )
        .unwrap_or_else(|message| panic!("{}", message));
    }

    fn compare_holon_graph_eq(
        expected: &HolonReference,
        actual: &HolonReference,
        visited_pairs: &mut HashSet<(String, String)>,
        policy: RelationshipComparisonPolicy,
    ) -> Result<(), String> {
        let pair_key = Self::pair_key(expected, actual);
        if !visited_pairs.insert(pair_key) {
            return Ok(());
        }

        // Key-only stubs stand in for holons saved outside the fixture's ledger
        // (e.g. schema-loaded descriptors); their full content and graph cannot
        // be reproduced at fixture time, so match by key and stop recursing.
        if Self::is_saved_lookup_stub(expected)? {
            return Self::compare_keys_only(expected, actual);
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

        let expected_relationship_map = match expected.all_related_holons() {
            Ok(map) => Some(map),
            Err(HolonError::NotImplemented(_)) => None,
            Err(e) => {
                return Err(format!("Failed to get expected all_related_holons: {:?}", e));
            }
        };
        let actual_relationship_map = match actual.all_related_holons() {
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

        let expected_relationship_map =
            expected_relationship_map.expect("checked above: expected_relationship_map is Some");
        let actual_relationship_map =
            actual_relationship_map.expect("checked above: actual_relationship_map is Some");

        let (expected_relationships, actual_relationships) = match policy {
            RelationshipComparisonPolicy::ExpectedSubsetAllRelationships => (
                Self::relationship_entries_with_members(&expected_relationship_map)?,
                Self::relationship_entries_with_members(&actual_relationship_map)?,
            ),
            RelationshipComparisonPolicy::ExactDefinitionalRelationships => {
                let definitional_relationship_names = Self::definitional_relationship_names(
                    actual,
                    &expected_relationship_map,
                    &actual_relationship_map,
                )?;
                (
                    Self::relationship_entries_for_names(
                        &expected_relationship_map,
                        &definitional_relationship_names,
                    )?,
                    Self::relationship_entries_for_names(
                        &actual_relationship_map,
                        &definitional_relationship_names,
                    )?,
                )
            }
        };

        if policy == RelationshipComparisonPolicy::ExactDefinitionalRelationships {
            let expected_relationship_names: HashSet<RelationshipName> = expected_relationships
                .iter()
                .map(|(relationship_name, _)| relationship_name.clone())
                .collect();
            for (actual_relationship_name, _) in actual_relationships.iter() {
                if !expected_relationship_names.contains(actual_relationship_name) {
                    return Err(format!(
                        "expected relationship map is missing definitional relationship {:?}",
                        actual_relationship_name
                    ));
                }
            }
        }

        for (relationship_name, expected_collection_arc) in expected_relationships {
            let expected_collection = expected_collection_arc.read().map_err(|e| {
                format!("Failed to acquire read lock for expected collection: {}", e)
            })?;
            let expected_members = expected_collection.get_members().clone();

            let actual_collection_arc = actual_relationships
                .iter()
                .find(|(actual_name, _)| actual_name == &relationship_name)
                .map(|(_, collection_arc)| collection_arc.clone())
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

            for expected_member in expected_members {
                let (matched_index, matched_visited_state, last_error) = match policy {
                    RelationshipComparisonPolicy::ExpectedSubsetAllRelationships => {
                        Self::match_by_graph(
                            &expected_member,
                            &unmatched_actual,
                            visited_pairs,
                            policy,
                        )
                    }
                    RelationshipComparisonPolicy::ExactDefinitionalRelationships => {
                        Self::match_by_identity(&expected_member, &unmatched_actual)
                    }
                };

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

            if policy == RelationshipComparisonPolicy::ExactDefinitionalRelationships
                && !unmatched_actual.is_empty()
            {
                return Err(format!(
                    "actual definitional relationship {:?} has {} extra member(s)",
                    relationship_name,
                    unmatched_actual.len()
                ));
            }
        }

        Ok(())
    }

    /// Returns whether the expected holon carries the saved-lookup stub marker.
    fn is_saved_lookup_stub(expected: &HolonReference) -> Result<bool, String> {
        match expected.property_value(SAVED_LOOKUP_STUB_MARKER) {
            Ok(value) => Ok(value.is_some()),
            Err(e) => Err(format!(
                "failed to read saved-lookup stub marker on expected {}:{}: {:?}",
                expected.reference_kind_string(),
                expected.reference_id_string(),
                e
            )),
        }
    }

    /// Key-only comparison used for saved-lookup stubs.
    fn compare_keys_only(expected: &HolonReference, actual: &HolonReference) -> Result<(), String> {
        let expected_key = expected
            .key()
            .map_err(|e| format!("failed to read expected stub key: {:?}", e))?
            .ok_or_else(|| "saved-lookup stub has no key to compare".to_string())?;
        let actual_key = actual
            .key()
            .map_err(|e| format!("failed to read actual holon key: {:?}", e))?
            .ok_or_else(|| {
                format!(
                    "actual holon {}:{} has no key but was matched against saved-lookup stub key {:?}",
                    actual.reference_kind_string(),
                    actual.reference_id_string(),
                    expected_key
                )
            })?;

        if expected_key == actual_key {
            Ok(())
        } else {
            Err(format!(
                "saved-lookup stub key mismatch: expected {:?}, actual {:?}",
                expected_key, actual_key
            ))
        }
    }

    fn match_by_graph(
        expected_member: &HolonReference,
        unmatched_actual: &[HolonReference],
        visited_pairs: &HashSet<(String, String)>,
        policy: RelationshipComparisonPolicy,
    ) -> (Option<usize>, Option<HashSet<(String, String)>>, Option<String>) {
        let mut matched_index: Option<usize> = None;
        let mut matched_visited_state: Option<HashSet<(String, String)>> = None;
        let mut last_error: Option<String> = None;

        for (index, actual_member) in unmatched_actual.iter().enumerate() {
            let mut candidate_visited = visited_pairs.clone();
            match Self::compare_holon_graph_eq(
                expected_member,
                actual_member,
                &mut candidate_visited,
                policy,
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

        (matched_index, matched_visited_state, last_error)
    }

    fn match_by_identity(
        expected_member: &HolonReference,
        unmatched_actual: &[HolonReference],
    ) -> (Option<usize>, Option<HashSet<(String, String)>>, Option<String>) {
        let mut last_error = None;

        for (index, actual_member) in unmatched_actual.iter().enumerate() {
            match Self::compare_identity(expected_member, actual_member) {
                Ok(()) => return (Some(index), None, None),
                Err(err) => last_error = Some(err),
            }
        }

        (None, None, last_error)
    }

    fn compare_identity(expected: &HolonReference, actual: &HolonReference) -> Result<(), String> {
        let expected_key =
            expected.key().map_err(|e| format!("failed to read expected member key: {:?}", e))?;
        let actual_key =
            actual.key().map_err(|e| format!("failed to read actual member key: {:?}", e))?;

        match (expected_key, actual_key) {
            (Some(expected_key), Some(actual_key)) if expected_key == actual_key => Ok(()),
            (Some(expected_key), Some(actual_key)) => Err(format!(
                "member key mismatch: expected {:?}, actual {:?}",
                expected_key, actual_key
            )),
            (None, _) => Err(format!(
                "expected relationship member {}:{} has no key for identity comparison",
                expected.reference_kind_string(),
                expected.reference_id_string()
            )),
            (_, None) => Err(format!(
                "actual relationship member {}:{} has no key for identity comparison",
                actual.reference_kind_string(),
                actual.reference_id_string()
            )),
        }
    }

    /// Returns only relationship entries whose collections currently contain
    /// members.
    ///
    /// This treats an empty relationship collection as equivalent to the
    /// relationship being absent, which keeps saved-content assertions aligned
    /// with current persistence behavior.
    fn relationship_entries_with_members(
        relationship_map: &RelationshipMap,
    ) -> Result<Vec<(RelationshipName, Arc<RwLock<HolonCollection>>)>, String> {
        let mut entries = Vec::new();

        for (relationship_name, collection_arc) in relationship_map.iter() {
            let count = collection_arc
                .read()
                .map_err(|e| {
                    format!(
                        "Failed to acquire read lock while normalizing relationship {:?}: {}",
                        relationship_name, e
                    )
                })?
                .get_members()
                .len();

            if count > 0 {
                entries.push((relationship_name, collection_arc));
            }
        }

        Ok(entries)
    }

    fn relationship_entries_for_names(
        relationship_map: &RelationshipMap,
        relationship_names: &HashSet<RelationshipName>,
    ) -> Result<Vec<(RelationshipName, Arc<RwLock<HolonCollection>>)>, String> {
        Ok(Self::relationship_entries_with_members(relationship_map)?
            .into_iter()
            .filter(|(relationship_name, _)| relationship_names.contains(relationship_name))
            .collect())
    }

    fn definitional_relationship_names(
        actual: &HolonReference,
        expected_relationship_map: &RelationshipMap,
        actual_relationship_map: &RelationshipMap,
    ) -> Result<HashSet<RelationshipName>, String> {
        let descriptor = match actual.holon_descriptor() {
            Ok(descriptor) => descriptor,
            Err(HolonError::MissingDescribedBy { .. }) => {
                let expected_relationships =
                    Self::relationship_entries_with_members(expected_relationship_map)?;
                let actual_relationships =
                    Self::relationship_entries_with_members(actual_relationship_map)?;
                if expected_relationships.is_empty() && actual_relationships.is_empty() {
                    return Ok(HashSet::new());
                }
                return Err(format!(
                    "cannot derive definitional relationships for undescribed actual holon {}:{} with relationship content",
                    actual.reference_kind_string(),
                    actual.reference_id_string()
                ));
            }
            Err(e) => {
                return Err(format!(
                    "failed to resolve descriptor for actual holon {}:{} while deriving definitional relationships: {:?}",
                    actual.reference_kind_string(),
                    actual.reference_id_string(),
                    e
                ));
            }
        };

        let mut relationship_names = HashSet::new();
        for relationship_descriptor in descriptor.instance_relationships().map_err(|e| {
            format!(
                "failed to resolve effective instance relationships for actual holon {}:{}: {:?}",
                actual.reference_kind_string(),
                actual.reference_id_string(),
                e
            )
        })? {
            if relationship_descriptor.is_definitional().map_err(|e| {
                format!(
                    "failed to read IsDefinitional while deriving saved-content relationship set for actual holon {}:{}: {:?}",
                    actual.reference_kind_string(),
                    actual.reference_id_string(),
                    e
                )
            })? {
                relationship_names.insert(relationship_descriptor.base_relationship_name().map_err(
                    |e| {
                        format!(
                            "failed to read definitional relationship name for actual holon {}:{}: {:?}",
                            actual.reference_kind_string(),
                            actual.reference_id_string(),
                            e
                        )
                    },
                )?);
            }
        }

        Ok(relationship_names)
    }

    fn pair_key(expected: &HolonReference, actual: &HolonReference) -> (String, String) {
        (
            format!("{}:{}", expected.reference_kind_string(), expected.reference_id_string()),
            format!("{}:{}", actual.reference_kind_string(), actual.reference_id_string()),
        )
    }
}
