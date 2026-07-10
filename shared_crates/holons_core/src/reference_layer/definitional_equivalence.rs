//! Definitional-equivalence comparison for [`HolonReference`].
//!
//! This module implements the definitional-equivalence relation inside the reference layer so
//! callers can ask whether two holons are definitionally equivalent without
//! extracting raw property maps or reimplementing graph-walk policy outside
//! `holons_core`.
//!
//! The recursive algorithm is:
//!
//! 1. Resolve each node through the caller-supplied [`EquivalenceResolver`].
//!    `Canonical` substitutes a reference for the rest of the comparison at that
//!    node. `MatchByKey` is an opaque placeholder path used by the test harness:
//!    it compares only keys and does not recurse.
//! 2. If both resolved references are saved, compare only their
//!    [`core_types::HolonId`] values. Saved identity is a conclusive anchor, so
//!    no structural fallback is attempted and no content is read.
//! 3. Apply coinductive cycle termination via a visited-pair set keyed by typed
//!    reference identity.
//! 4. Compare raw property maps through a crate-private accessor. This
//!    intentionally excludes lifecycle-dependent state such as staged errors;
//!    key semantics are already part of the property surface.
//! 5. Derive each side's definitional relationship-name set from that side's
//!    own descriptor. Undescribed holons with no relationship content are
//!    treated as having an empty definitional surface; undescribed holons with
//!    relationship content are schema-integrity errors.
//! 6. For each shared definitional relationship, compare members recursively.
//!    Ordered relationships are matched positionally. Unordered relationships
//!    use a greedy shrinking-multiset strategy with a per-candidate visited-set
//!    snapshot. This is intentionally incomplete for ambiguous unsaved
//!    duplicates, but keeps the implementation WASM-safe and bounded.
//! 7. Return [`EquivalenceOutcome::Equivalent`] on success, or
//!    [`EquivalenceOutcome::Divergent`] with a breadcrumb path and human-readable
//!    reason at the first comparison failure.
//!
//! Breadcrumb segments are prepended while the recursion unwinds, so successful
//! comparisons allocate no path segments.
//!
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, RwLock};

use crate::core_shared_objects::{transactions::TxId, HolonCollection, RelationshipMap};
use crate::reference_layer::{readable_impl::ReadableHolonImpl, HolonReference, ReadableHolon};
use core_types::{HolonError, HolonId, PropertyMap, RelationshipName, TemporaryId};

/// Outcome of definitional-equivalence comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquivalenceOutcome {
    Equivalent,
    Divergent(Divergence),
}

/// Breadcrumb from the comparison root to the divergence site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub path: Vec<RelationshipName>,
    pub reason: String,
}

/// Resolver-directed substitution policy for a comparison node.
#[derive(Debug, Clone)]
pub enum NodeResolution {
    /// Substitute this reference, then continue normal comparison.
    Canonical(HolonReference),
    /// Compare keys only and do not recurse into this node.
    MatchByKey,
    /// Compare the original reference as-is.
    AsIs,
}

/// Reference translation seam for callers that need fixture or runtime rebinding.
pub trait EquivalenceResolver {
    fn resolve(&self, reference: &HolonReference) -> Result<NodeResolution, HolonError>;
}

/// Default resolver: every reference compares as itself.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpResolver;

impl EquivalenceResolver for NoOpResolver {
    fn resolve(&self, _reference: &HolonReference) -> Result<NodeResolution, HolonError> {
        Ok(NodeResolution::AsIs)
    }
}

pub(crate) fn definitional_equivalence<T>(
    source: &T,
    other: &HolonReference,
    resolver: &dyn EquivalenceResolver,
) -> Result<EquivalenceOutcome, HolonError>
where
    T: ReadableHolonImpl + ?Sized,
{
    let source_reference = source.holon_reference_impl();
    compare_references(&source_reference, other, resolver, &mut HashSet::new())
}

