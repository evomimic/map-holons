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

use crate::{
    ExecutionEquivalenceResolver, ExecutionHolons, ExpectedSnapshot, FixtureHeadIndex,
    TestReference, SAVED_LOOKUP_STUB_MARKER,
};
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_prelude::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
struct DefinitionalRelationshipPolicy {
    name: RelationshipName,
    is_ordered: bool,
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
    #[allow(deprecated)]
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
        Self::compare_expected_subset(&expected_root, &actual_root, &mut visited_pairs)
            .unwrap_or_else(|message| panic!("{}", message));
    }

    /// Assert saved DB content using harness-root policy plus shared member equivalence.
    ///
    /// Commit may persist non-definitional navigational edges, such as inverse
    /// SmartLinks, that were never present in fixture snapshots. Loader-owned
    /// schema descriptors may also appear in fixtures only as key-only
    /// `SavedLookup` stubs. The harness therefore keeps the root comparison
    /// aligned with fixture reality: compare root properties structurally,
    /// derive definitional relationship policy from the actual saved root, and
    /// delegate relationship member comparison to the shared reference-layer
    /// comparator through `ExecutionEquivalenceResolver`.
    pub fn assert_saved_content_eq(
        &self,
        execution_holons: &ExecutionHolons,
        fixture_head_index: &FixtureHeadIndex,
    ) {
        let expected_root = HolonReference::from(self.expected_snapshot.snapshot());
        let actual_root = self
            .execution_handle
            .get_holon_reference()
            .expect("Failed to get HolonReference for execution_handle");
        let resolver = ExecutionEquivalenceResolver::new(execution_holons, fixture_head_index);

        Self::compare_saved_content_root(&expected_root, &actual_root, &resolver).unwrap_or_else(
            |message| {
                panic!(
                    "saved content mismatch for expected {}:{} vs actual {}:{}\n{}",
                    expected_root.reference_kind_string(),
                    expected_root.reference_id_string(),
                    actual_root.reference_kind_string(),
                    actual_root.reference_id_string(),
                    message
                )
            },
        );
    }

    /// Saved-content root policy for fixture-authored expectations.
    ///
    /// This is intentionally not the full shared definitional-equivalence
    /// relation at the root. Fixture snapshots own authored root properties and
    /// declared root edges, but loader-saved schema/type descriptors are modeled
    /// as opaque key-only stubs. The actual saved descriptor is therefore the
    /// source of truth for which root relationships are definitional; member
    /// subtrees use the shared comparator.
    #[allow(deprecated)]
    fn compare_saved_content_root(
        expected: &HolonReference,
        actual: &HolonReference,
        resolver: &ExecutionEquivalenceResolver<'_>,
    ) -> Result<(), String> {
        let expected_content = expected.essential_content().map_err(|e| {
            format!(
                "failed to read expected root content {}:{}: {:?}",
                expected.reference_kind_string(),
                expected.reference_id_string(),
                e
            )
        })?;
        let actual_content = actual.essential_content().map_err(|e| {
            format!(
                "failed to read actual root content {}:{}: {:?}",
                actual.reference_kind_string(),
                actual.reference_id_string(),
                e
            )
        })?;

        if expected_content.property_map != actual_content.property_map {
            return Err(format!(
                "path: <root>\nreason: property map mismatch\nexpected: {:#?}\nactual: {:#?}",
                expected_content.property_map, actual_content.property_map
            ));
        }

        let relationship_policies = Self::actual_definitional_relationship_policies(actual)?;
        let expected_relationship_map = expected
            .all_related_holons()
            .map_err(|e| format!("failed to get expected root all_related_holons: {:?}", e))?;
        let actual_relationship_map = actual
            .all_related_holons()
            .map_err(|e| format!("failed to get actual root all_related_holons: {:?}", e))?;

        for relationship_policy in relationship_policies {
            Self::compare_saved_root_relationship(
                &expected_relationship_map,
                &actual_relationship_map,
                &relationship_policy,
                resolver,
            )?;
        }

        Ok(())
    }

    /// Harness-local subset assertion for per-step expected-content checks.
    ///
    /// Definitional equivalence lives in `holons_core`; this helper intentionally
    /// keeps the older expected-subset/all-relationships semantics used by
    /// `assert_essential_content_eq`.
    #[allow(deprecated)]
    fn compare_expected_subset(
        expected: &HolonReference,
        actual: &HolonReference,
        visited_pairs: &mut HashSet<(String, String)>,
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

        let expected_relationships =
            Self::relationship_entries_with_members(&expected_relationship_map)?;
        let actual_relationships =
            Self::relationship_entries_with_members(&actual_relationship_map)?;

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
                let (matched_index, matched_visited_state, last_error) =
                    Self::match_by_graph(&expected_member, &unmatched_actual, visited_pairs);

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
        }

        Ok(())
    }

    fn actual_definitional_relationship_policies(
        actual: &HolonReference,
    ) -> Result<Vec<DefinitionalRelationshipPolicy>, String> {
        let descriptor = match actual.holon_descriptor() {
            Ok(descriptor) => descriptor,
            Err(HolonError::MissingDescribedBy { .. }) => {
                let relationship_map = actual.all_related_holons().map_err(|e| {
                    format!(
                        "failed to get actual root relationships for undescribed holon check: {:?}",
                        e
                    )
                })?;
                if Self::relationship_entries_with_members(&relationship_map)?.is_empty() {
                    return Ok(Vec::new());
                }
                return Err(format!(
                    "path: <root>\nreason: cannot derive definitional relationships for undescribed actual holon {}:{} with relationship content",
                    actual.reference_kind_string(),
                    actual.reference_id_string()
                ));
            }
            Err(e) => {
                return Err(format!(
                    "path: <root>\nreason: failed to resolve actual root descriptor: {:?}",
                    e
                ));
            }
        };

        let mut policies = HashMap::new();
        for relationship_descriptor in descriptor.instance_relationships().map_err(|e| {
            format!(
                "path: <root>\nreason: failed to resolve actual root instance relationships: {:?}",
                e
            )
        })? {
            if relationship_descriptor.is_definitional().map_err(|e| {
                format!("path: <root>\nreason: failed to read IsDefinitional: {:?}", e)
            })? {
                let name = relationship_descriptor.base_relationship_name().map_err(|e| {
                    format!(
                        "path: <root>\nreason: failed to read definitional relationship name: {:?}",
                        e
                    )
                })?;
                let is_ordered = relationship_descriptor.is_ordered().map_err(|e| {
                    format!(
                        "path: <root>\nreason: failed to read IsOrdered for {:?}: {:?}",
                        name, e
                    )
                })?;
                policies.entry(name).or_insert(is_ordered);
            }
        }

        let mut policies: Vec<_> = policies
            .into_iter()
            .map(|(name, is_ordered)| DefinitionalRelationshipPolicy { name, is_ordered })
            .collect();
        policies.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(policies)
    }

    fn compare_saved_root_relationship(
        expected_relationship_map: &RelationshipMap,
        actual_relationship_map: &RelationshipMap,
        relationship_policy: &DefinitionalRelationshipPolicy,
        resolver: &dyn EquivalenceResolver,
    ) -> Result<(), String> {
        let expected_members =
            Self::relationship_members(expected_relationship_map, &relationship_policy.name)?;
        let actual_members =
            Self::relationship_members(actual_relationship_map, &relationship_policy.name)?;

        if expected_members.is_empty() && actual_members.is_empty() {
            return Ok(());
        }

        if relationship_policy.is_ordered {
            return Self::compare_ordered_saved_root_members(
                &relationship_policy.name,
                &expected_members,
                &actual_members,
                resolver,
            );
        }

        Self::compare_unordered_saved_root_members(
            &relationship_policy.name,
            &expected_members,
            &actual_members,
            resolver,
        )
    }

    fn compare_ordered_saved_root_members(
        relationship_name: &RelationshipName,
        expected_members: &[HolonReference],
        actual_members: &[HolonReference],
        resolver: &dyn EquivalenceResolver,
    ) -> Result<(), String> {
        if expected_members.len() != actual_members.len() {
            return Err(format!(
                "path: {:?}\nreason: ordered member count mismatch: expected {}, actual {}",
                relationship_name,
                expected_members.len(),
                actual_members.len()
            ));
        }

        for (index, (expected_member, actual_member)) in
            expected_members.iter().zip(actual_members.iter()).enumerate()
        {
            Self::compare_saved_root_member_pair(
                relationship_name,
                expected_member,
                actual_member,
                resolver,
            )
            .map_err(|message| format!("{} at index {}", message, index))?;
        }

        Ok(())
    }

    fn compare_unordered_saved_root_members(
        relationship_name: &RelationshipName,
        expected_members: &[HolonReference],
        actual_members: &[HolonReference],
        resolver: &dyn EquivalenceResolver,
    ) -> Result<(), String> {
        let mut unmatched_actual = actual_members.to_vec();
        let mut last_error = None;

        for expected_member in expected_members {
            let mut matched_index = None;
            for (index, actual_member) in unmatched_actual.iter().enumerate() {
                match Self::compare_saved_root_member_pair(
                    relationship_name,
                    expected_member,
                    actual_member,
                    resolver,
                ) {
                    Ok(()) => {
                        matched_index = Some(index);
                        break;
                    }
                    Err(err) => last_error = Some(err),
                }
            }

            match matched_index {
                Some(index) => {
                    unmatched_actual.remove(index);
                }
                None => {
                    return Err(format!(
                        "path: {:?}\nreason: no matching member found for expected member {}:{}. Last mismatch: {}",
                        relationship_name,
                        expected_member.reference_kind_string(),
                        expected_member.reference_id_string(),
                        last_error.unwrap_or_else(|| "no candidate mismatch details available".to_string())
                    ));
                }
            }
        }

        if !unmatched_actual.is_empty() {
            return Err(format!(
                "path: {:?}\nreason: actual root has {} extra definitional member(s)",
                relationship_name,
                unmatched_actual.len()
            ));
        }

        Ok(())
    }

    fn compare_saved_root_member_pair(
        relationship_name: &RelationshipName,
        expected_member: &HolonReference,
        actual_member: &HolonReference,
        resolver: &dyn EquivalenceResolver,
    ) -> Result<(), String> {
        match expected_member.definitional_equivalence_with_resolver(actual_member, resolver) {
            Ok(EquivalenceOutcome::Equivalent) => Ok(()),
            Ok(EquivalenceOutcome::Divergent(mut divergence)) => {
                divergence.path.insert(0, relationship_name.clone());
                Err(format!(
                    "path: {}\nreason: {}",
                    Self::format_divergence_path(&divergence.path),
                    divergence.reason
                ))
            }
            Err(e) => Err(format!(
                "path: {:?}\nreason: failed to compare relationship member: {:?}",
                relationship_name, e
            )),
        }
    }

    fn relationship_members(
        relationship_map: &RelationshipMap,
        relationship_name: &RelationshipName,
    ) -> Result<Vec<HolonReference>, String> {
        let Some(collection_arc) = relationship_map
            .iter()
            .into_iter()
            .find(|(name, _)| name == relationship_name)
            .map(|(_, collection_arc)| collection_arc.clone())
        else {
            return Ok(Vec::new());
        };

        let collection = collection_arc.read().map_err(|e| {
            format!("Failed to acquire read lock for relationship {:?}: {}", relationship_name, e)
        })?;

        Ok(collection.get_members().clone())
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
    ) -> (Option<usize>, Option<HashSet<(String, String)>>, Option<String>) {
        let mut matched_index: Option<usize> = None;
        let mut matched_visited_state: Option<HashSet<(String, String)>> = None;
        let mut last_error: Option<String> = None;

        for (index, actual_member) in unmatched_actual.iter().enumerate() {
            let mut candidate_visited = visited_pairs.clone();
            match Self::compare_expected_subset(
                expected_member,
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

        (matched_index, matched_visited_state, last_error)
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

    fn pair_key(expected: &HolonReference, actual: &HolonReference) -> (String, String) {
        (
            format!("{}:{}", expected.reference_kind_string(), expected.reference_id_string()),
            format!("{}:{}", actual.reference_kind_string(), actual.reference_id_string()),
        )
    }

    fn format_divergence_path(path: &[RelationshipName]) -> String {
        if path.is_empty() {
            "<root>".to_string()
        } else {
            path.iter().map(|segment| format!("{:?}", segment)).collect::<Vec<_>>().join(" -> ")
        }
    }
}
