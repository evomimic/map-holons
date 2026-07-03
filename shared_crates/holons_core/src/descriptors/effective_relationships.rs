//! Effective outbound relationship discovery over the descriptor graph.
//!
//! This module keeps several distinct concerns deliberately separate:
//!
//! * **Schema semantics** — `SourceType`, `TargetType`, `HasInverse`/`InverseOf`
//!   describe the relationship pair itself. A declared relationship is licensed
//!   on its source type via `InstanceRelationships`; the paired inverse
//!   descriptor's `SourceType` is the declared relationship's `TargetType`.
//! * **Mutation semantics** — only declared relationships are editable on a
//!   source holon. Inverse relationships are materialized as SmartLinks at
//!   commit time and are never written directly.
//! * **Navigation semantics** — navigation is always *outbound from the
//!   source holon*. A relationship (declared **or** inverse descriptor) is
//!   effective for type `T` iff its `SourceType` equals-or-is-extended-by `T`.
//!   Inverse descriptors are first-class outbound relationships, not a
//!   syntactic alias for traversing the declared relationship backwards.
//! * **Runtime-state semantics** — whether an effective relationship is
//!   *available* on a concrete holon depends on the source reference state:
//!   declared relationships are always available, while inverse relationships
//!   require a committed source (inverse SmartLinks are populated at commit).
//!   See [`available_relationships`].
//! * **Optimization semantics** — mapping a requested relationship onto a
//!   concrete access path (e.g. rewriting an inverse navigation as a reverse
//!   scan of the declared relationship) is a future query-planning concern and
//!   is intentionally not handled here.

use std::collections::HashSet;

use crate::descriptors::{
    accessor_helpers, inheritance::equals_or_extends, inheritance::flatten_related_members,
    walk_extends_chain, DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor,
    InverseRelationshipDescriptor, RelationshipDescriptor, RelationshipDirection,
};
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::{HolonError, RelationshipName};
use type_names::{CoreRelationshipTypeName, ToRelationshipName};

/// A relationship descriptor effective outbound from a descriptor endpoint.
pub struct QualifiedRelationship {
    /// The matched relationship descriptor: the declared descriptor for
    /// declared navigation, or the inverse descriptor for inverse navigation.
    pub descriptor: RelationshipDescriptor,
    /// Whether `descriptor` is the declared or inverse relationship descriptor.
    pub descriptor_direction: RelationshipDirection,
}

struct NavigationCandidate {
    name: RelationshipName,
    qualified: QualifiedRelationship,
    // Identity of the matched descriptor, used to dedupe candidates reached
    // through multiple discovery paths.
    descriptor_id: String,
}

struct CandidateSet {
    candidates: Vec<NavigationCandidate>,
    // Set when a licensed target-owned declaration lacks `HasInverse`, so the
    // schema defect can be surfaced instead of a generic not-found/omission.
    missing_inverse_declaration: Option<HolonReference>,
    requires_materialized_target_index: bool,
}

/// Collects declared relationship descriptors from an already-selected lineage.
///
/// Anchors are expected in most-specific to most-general order. Each anchor is
/// read directly for `InstanceRelationships`; the caller owns which lineage is
/// appropriate for the subject. Repeated references are deduped, while distinct
/// descriptors with the same base relationship type name fail eagerly as a local
/// effective-lineage schema defect.
pub(crate) fn collect_declared_from_anchors(
    anchors: impl IntoIterator<Item = Result<HolonReference, HolonError>>,
    subject_label: &str,
) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
    let mut declared_descriptors = Vec::new();
    let mut seen_declaration_refs = HashSet::new();
    let mut seen_declaration_names = HashSet::new();

    for anchor in anchors {
        let anchor = anchor?;
        let collection_arc =
            anchor.related_holons(CoreRelationshipTypeName::InstanceRelationships)?;
        let collection = collection_arc.read().map_err(accessor_helpers::lock_error)?;

        for declaration_ref in collection.get_members() {
            if !seen_declaration_refs.insert(declaration_ref.reference_id_string()) {
                continue;
            }

            let declared = DeclaredRelationshipDescriptor::try_from_holon(declaration_ref.clone())?;
            let declaration_name = declared.base_relationship_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen_declaration_names.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "relationship".to_string(),
                    name: declaration_label,
                    descriptor: subject_label.to_string(),
                });
            }

            declared_descriptors.push(declared);
        }
    }

    Ok(declared_descriptors)
}

