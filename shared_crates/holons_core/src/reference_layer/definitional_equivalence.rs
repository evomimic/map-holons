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
//! 4. Compare raw `property_map` values internally via
//!    `essential_content_impl()?.property_map`. This intentionally excludes
//!    `EssentialHolonContent.key` and `.errors`; key semantics are already part
//!    of the property surface, and `errors` are lifecycle-dependent rather than
//!    definition-level content.
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
#![allow(dead_code)]

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
    // raw-map touch in the algorithm, and it excludes key/errors from
    // EssentialHolonContent for the reasons described in the module header.
    let left_content = left_reference.essential_content_impl()?;
    let right_content = right_reference.essential_content_impl()?;
    if let Some(divergence) =
        compare_property_maps(&left_content.property_map, &right_content.property_map)
    {
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
            if relationship_map_is_empty(&all_related) {
                Ok(BTreeMap::new())
            } else {
                Err(HolonError::MissingDescribedBy { holon: reference.summarize()? })
            }
        }
        Err(error) => Err(error),
    }
}

fn relationship_map_is_empty(relationship_map: &RelationshipMap) -> bool {
    relationship_map.count() == 0
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