/// Compares two references under the definitional-equivalence rule.
///
/// The function is structured cheapest-first: resolver rewrite, saved identity,
/// cycle check, property surface, then descriptor-derived definitional
/// relationships. Divergence paths are assembled on unwind by the caller frame
/// that knows which relationship edge was being traversed.
fn compare_references(
    left: &HolonReference,
    right: &HolonReference,
    resolver: &dyn EquivalenceResolver,
    visited: &mut HashSet<VisitedPairKey>,
) -> Result<EquivalenceOutcome, HolonError> {
    // Step 1: Canonicalize each node through the resolver. This is a single
    // substitution step per side; the comparator does not loop until fixed
    // point.
    let left_resolution = resolver.resolve(left)?;
    let right_resolution = resolver.resolve(right)?;

    let left_reference = resolved_reference(left, &left_resolution);
    let right_reference = resolved_reference(right, &right_resolution);

    // Resolver placeholders compare only keys and intentionally do not recurse
    // into structure.
    if matches!(left_resolution, NodeResolution::MatchByKey)
        || matches!(right_resolution, NodeResolution::MatchByKey)
    {
        return compare_by_key_only(left_reference, right_reference);
    }

    // Step 2: Saved-vs-saved identity is conclusive. If both sides are saved,
    // the algorithm stops here and never reads content.
    if left_reference.is_saved() && right_reference.is_saved() {
        return compare_saved_identity(left_reference, right_reference);
    }

    // Step 3: Coinductive cycle handling. A pair already under comparison is
    // presumed equivalent so recursion can terminate on cyclic graphs.
    let pair_key = VisitedPairKey::new(left_reference, right_reference)?;
    if visited.contains(&pair_key) {
        return Ok(EquivalenceOutcome::Equivalent);
    }
    visited.insert(pair_key);

    // Step 4: Compare the raw property surface internally. This is the only
    // raw-map touch in the algorithm, and it excludes lifecycle state for the
    // reasons described in the module header.
    let left_property_map = left_reference.property_map_impl()?;
    let right_property_map = right_reference.property_map_impl()?;
    if let Some(divergence) = compare_property_maps(&left_property_map, &right_property_map) {
        return Ok(EquivalenceOutcome::Divergent(divergence));
    }

    // Steps 5-6: Derive each side's definitional relationship surface from its
    // own descriptor, then require the two name sets to match exactly before
    // descending into members.
    let left_relationships = definitional_relationships(left_reference)?;
    let right_relationships = definitional_relationships(right_reference)?;
    if let Some(divergence) =
        compare_relationship_name_sets(&left_relationships, &right_relationships)
    {
        return Ok(EquivalenceOutcome::Divergent(divergence));
    }

    for relationship_name in left_relationships.keys() {
        let left_descriptor = left_relationships
            .get(relationship_name)
            .expect("left relationship name should exist in left relationship map");
        let right_descriptor = right_relationships
            .get(relationship_name)
            .expect("shared relationship name should exist in right relationship map");

        let left_members =
            collection_members(left_reference.related_holons(relationship_name.clone())?)?;
        let right_members =
            collection_members(right_reference.related_holons(relationship_name.clone())?)?;

        // Step 7: Recurse through relationship members. Ordered relationships
        // compare positionally; unordered relationships use greedy multiset
        // matching. If a child divergence is found, prepend the current
        // relationship segment while unwinding.
        let ordered = left_descriptor.is_ordered || right_descriptor.is_ordered;
        let outcome = if ordered {
            compare_ordered_members(
                relationship_name,
                &left_members,
                &right_members,
                resolver,
                visited,
            )?
        } else {
            compare_unordered_members(
                relationship_name,
                &left_members,
                &right_members,
                resolver,
                visited,
            )?
        };

        if let EquivalenceOutcome::Divergent(divergence) = outcome {
            return Ok(EquivalenceOutcome::Divergent(prepend_relationship_segment(
                relationship_name,
                divergence,
            )));
        }
    }

    Ok(EquivalenceOutcome::Equivalent)
}

fn resolved_reference<'a>(
    original: &'a HolonReference,
    resolution: &'a NodeResolution,
) -> &'a HolonReference {
    match resolution {
        NodeResolution::Canonical(reference) => reference,
        NodeResolution::MatchByKey | NodeResolution::AsIs => original,
    }
}

fn compare_by_key_only(
    left: &HolonReference,
    right: &HolonReference,
) -> Result<EquivalenceOutcome, HolonError> {
    let left_key = left.key()?;
    let right_key = right.key()?;

    if left_key == right_key {
        Ok(EquivalenceOutcome::Equivalent)
    } else {
        Ok(EquivalenceOutcome::Divergent(Divergence {
            path: Vec::new(),
            reason: format!(
                "Keys differ under MatchByKey resolution: left={:?}, right={:?}",
                left_key, right_key
            ),
        }))
    }
}

fn compare_saved_identity(
    left: &HolonReference,
    right: &HolonReference,
) -> Result<EquivalenceOutcome, HolonError> {
    let left_id = left.holon_id()?;
    let right_id = right.holon_id()?;

    if left_id == right_id {
        Ok(EquivalenceOutcome::Equivalent)
    } else {
        Ok(EquivalenceOutcome::Divergent(Divergence {
            path: Vec::new(),
            reason: format!("Saved holon ids differ: left={}, right={}", left_id, right_id),
        }))
    }
}