/// Enumerates effective declared relationships for `endpoint`.
///
/// This is staged-safe because it uses the forward `InstanceRelationships`
/// declaration surface on the endpoint's `Extends` lineage.
pub(crate) fn effective_declared_relationships(
    endpoint: &HolonDescriptor,
) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
    collect_declared_from_anchors(
        walk_extends_chain(endpoint.holon()),
        &accessor_helpers::descriptor_label(endpoint.holon()),
    )
}

/// Enumerates effective inverse relationships for `endpoint`.
///
/// This uses the materialized `TargetOf` index and therefore keeps the staged
/// guard when the endpoint is unsaved and the index is empty.
pub(crate) fn effective_inverse_relationships(
    endpoint: &HolonDescriptor,
) -> Result<Vec<InverseRelationshipDescriptor>, HolonError> {
    let set = collect_inverse_candidates(endpoint)?;

    if set.requires_materialized_target_index {
        return Err(HolonError::UnsupportedStagedTraversal {
            relationship: CoreRelationshipTypeName::TargetOf.to_relationship_name().to_string(),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    surface_missing_inverse(set.missing_inverse_declaration)?;

    Ok(set.inverse_descriptors)
}

/// Enumerates the effective outbound relationships for `endpoint`.
///
/// The result is the union of:
/// * declared relationships licensed on this type (or inherited) via
///   `InstanceRelationships`, and
/// * inverse relationships whose `SourceType` is this type, discovered through
///   the materialized `TargetOf` index and the paired `HasInverse` descriptor.
///
/// Errors with `UnsupportedStagedTraversal` when `endpoint` is unsaved and the
/// `TargetOf` index is empty: the inverse portion of the enumeration is not
/// answerable yet. Callers needing a staged-safe declared-only enumeration
/// should use [`HolonDescriptor::effective_declared_relationships`] (or
/// [`available_relationships`] for a concrete source holon).
pub(crate) fn effective_relationships(
    endpoint: &HolonDescriptor,
) -> Result<Vec<QualifiedRelationship>, HolonError> {
    let mut relationships = effective_declared_relationships(endpoint)?
        .into_iter()
        .map(|declared| QualifiedRelationship {
            descriptor: RelationshipDescriptor::from_holon(declared.holon().clone()),
            descriptor_direction: RelationshipDirection::Declared,
        })
        .collect::<Vec<_>>();

    relationships.extend(effective_inverse_relationships(endpoint)?.into_iter().map(|inverse| {
        QualifiedRelationship {
            descriptor: RelationshipDescriptor::from_holon(inverse.holon().clone()),
            descriptor_direction: RelationshipDirection::Inverse,
        }
    }));

    Ok(relationships)
}

/// Validates that `relationship_name` is effective outbound from `endpoint`.
///
/// Matches the requested name against both declared and inverse effective
/// relationships (see [`effective_relationships`]).
pub(crate) fn allows_relationship(
    endpoint: &HolonDescriptor,
    relationship_name: RelationshipName,
) -> Result<QualifiedRelationship, HolonError> {
    let set = collect_candidates(endpoint)?;

    let mut matches: Vec<NavigationCandidate> = set
        .candidates
        .into_iter()
        .filter(|candidate| candidate.name == relationship_name)
        .collect();

    if matches.len() == 1 {
        return Ok(matches.remove(0).qualified);
    }

    if matches.len() > 1 {
        return Err(HolonError::AmbiguousRelationshipTraversal {
            relationship: relationship_name.to_string(),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    surface_missing_inverse(set.missing_inverse_declaration)?;

    if set.requires_materialized_target_index {
        return Err(HolonError::UnsupportedStagedTraversal {
            relationship: relationship_name.to_string(),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    Err(HolonError::DescriptorDeclarationNotFound {
        kind: "relationship".to_string(),
        name: relationship_name.to_string(),
        descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
    })
}

/// Enumerates the relationships actually available on `source_ref` in its
/// current reference state.
///
/// Availability filters the type-level navigable set by source commit state:
///
/// | Source reference state          | Declared | Inverse |
/// | ------------------------------- | -------- | ------- |
/// | `SmartReference` (saved)        | yes      | yes     |
/// | `StagedReference` committed     | yes      | yes     |
/// | `StagedReference` not committed | yes      | no      |
/// | `TransientReference`            | yes      | no      |
///
/// Inverse relationships require a committed source because their SmartLinks
/// are only materialized at commit. For uncommitted sources the inverse
/// candidates are excluded by construction (the `TargetOf` index is never
/// consulted), so this helper never returns `UnsupportedStagedTraversal`.
///
/// This lives in `descriptors/` as a free helper for now; it could later move
/// onto `HolonReference` itself.
pub fn available_relationships(
    source_ref: &HolonReference,
) -> Result<Vec<QualifiedRelationship>, HolonError> {
    let descriptor = source_ref.holon_descriptor()?;

    let source_is_committed = match source_ref {
        HolonReference::Smart(_) => true,
        HolonReference::Staged(staged) => staged.is_committed()?,
        HolonReference::Transient(_) => false,
    };

    if source_is_committed {
        return descriptor.effective_relationships();
    }

    // Uncommitted sources carry no materialized inverse SmartLinks, so only
    // declared relationships are available; the TargetOf index is not needed.
    Ok(descriptor
        .effective_declared_relationships()?
        .into_iter()
        .map(|declared| QualifiedRelationship {
            descriptor: RelationshipDescriptor::from_holon(declared.holon().clone()),
            descriptor_direction: RelationshipDirection::Declared,
        })
        .collect())
}

fn collect_candidates(endpoint: &HolonDescriptor) -> Result<CandidateSet, HolonError> {
    let mut candidates: Vec<NavigationCandidate> = Vec::new();

    for declared in effective_declared_relationships(endpoint)? {
        let name = declared.base_relationship_name()?;
        push_unique(
            &mut candidates,
            NavigationCandidate {
                name,
                descriptor_id: declared.holon().reference_id_string(),
                qualified: QualifiedRelationship {
                    descriptor: RelationshipDescriptor::from_holon(declared.holon().clone()),
                    descriptor_direction: RelationshipDirection::Declared,
                },
            },
        );
    }

    let inverse_set = collect_inverse_candidates(endpoint)?;
    for inverse in inverse_set.inverse_descriptors {
        let name = inverse.base_relationship_name()?;
        push_unique(
            &mut candidates,
            NavigationCandidate {
                name,
                descriptor_id: inverse.holon().reference_id_string(),
                qualified: QualifiedRelationship {
                    descriptor: RelationshipDescriptor::from_holon(inverse.holon().clone()),
                    descriptor_direction: RelationshipDirection::Inverse,
                },
            },
        );
    }

    Ok(CandidateSet {
        candidates,
        missing_inverse_declaration: inverse_set.missing_inverse_declaration,
        requires_materialized_target_index: inverse_set.requires_materialized_target_index,
    })
}

struct InverseCandidateSet {
    inverse_descriptors: Vec<InverseRelationshipDescriptor>,
    missing_inverse_declaration: Option<HolonReference>,
    requires_materialized_target_index: bool,
}

fn collect_inverse_candidates(
    endpoint: &HolonDescriptor,
) -> Result<InverseCandidateSet, HolonError> {
    let mut inverse_descriptors = Vec::new();
    let mut seen_inverse_refs = HashSet::new();
    let mut missing_inverse_declaration = None;

    // Inverse relationships whose SourceType is this type, discovered through
    // the materialized TargetOf index on the declared relationship's target.
    let target_of_members =
        flatten_related_members(endpoint.holon(), CoreRelationshipTypeName::TargetOf)?;
    for member in &target_of_members {
        let declared = DeclaredRelationshipDescriptor::try_from_holon(member.clone())?;
        if !target_endpoint_is_compatible(endpoint, &declared)? {
            continue;
        }
        if !source_licenses_declared_relationship(&declared)? {
            continue;
        }

        match declared.has_inverse()? {
            Some(inverse) => {
                if seen_inverse_refs.insert(inverse.holon().reference_id_string()) {
                    inverse_descriptors.push(inverse);
                }
            }
            None => {
                // A licensed declaration reachable through TargetOf must carry
                // HasInverse; remember the defect to surface it explicitly.
                if missing_inverse_declaration.is_none() {
                    missing_inverse_declaration = Some(declared.holon().clone());
                }
            }
        }
    }

    Ok(InverseCandidateSet {
        inverse_descriptors,
        missing_inverse_declaration,
        // A staged/transient endpoint cannot rely on the materialized `TargetOf`
        // inverse index, so an empty index there means "inverse navigation is
        // not answerable yet" rather than "no inverse relationships". This
        // deliberately absorbs genuine not-found cases for unsaved endpoints
        // (e.g. a typo that also finds nothing declared): they surface as
        // `UnsupportedStagedTraversal` instead of `DescriptorDeclarationNotFound`.
        // A saved endpoint has a trustworthy index, so an empty result there is
        // a true absence.
        requires_materialized_target_index: target_of_members.is_empty()
            && !endpoint.holon().is_saved(),
    })
}

fn push_unique(candidates: &mut Vec<NavigationCandidate>, candidate: NavigationCandidate) {
    if candidates.iter().any(|existing| existing.descriptor_id == candidate.descriptor_id) {
        return;
    }
    candidates.push(candidate);
}

fn surface_missing_inverse(declared_ref: Option<HolonReference>) -> Result<(), HolonError> {
    if let Some(declared_ref) = declared_ref {
        // The marker is only set when `has_inverse()` was `None`, so
        // `required_inverse()` always errors here; the `Ok` case cannot occur.
        DeclaredRelationshipDescriptor::try_from_holon(declared_ref)?.required_inverse()?;
    }
    Ok(())
}

fn target_endpoint_is_compatible(
    endpoint: &HolonDescriptor,
    declared: &DeclaredRelationshipDescriptor,
) -> Result<bool, HolonError> {
    equals_or_extends(endpoint.holon(), declared.target_type()?.holon())
}

fn source_licenses_declared_relationship(
    declared: &DeclaredRelationshipDescriptor,
) -> Result<bool, HolonError> {
    let source_type = declared.source_type()?;
    let declared_name = declared.base_relationship_name()?;
    for licensed in source_type.effective_declared_relationships()? {
        if licensed.base_relationship_name()? == declared_name {
            return Ok(
                licensed.holon().reference_id_string() == declared.holon().reference_id_string()
            );
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon,
    };
    use crate::reference_layer::{HolonReference, TransientReference, WritableHolon};
    use base_types::MapString;
    use core_types::RelationshipName;
    use std::sync::Arc;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    struct RelationshipPairFixture {
        source_type: TransientReference,
        target_type: TransientReference,
        declared: TransientReference,
    }

    fn relationship_name(name: &str) -> RelationshipName {
        RelationshipName(MapString(name.to_string()))
    }

    fn build_relationship_pair(
        context: &Arc<crate::core_shared_objects::transactions::TransactionContext>,
        key_prefix: &str,
        declared_name: &str,
        inverse_name: &str,
        license_source: bool,
        populate_target_of: bool,
    ) -> Result<RelationshipPairFixture, HolonError> {
        let declared_type = new_descriptor_holon(
            context,
            &format!("{key_prefix}-declared-type"),
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let inverse_type = new_descriptor_holon(
            context,
            &format!("{key_prefix}-inverse-type"),
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?;
        let mut source_type =
            new_holon_type_descriptor(context, &format!("{key_prefix}-source"), "BookType")?;
        let mut target_type =
            new_holon_type_descriptor(context, &format!("{key_prefix}-target"), "PersonType")?;
        let mut declared = new_relationship_descriptor_holon(
            context,
            &format!("{key_prefix}-declared"),
            declared_name,
            HolonReference::from(&source_type),
            HolonReference::from(&target_type),
        )?;
        let mut inverse = new_relationship_descriptor_holon(
            context,
            &format!("{key_prefix}-inverse"),
            inverse_name,
            HolonReference::from(&target_type),
            HolonReference::from(&source_type),
        )?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        inverse.add_related_holons(CoreRelationshipTypeName::Extends, vec![inverse_type.into()])?;
        declared
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&inverse).into()])?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::InverseOf, vec![(&declared).into()])?;

        if license_source {
            source_type.add_related_holons(
                CoreRelationshipTypeName::InstanceRelationships,
                vec![(&declared).into()],
            )?;
        }
        if populate_target_of {
            target_type
                .add_related_holons(CoreRelationshipTypeName::TargetOf, vec![(&declared).into()])?;
        }

        Ok(RelationshipPairFixture { source_type, target_type, declared })
    }

    fn qualified_names(
        relationships: &[QualifiedRelationship],
    ) -> Result<Vec<(String, RelationshipDirection)>, HolonError> {
        relationships
            .iter()
            .map(|qualified| {
                Ok((
                    qualified.descriptor.base_relationship_name()?.to_string(),
                    qualified.descriptor_direction,
                ))
            })
            .collect()
    }

    #[test]
    fn allows_relationship_resolves_declared_name_outbound() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "source-declared",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.source_type.into());

        let qualified = descriptor.allows_relationship("authored_by")?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Declared);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("AuthoredBy"));

        Ok(())
    }

    #[test]
    fn allows_relationship_resolves_inverse_name_outbound_from_target_type(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "target-inverse",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        let qualified = descriptor.allows_relationship("authors")?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Inverse);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("Authors"));

        Ok(())
    }

    #[test]
    fn allows_relationship_reports_not_found_for_unknown_name_on_saved_index(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let fixture =
            build_relationship_pair(&context, "unknown-name", "AuthoredBy", "Authors", true, true)?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        assert!(matches!(
            descriptor.allows_relationship("missing_relationship"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "MissingRelationship"
        ));

        Ok(())
    }

    #[test]
    fn declared_name_is_not_navigable_from_target_type() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "declared-from-target",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        // The declared relationship's SourceType is the source type, so it is
        // not outbound from the target type; only the inverse name is.
        assert!(matches!(
            descriptor.allows_relationship("authored_by"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));

        Ok(())
    }

    #[test]
    fn missing_has_inverse_on_licensed_target_of_member_fails_clearly() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "missing-inverse-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut source_type =
            new_holon_type_descriptor(&context, "missing-inverse-source", "BookType")?;
        let mut target_type =
            new_holon_type_descriptor(&context, "missing-inverse-target", "PersonType")?;
        let mut declared = new_relationship_descriptor_holon(
            &context,
            "missing-inverse-authored-by",
            "AuthoredBy",
            HolonReference::from(&source_type),
            HolonReference::from(&target_type),
        )?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;
        target_type
            .add_related_holons(CoreRelationshipTypeName::TargetOf, vec![declared.into()])?;

        let descriptor = HolonDescriptor::from_holon(target_type.into());

        assert!(matches!(
            descriptor.allows_relationship("authors"),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "HasInverse"
        ));

        Ok(())
    }

    #[test]
    fn inherited_source_owned_relationships_are_discovered() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "inherited-source",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let mut child = new_holon_type_descriptor(&context, "child-book-type", "ChildBookType")?;
        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&fixture.source_type)],
        )?;
        let descriptor = HolonDescriptor::from_holon(child.into());

        let qualified = descriptor.allows_relationship("authored_by")?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Declared);

        Ok(())
    }

    #[test]
    fn effective_declared_relationships_errors_on_duplicate_base_name() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "duplicate-effective-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut parent =
            new_holon_type_descriptor(&context, "duplicate-effective-parent", "ParentType")?;
        let mut child =
            new_holon_type_descriptor(&context, "duplicate-effective-child", "ChildType")?;
        let target = new_holon_type_descriptor(&context, "duplicate-effective-target", "Target")?;
        let mut parent_declared = new_relationship_descriptor_holon(
            &context,
            "duplicate-effective-parent-rel",
            "AuthoredBy",
            HolonReference::from(&parent),
            HolonReference::from(&target),
        )?;
        let mut child_declared = new_relationship_descriptor_holon(
            &context,
            "duplicate-effective-child-rel",
            "AuthoredBy",
            HolonReference::from(&child),
            HolonReference::from(&target),
        )?;

        parent_declared.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&declared_type)],
        )?;
        child_declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        parent.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![parent_declared.into()],
        )?;
        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&parent)],
        )?;
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![child_declared.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(child.into());

        assert!(matches!(
            descriptor.effective_declared_relationships(),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));

        Ok(())
    }

    #[test]
    fn inherited_target_anchors_are_compatible() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "target-anchor",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let mut child_target =
            new_holon_type_descriptor(&context, "author-target-type", "AuthorType")?;
        child_target.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&fixture.target_type)],
        )?;
        fixture.target_type.clone().add_related_holons(
            CoreRelationshipTypeName::TargetOf,
            vec![HolonReference::from(&fixture.declared)],
        )?;
        let descriptor = HolonDescriptor::from_holon(child_target.into());

        let qualified = descriptor.allows_relationship("authors")?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Inverse);

        Ok(())
    }

    #[test]
    fn target_of_candidate_without_source_license_is_rejected() -> Result<(), HolonError> {
        let context = build_context();
        let fixture =
            build_relationship_pair(&context, "unlicensed", "AuthoredBy", "Authors", false, true)?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        assert!(matches!(
            descriptor.allows_relationship("authors"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "Authors"
        ));

        Ok(())
    }

    #[test]
    fn ambiguous_distinct_inverse_matches_fail() -> Result<(), HolonError> {
        let context = build_context();
        let mut first = build_relationship_pair(
            &context,
            "ambiguous-one",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let second =
            build_relationship_pair(&context, "ambiguous-two", "EditedBy", "Authors", true, false)?;
        first.target_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&second.target_type)],
        )?;
        first.target_type.add_related_holons(
            CoreRelationshipTypeName::TargetOf,
            vec![HolonReference::from(&first.declared), HolonReference::from(&second.declared)],
        )?;
        let descriptor = HolonDescriptor::from_holon(first.target_type.into());

        assert!(matches!(
            descriptor.allows_relationship("authors"),
            Err(HolonError::AmbiguousRelationshipTraversal { relationship, .. })
                if relationship == "Authors"
        ));

        Ok(())
    }

    #[test]
    fn staged_endpoint_without_target_of_index_is_guarded() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "staged-target",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let staged_target = context.mutation().stage_new_holon(fixture.target_type)?;
        let descriptor = HolonDescriptor::from_holon(staged_target.into());

        assert!(matches!(
            descriptor.allows_relationship("authors"),
            Err(HolonError::UnsupportedStagedTraversal { relationship, .. })
                if relationship == "Authors"
        ));

        Ok(())
    }

    #[test]
    fn effective_relationships_enumerates_declared_and_inverse() -> Result<(), HolonError> {
        let context = build_context();
        // Outbound from PersonType: an inverse (Authors, from being the target
        // of AuthoredBy) and a declared relationship of its own (MemberOf).
        let fixture =
            build_relationship_pair(&context, "enumerate", "AuthoredBy", "Authors", true, true)?;
        let club_type = new_holon_type_descriptor(&context, "club-type", "ClubType")?;
        let declared_type = new_descriptor_holon(
            &context,
            "enumerate-member-of-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut member_of = new_relationship_descriptor_holon(
            &context,
            "enumerate-member-of",
            "MemberOf",
            HolonReference::from(&fixture.target_type),
            HolonReference::from(&club_type),
        )?;
        member_of
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        fixture.target_type.clone().add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![member_of.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());
        let names = qualified_names(&descriptor.effective_relationships()?)?;

        assert!(names.contains(&("MemberOf".to_string(), RelationshipDirection::Declared)));
        assert!(names.contains(&("Authors".to_string(), RelationshipDirection::Inverse)));
        assert_eq!(names.len(), 2);

        Ok(())
    }

    #[test]
    fn effective_relationships_guards_unsaved_endpoint_without_index() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "enumerate-staged",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let staged_target = context.mutation().stage_new_holon(fixture.target_type)?;
        let descriptor = HolonDescriptor::from_holon(staged_target.into());

        assert!(matches!(
            descriptor.effective_relationships(),
            Err(HolonError::UnsupportedStagedTraversal { .. })
        ));

        Ok(())
    }

    #[test]
    fn available_relationships_on_transient_source_is_declared_only() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "avail-transient",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;
        let mut instance =
            crate::descriptors::test_support::new_test_holon(&context, "avail-book")?;
        instance.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&fixture.source_type)],
        )?;

        let names = qualified_names(&available_relationships(&(&instance).into())?)?;

        assert_eq!(names, vec![("AuthoredBy".to_string(), RelationshipDirection::Declared)]);

        Ok(())
    }

    #[test]
    fn available_relationships_on_uncommitted_staged_source_is_declared_only(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            "avail-staged-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?)?;
        let mut source_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "avail-staged-source",
            "BookType",
        )?)?;
        let target_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "avail-staged-target",
            "PersonType",
        )?)?;
        let declared_transient = new_relationship_descriptor_holon(
            &context,
            "avail-staged-authored-by",
            "AuthoredBy",
            HolonReference::from(&source_type),
            HolonReference::from(&target_type),
        )?;
        let mut declared = context.mutation().stage_new_holon(declared_transient)?;
        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&declared_type).into()])?;
        source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;
        let instance =
            crate::descriptors::test_support::new_test_holon(&context, "avail-staged-book")?;
        let mut staged_instance = context.mutation().stage_new_holon(instance)?;
        staged_instance.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![(&source_type).into()],
        )?;

        let names = qualified_names(&available_relationships(&staged_instance.into())?)?;

        assert_eq!(names, vec![("AuthoredBy".to_string(), RelationshipDirection::Declared)]);

        Ok(())
    }
}