fn compare_property_maps(left: &PropertyMap, right: &PropertyMap) -> Option<Divergence> {
    for (property_name, left_value) in left {
        match right.get(property_name) {
            Some(right_value) if right_value == left_value => {}
            Some(right_value) => {
                return Some(Divergence {
                    path: Vec::new(),
                    reason: format!(
                        "Property {} differs: left={:?}, right={:?}",
                        property_name, left_value, right_value
                    ),
                })
            }
            None => {
                return Some(Divergence {
                    path: Vec::new(),
                    reason: format!("Property {} is missing on the right side", property_name),
                })
            }
        }
    }

    for property_name in right.keys() {
        if !left.contains_key(property_name) {
            return Some(Divergence {
                path: Vec::new(),
                reason: format!("Property {} is missing on the left side", property_name),
            });
        }
    }

    None
}

fn definitional_relationships(
    reference: &HolonReference,
) -> Result<BTreeMap<RelationshipName, RelationshipComparisonDescriptor>, HolonError> {
    match reference.holon_descriptor() {
        Ok(descriptor) => {
            let mut relationships = BTreeMap::new();
            for descriptor in descriptor.instance_relationships()? {
                if descriptor.is_definitional()? {
                    let name = descriptor.base_relationship_name()?;
                    relationships.insert(
                        name,
                        RelationshipComparisonDescriptor { is_ordered: descriptor.is_ordered()? },
                    );
                }
            }
            Ok(relationships)
        }
        Err(HolonError::MissingDescribedBy { .. }) => {
            let all_related = reference.all_related_holons()?;
            if relationship_map_is_empty(&all_related)? {
                Ok(BTreeMap::new())
            } else {
                Err(HolonError::MissingDescribedBy { holon: reference.summarize()? })
            }
        }
        Err(error) => Err(error),
    }
}

/// Returns `true` only when no relationship in the map holds any members.
///
/// A relationship name can linger in the map with an empty collection after all
/// of its members are removed, so map size alone does not indicate emptiness;
/// each collection's membership must be inspected. This keeps a relationless
/// undescribed holon's definitional surface empty (per the equivalence rule)
/// instead of forcing a spurious `MissingDescribedBy`.
fn relationship_map_is_empty(relationship_map: &RelationshipMap) -> Result<bool, HolonError> {
    for (_relationship_name, collection) in relationship_map.iter() {
        let has_members = !collection
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon collection: {}",
                    e
                ))
            })?
            .get_members()
            .is_empty();
        if has_members {
            return Ok(false);
        }
    }
    Ok(true)
}

fn compare_relationship_name_sets(
    left: &BTreeMap<RelationshipName, RelationshipComparisonDescriptor>,
    right: &BTreeMap<RelationshipName, RelationshipComparisonDescriptor>,
) -> Option<Divergence> {
    let left_names = left.keys().cloned().collect::<BTreeSet<_>>();
    let right_names = right.keys().cloned().collect::<BTreeSet<_>>();

    if left_names == right_names {
        return None;
    }

    let left_only =
        left_names.difference(&right_names).map(ToString::to_string).collect::<Vec<_>>();
    let right_only =
        right_names.difference(&left_names).map(ToString::to_string).collect::<Vec<_>>();

    Some(Divergence {
        path: Vec::new(),
        reason: format!(
            "Definitional relationships differ: left-only=[{}], right-only=[{}]",
            left_only.join(", "),
            right_only.join(", ")
        ),
    })
}

fn compare_ordered_members(
    relationship_name: &RelationshipName,
    left_members: &[HolonReference],
    right_members: &[HolonReference],
    resolver: &dyn EquivalenceResolver,
    visited: &mut HashSet<VisitedPairKey>,
) -> Result<EquivalenceOutcome, HolonError> {
    if left_members.len() != right_members.len() {
        return Ok(diverge_here(format!(
            "Ordered relationship {} has different lengths: left={}, right={}",
            relationship_name,
            left_members.len(),
            right_members.len()
        )));
    }

    for (left_member, right_member) in left_members.iter().zip(right_members.iter()) {
        let outcome = compare_references(left_member, right_member, resolver, visited)?;
        if !matches!(outcome, EquivalenceOutcome::Equivalent) {
            return Ok(outcome);
        }
    }

    Ok(EquivalenceOutcome::Equivalent)
}

/// Greedy unordered matching may return false negatives for ambiguous duplicate unsaved members.
fn compare_unordered_members(
    relationship_name: &RelationshipName,
    left_members: &[HolonReference],
    right_members: &[HolonReference],
    resolver: &dyn EquivalenceResolver,
    visited: &mut HashSet<VisitedPairKey>,
) -> Result<EquivalenceOutcome, HolonError> {
    if left_members.len() != right_members.len() {
        return Ok(diverge_here(format!(
            "Unordered relationship {} has different lengths: left={}, right={}",
            relationship_name,
            left_members.len(),
            right_members.len()
        )));
    }

    let mut remaining_right = right_members.to_vec();

    for left_member in left_members {
        let mut matched_index = None;
        let mut matched_visited = None;

        for (candidate_index, right_member) in remaining_right.iter().enumerate() {
            let mut trial_visited = visited.clone();
            match compare_references(left_member, right_member, resolver, &mut trial_visited)? {
                EquivalenceOutcome::Equivalent => {
                    matched_index = Some(candidate_index);
                    matched_visited = Some(trial_visited);
                    break;
                }
                EquivalenceOutcome::Divergent(_) => {}
            }
        }

        match (matched_index, matched_visited) {
            (Some(index), Some(trial_visited)) => {
                *visited = trial_visited;
                remaining_right.remove(index);
            }
            _ => {
                return Ok(diverge_here(format!(
                    "Unordered relationship {} has no matching member for {}",
                    relationship_name,
                    left_member.summarize()?
                )))
            }
        }
    }

    if !remaining_right.is_empty() {
        return Ok(diverge_here(format!(
            "Unordered relationship {} has unmatched right-side members",
            relationship_name
        )));
    }

    Ok(EquivalenceOutcome::Equivalent)
}

fn collection_members(
    collection: Arc<RwLock<HolonCollection>>,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection = collection.read().map_err(|e| {
        HolonError::FailedToAcquireLock(format!(
            "Failed to acquire read lock on holon collection: {}",
            e
        ))
    })?;
    Ok(collection.get_members().clone())
}

fn prepend_relationship_segment(
    relationship_name: &RelationshipName,
    mut divergence: Divergence,
) -> Divergence {
    let mut path = Vec::with_capacity(divergence.path.len() + 1);
    path.push(relationship_name.clone());
    path.append(&mut divergence.path);
    Divergence { path, reason: divergence.reason }
}

fn diverge_here(reason: String) -> EquivalenceOutcome {
    EquivalenceOutcome::Divergent(Divergence { path: Vec::new(), reason })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ReferenceIdentity {
    Saved { tx_id: TxId, holon_id: HolonId },
    Staged { tx_id: TxId, temporary_id: TemporaryId },
    Transient { tx_id: TxId, temporary_id: TemporaryId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VisitedPairKey {
    left: ReferenceIdentity,
    right: ReferenceIdentity,
}

#[derive(Debug, Clone)]
struct RelationshipComparisonDescriptor {
    is_ordered: bool,
}

impl ReferenceIdentity {
    fn from_reference(reference: &HolonReference) -> Result<Self, HolonError> {
        match reference {
            HolonReference::Smart(_) => {
                Ok(Self::Saved { tx_id: reference.tx_id(), holon_id: reference.holon_id()? })
            }
            HolonReference::Staged(staged) => {
                Ok(Self::Staged { tx_id: staged.tx_id(), temporary_id: staged.temporary_id() })
            }
            HolonReference::Transient(transient) => Ok(Self::Transient {
                tx_id: transient.tx_id(),
                temporary_id: transient.temporary_id(),
            }),
        }
    }
}

impl VisitedPairKey {
    fn new(left: &HolonReference, right: &HolonReference) -> Result<Self, HolonError> {
        Ok(Self {
            left: ReferenceIdentity::from_reference(left)?,
            right: ReferenceIdentity::from_reference(right)?,
        })
    }
}

#[cfg(test)]
// These tests run against the in-memory `build_context()` over `TestHolonService`,
// which cannot perform real commits (host/guest crossings return `NotImplemented`).
// Consequences for coverage:
// - Genuine saved holons carrying content don't exist here, so saved-vs-unsaved
//   *structural* recursion (reading a saved side's property map and members while the
//   other side is transient/staged) is intentionally NOT exercised in this module; it
//   is covered by the Phase 5 sweettest regression.
// - Saved-identity tests use synthetic `SmartReference` ids, which is valid precisely
//   because the saved short-circuit never reads content.
// - All descriptors here are transient; no staged-phase holon is exercised.
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_holon_type_descriptor, new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::{SmartReference, TransientReference, WritableHolon};
    use base_types::{BaseValue, MapBoolean, MapString};
    use core_types::{LocalId, PropertyName};
    use std::sync::Arc;
    use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, ToPropertyName};

    type TestContext = Arc<crate::core_shared_objects::transactions::TransactionContext>;

    fn relationship_name(name: &str) -> RelationshipName {
        RelationshipName(MapString(name.to_string()))
    }

    fn property_name(name: &str) -> PropertyName {
        MapString(name.to_string()).to_property_name()
    }

    fn holon_id(byte: u8) -> HolonId {
        HolonId::Local(LocalId(vec![byte; 39]))
    }

    fn saved_reference(context: &TestContext, byte: u8) -> HolonReference {
        HolonReference::Smart(SmartReference::new_from_id(context.context_handle(), holon_id(byte)))
    }

    fn saved_reference_with_key(context: &TestContext, byte: u8, key: &str) -> HolonReference {
        HolonReference::smart_with_key(
            context.context_handle(),
            holon_id(byte),
            MapString(key.to_string()),
        )
    }

    fn described_holon(
        context: &TestContext,
        key: &str,
        descriptor: &TransientReference,
    ) -> Result<TransientReference, HolonError> {
        let mut holon = new_test_holon(context, key)?;
        holon.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(descriptor)],
        )?;
        Ok(holon)
    }

    fn add_relationship_descriptor(
        context: &TestContext,
        source_type: &mut TransientReference,
        target_type: &TransientReference,
        name: &str,
        is_definitional: bool,
        is_ordered: bool,
    ) -> Result<TransientReference, HolonError> {
        let mut relationship = new_relationship_descriptor_holon(
            context,
            &format!("{}-{}-relationship", name.to_lowercase(), source_type.temporary_id()),
            name,
            HolonReference::from(&*source_type),
            HolonReference::from(target_type),
        )?;
        relationship
            .with_property_value(CorePropertyTypeName::IsDefinitional, is_definitional)?
            .with_property_value(CorePropertyTypeName::IsOrdered, is_ordered)?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![HolonReference::from(&relationship)],
        )?;
        Ok(relationship)
    }

    fn simple_types(
        context: &TestContext,
        prefix: &str,
    ) -> Result<(TransientReference, TransientReference), HolonError> {
        Ok((
            new_holon_type_descriptor(context, &format!("{prefix}-source-type"), "SourceType")?,
            new_holon_type_descriptor(context, &format!("{prefix}-target-type"), "TargetType")?,
        ))
    }

    fn outcome(
        left: &TransientReference,
        right: &TransientReference,
    ) -> Result<EquivalenceOutcome, HolonError> {
        left.definitional_equivalence(&HolonReference::from(right))
    }

    fn assert_equivalent(
        left: &TransientReference,
        right: &TransientReference,
    ) -> Result<(), HolonError> {
        assert_eq!(outcome(left, right)?, EquivalenceOutcome::Equivalent);
        Ok(())
    }

    fn assert_divergent(
        left: &TransientReference,
        right: &TransientReference,
    ) -> Result<Divergence, HolonError> {
        match outcome(left, right)? {
            EquivalenceOutcome::Equivalent => {
                panic!("expected divergent definitional-equivalence outcome")
            }
            EquivalenceOutcome::Divergent(divergence) => Ok(divergence),
        }
    }

    #[test]
    fn identical_property_maps_without_definitional_relationships_are_equivalent(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let (source_type, _) = simple_types(&context, "same-properties")?;
        let mut left = described_holon(&context, "same-key", &source_type)?;
        let mut right = described_holon(&context, "same-key", &source_type)?;
        left.with_property_value(CorePropertyTypeName::Description, "same")?;
        right.with_property_value(CorePropertyTypeName::Description, "same")?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn property_map_mismatch_is_divergent_at_root() -> Result<(), HolonError> {
        let context = build_context();
        let (source_type, _) = simple_types(&context, "property-mismatch")?;
        let mut left = described_holon(&context, "same-key", &source_type)?;
        let mut right = described_holon(&context, "same-key", &source_type)?;
        left.with_property_value(CorePropertyTypeName::Description, "left")?;
        right.with_property_value(CorePropertyTypeName::Description, "right")?;

        let divergence = assert_divergent(&left, &right)?;

        assert!(divergence.path.is_empty());
        assert!(divergence.reason.contains("Description"));
        Ok(())
    }

    #[test]
    fn saved_pairs_short_circuit_on_identity() -> Result<(), HolonError> {
        let context = build_context();
        let left = saved_reference(&context, 7);
        let same = saved_reference(&context, 7);
        let different = saved_reference(&context, 8);

        assert_eq!(left.definitional_equivalence(&same)?, EquivalenceOutcome::Equivalent);
        assert!(matches!(
            left.definitional_equivalence(&different)?,
            EquivalenceOutcome::Divergent(_)
        ));
        Ok(())
    }

    #[test]
    fn missing_extra_definitional_relationship_members_diverge_both_directions(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "missing-member")?;
        let relation = relationship_name("RelatedTo");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "RelatedTo",
            true,
            false,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let right = described_holon(&context, "root", &source_type)?;
        let member = described_holon(&context, "member", &target_type)?;
        left.add_related_holons(relation.clone(), vec![HolonReference::from(member)])?;

        assert!(matches!(outcome(&left, &right)?, EquivalenceOutcome::Divergent(_)));
        assert!(matches!(outcome(&right, &left)?, EquivalenceOutcome::Divergent(_)));
        Ok(())
    }

    #[test]
    fn wrong_definitional_target_diverges_with_relationship_path() -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "wrong-target")?;
        let relation = relationship_name("RelatedTo");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "RelatedTo",
            true,
            false,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let mut right = described_holon(&context, "root", &source_type)?;
        let left_member = described_holon(&context, "left-member", &target_type)?;
        let right_member = described_holon(&context, "right-member", &target_type)?;
        left.add_related_holons(relation.clone(), vec![HolonReference::from(left_member)])?;
        right.add_related_holons(relation.clone(), vec![HolonReference::from(right_member)])?;

        let divergence = assert_divergent(&left, &right)?;

        assert_eq!(divergence.path, vec![relation]);
        Ok(())
    }

    #[test]
    fn definitional_relationship_name_sets_must_match() -> Result<(), HolonError> {
        let context = build_context();
        let mut left_type = new_holon_type_descriptor(&context, "left-name-set-type", "RootType")?;
        let right_type = new_holon_type_descriptor(&context, "right-name-set-type", "RootType")?;
        let target_type =
            new_holon_type_descriptor(&context, "name-set-target-type", "TargetType")?;
        add_relationship_descriptor(
            &context,
            &mut left_type,
            &target_type,
            "OnlyOnLeft",
            true,
            false,
        )?;
        let left = described_holon(&context, "root", &left_type)?;
        let right = described_holon(&context, "root", &right_type)?;

        let divergence = assert_divergent(&left, &right)?;

        assert!(divergence.reason.contains("OnlyOnLeft"));
        Ok(())
    }

    #[test]
    fn different_described_by_targets_diverge_through_relationship_rule() -> Result<(), HolonError>
    {
        let context = build_context();
        let meta_type = new_holon_type_descriptor(&context, "descriptor-meta-type", "MetaType")?;
        let mut left_type =
            new_holon_type_descriptor(&context, "left-described-by-type", "LeftType")?;
        let mut right_type =
            new_holon_type_descriptor(&context, "right-described-by-type", "RightType")?;
        let described_by = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
        add_relationship_descriptor(
            &context,
            &mut left_type,
            &meta_type,
            "DescribedBy",
            true,
            false,
        )?;
        add_relationship_descriptor(
            &context,
            &mut right_type,
            &meta_type,
            "DescribedBy",
            true,
            false,
        )?;
        left_type.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&meta_type)],
        )?;
        right_type.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&meta_type)],
        )?;
        let left = described_holon(&context, "root", &left_type)?;
        let right = described_holon(&context, "root", &right_type)?;

        let divergence = assert_divergent(&left, &right)?;

        assert_eq!(divergence.path, vec![described_by]);
        Ok(())
    }

    #[test]
    fn same_key_different_saved_targets_are_divergent() -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "saved-target")?;
        let relation = relationship_name("SavedTarget");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "SavedTarget",
            true,
            true,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let mut right = described_holon(&context, "root", &source_type)?;
        left.add_related_holons(
            relation.clone(),
            vec![saved_reference_with_key(&context, 1, "same-saved-key")],
        )?;
        right.add_related_holons(
            relation.clone(),
            vec![saved_reference_with_key(&context, 2, "same-saved-key")],
        )?;

        let divergence = assert_divergent(&left, &right)?;

        assert_eq!(divergence.path, vec![relation]);
        assert!(divergence.reason.contains("Saved holon ids differ"));
        Ok(())
    }

    #[test]
    fn ordered_relationship_members_are_compared_positionally() -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "ordered")?;
        let relation = relationship_name("OrderedMembers");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "OrderedMembers",
            true,
            true,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let mut right = described_holon(&context, "root", &source_type)?;
        let left_a = described_holon(&context, "a", &target_type)?;
        let left_b = described_holon(&context, "b", &target_type)?;
        let right_a = described_holon(&context, "a", &target_type)?;
        let right_b = described_holon(&context, "b", &target_type)?;

        left.add_related_holons(
            relation.clone(),
            vec![HolonReference::from(left_a), HolonReference::from(left_b)],
        )?;
        right.add_related_holons(
            relation,
            vec![HolonReference::from(right_b), HolonReference::from(right_a)],
        )?;

        assert!(matches!(outcome(&left, &right)?, EquivalenceOutcome::Divergent(_)));
        Ok(())
    }

    #[test]
    fn unordered_relationship_members_are_matched_as_multisets() -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "unordered")?;
        let relation = relationship_name("UnorderedMembers");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "UnorderedMembers",
            true,
            false,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let mut right = described_holon(&context, "root", &source_type)?;
        let left_a = described_holon(&context, "a", &target_type)?;
        let left_b = described_holon(&context, "b", &target_type)?;
        let right_a = described_holon(&context, "a", &target_type)?;
        let right_b = described_holon(&context, "b", &target_type)?;

        left.add_related_holons(
            relation.clone(),
            vec![HolonReference::from(left_a), HolonReference::from(left_b)],
        )?;
        right.add_related_holons(
            relation,
            vec![HolonReference::from(right_b), HolonReference::from(right_a)],
        )?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn undescribed_holons_compare_by_properties_only_when_relationless() -> Result<(), HolonError> {
        let context = build_context();
        let left = new_test_holon(&context, "same-key")?;
        let right = new_test_holon(&context, "same-key")?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn undescribed_holons_with_relationship_members_error() -> Result<(), HolonError> {
        let context = build_context();
        let relation = relationship_name("Unlicensed");
        let mut left = new_test_holon(&context, "same-key")?;
        let right = new_test_holon(&context, "same-key")?;
        let member = new_test_holon(&context, "member")?;
        left.add_related_holons(relation, vec![HolonReference::from(member)])?;

        assert!(matches!(outcome(&left, &right), Err(HolonError::MissingDescribedBy { .. })));
        Ok(())
    }

    #[test]
    fn undescribed_holons_with_emptied_relationship_are_relationless() -> Result<(), HolonError> {
        // Removing the only member leaves the relationship name in the map with an
        // empty collection. An undescribed holon in that state must still present an
        // empty definitional surface rather than erroring with MissingDescribedBy.
        let context = build_context();
        let mut left = new_test_holon(&context, "same-key")?;
        let right = new_test_holon(&context, "same-key")?;
        let member = HolonReference::from(new_test_holon(&context, "member")?);
        left.add_related_holons(relationship_name("Unlicensed"), vec![member.clone()])?;
        left.remove_related_holons(relationship_name("Unlicensed"), vec![member])?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn non_definitional_relationships_are_ignored() -> Result<(), HolonError> {
        let context = build_context();
        let (mut source_type, target_type) = simple_types(&context, "non-def")?;
        let relation = relationship_name("NavigatesTo");
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "NavigatesTo",
            false,
            false,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let right = described_holon(&context, "root", &source_type)?;
        let member = described_holon(&context, "member", &target_type)?;
        left.add_related_holons(relation, vec![HolonReference::from(member)])?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn self_referencing_definitional_cycles_terminate() -> Result<(), HolonError> {
        let context = build_context();
        let mut source_type = new_holon_type_descriptor(&context, "self-cycle-type", "CycleType")?;
        let relation = relationship_name("LinksTo");
        let target_type = source_type.clone();
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "LinksTo",
            true,
            false,
        )?;
        let mut left = described_holon(&context, "root", &source_type)?;
        let mut right = described_holon(&context, "root", &source_type)?;
        left.add_related_holons(relation.clone(), vec![HolonReference::from(&left)])?;
        right.add_related_holons(relation, vec![HolonReference::from(&right)])?;

        assert_equivalent(&left, &right)
    }

    #[test]
    fn mutually_referencing_definitional_cycles_terminate() -> Result<(), HolonError> {
        let context = build_context();
        let mut source_type =
            new_holon_type_descriptor(&context, "mutual-cycle-type", "CycleType")?;
        let relation = relationship_name("LinksTo");
        let target_type = source_type.clone();
        add_relationship_descriptor(
            &context,
            &mut source_type,
            &target_type,
            "LinksTo",
            true,
            false,
        )?;
        let mut left_a = described_holon(&context, "a", &source_type)?;
        let mut left_b = described_holon(&context, "b", &source_type)?;
        let mut right_a = described_holon(&context, "a", &source_type)?;
        let mut right_b = described_holon(&context, "b", &source_type)?;
        left_a.add_related_holons(relation.clone(), vec![HolonReference::from(&left_b)])?;
        left_b.add_related_holons(relation.clone(), vec![HolonReference::from(&left_a)])?;
        right_a.add_related_holons(relation.clone(), vec![HolonReference::from(&right_b)])?;
        right_b.add_related_holons(relation, vec![HolonReference::from(&right_a)])?;

        assert_equivalent(&left_a, &right_a)
    }

    struct TestResolver {
        canonical_key: MapString,
        canonical_reference: HolonReference,
        match_by_key_property: PropertyName,
    }

    impl EquivalenceResolver for TestResolver {
        fn resolve(&self, reference: &HolonReference) -> Result<NodeResolution, HolonError> {
            match reference {
                HolonReference::Smart(smart_reference) => {
                    if smart_reference
                        .smart_property_values()
                        .and_then(|properties| properties.get(&self.match_by_key_property))
                        .is_some()
                    {
                        return Ok(NodeResolution::MatchByKey);
                    }
                }
                _ if reference.property_value(self.match_by_key_property.clone())?.is_some() => {
                    return Ok(NodeResolution::MatchByKey);
                }
                _ => {}
            }
            if !reference.is_saved() && reference.key()? == Some(self.canonical_key.clone()) {
                return Ok(NodeResolution::Canonical(self.canonical_reference.clone()));
            }
            Ok(NodeResolution::AsIs)
        }
    }

    #[test]
    fn resolver_canonical_substitution_feeds_saved_identity_rule() -> Result<(), HolonError> {
        let context = build_context();
        let alias = new_test_holon(&context, "alias")?;
        let canonical = saved_reference(&context, 9);
        let resolver = TestResolver {
            canonical_key: MapString("alias".to_string()),
            canonical_reference: canonical.clone(),
            match_by_key_property: property_name("MatchByKey"),
        };

        assert_eq!(
            alias.definitional_equivalence_with_resolver(&canonical, &resolver)?,
            EquivalenceOutcome::Equivalent
        );
        Ok(())
    }

    #[test]
    fn resolver_match_by_key_compares_keys_without_recursing() -> Result<(), HolonError> {
        let context = build_context();
        let mut marker_properties = PropertyMap::new();
        marker_properties
            .insert(property_name("MatchByKey"), BaseValue::BooleanValue(MapBoolean(true)));
        let left = HolonReference::smart_with_properties(context.context_handle(), holon_id(1), {
            let mut properties = marker_properties.clone();
            properties.insert(
                CorePropertyTypeName::Key.to_property_name(),
                BaseValue::StringValue(MapString("shared-key".to_string())),
            );
            properties
        });
        let right = saved_reference_with_key(&context, 2, "shared-key");
        let resolver = TestResolver {
            canonical_key: MapString("unused".to_string()),
            canonical_reference: saved_reference(&context, 3),
            match_by_key_property: property_name("MatchByKey"),
        };

        assert_eq!(
            left.definitional_equivalence_with_resolver(&right, &resolver)?,
            EquivalenceOutcome::Equivalent
        );
        Ok(())
    }

    #[test]
    fn divergence_path_is_assembled_outermost_first() -> Result<(), HolonError> {
        let context = build_context();
        let mut root_type = new_holon_type_descriptor(&context, "path-root-type", "RootType")?;
        let mut child_type = new_holon_type_descriptor(&context, "path-child-type", "ChildType")?;
        let grand_type = new_holon_type_descriptor(&context, "path-grand-type", "GrandType")?;
        let outer = relationship_name("Outer");
        let inner = relationship_name("Inner");
        add_relationship_descriptor(&context, &mut root_type, &child_type, "Outer", true, true)?;
        add_relationship_descriptor(&context, &mut child_type, &grand_type, "Inner", true, true)?;
        let mut left_root = described_holon(&context, "root", &root_type)?;
        let mut right_root = described_holon(&context, "root", &root_type)?;
        let mut left_child = described_holon(&context, "child", &child_type)?;
        let mut right_child = described_holon(&context, "child", &child_type)?;
        let mut left_grand = described_holon(&context, "grand", &grand_type)?;
        let mut right_grand = described_holon(&context, "grand", &grand_type)?;
        left_grand.with_property_value(CorePropertyTypeName::Description, "left")?;
        right_grand.with_property_value(CorePropertyTypeName::Description, "right")?;
        left_child.add_related_holons(inner.clone(), vec![HolonReference::from(left_grand)])?;
        right_child.add_related_holons(inner.clone(), vec![HolonReference::from(right_grand)])?;
        left_root.add_related_holons(outer.clone(), vec![HolonReference::from(left_child)])?;
        right_root.add_related_holons(outer.clone(), vec![HolonReference::from(right_child)])?;

        let divergence = assert_divergent(&left_root, &right_root)?;

        assert_eq!(divergence.path, vec![outer, inner]);
        Ok(())
    }
}
