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
//!   See [`crate::reference_layer::ReadableHolon::available_relationships`].
//! * **Optimization semantics** — mapping a requested relationship onto a
//!   concrete access path (e.g. rewriting an inverse navigation as a reverse
//!   scan of the declared relationship) is a future query-planning concern and
//!   is intentionally not handled here.
//!
//! Duplicate and ambiguity policy: two distinct declared descriptors sharing a
//! base relationship type name within one effective lineage fail eagerly as
//! `DuplicateInheritedDeclaration` — a local schema defect, detected during any
//! collection over that lineage. `AmbiguousRelationshipTraversal` is reserved
//! for lookups where a requested name matches multiple distinct candidates
//! across the declared/inverse union. Schemas are user-extensible, so global
//! `type_name` uniqueness cannot be assumed; a which-descriptor-wins resolution
//! policy belongs at the schema-import/holon-space level (alongside per-space
//! unique-`key` enforcement) and is deliberately not implemented here.

use std::collections::HashSet;

use crate::descriptors::{
    accessor_helpers, effective_descriptor_lineage,
    inheritance::collect_named_related_members_from_lineage, inheritance::equals_or_extends,
    inheritance::flatten_named_related_members, inheritance::flatten_related_members,
    DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor, InverseRelationshipDescriptor,
    RelationshipDescriptor, RelationshipDirection,
};
use crate::reference_layer::HolonReference;
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

/// Collects relationship descriptors from an already-selected declared surface lineage.
///
/// Anchors are expected in most-specific to most-general order. Each anchor is
/// read directly for `InstanceRelationships`; the caller owns which lineage is
/// appropriate for the subject. Repeated references are deduped, while distinct
/// descriptors with the same base relationship type name fail eagerly as a local
/// effective-lineage schema defect. `subject_label` is evaluated lazily, only
/// for duplicate-error attribution.
pub(crate) fn collect_relationships_from_anchors(
    anchors: impl IntoIterator<Item = Result<HolonReference, HolonError>>,
    subject_label: impl Fn() -> String,
) -> Result<Vec<RelationshipDescriptor>, HolonError> {
    let lineage = anchors.into_iter().collect::<Result<Vec<_>, _>>()?;
    let members = collect_named_related_members_from_lineage(
        lineage,
        CoreRelationshipTypeName::InstanceRelationships,
        "relationship",
        |member| {
            RelationshipDescriptor::from_holon(member.clone())
                .base_relationship_name()
                .map(|name| name.to_string())
        },
    )?;
    let _ = subject_label;
    Ok(members.into_iter().map(RelationshipDescriptor::from_holon).collect())
}

/// Collects declared relationship descriptors from an already-selected lineage.
pub(crate) fn collect_declared_from_anchors(
    anchors: impl IntoIterator<Item = Result<HolonReference, HolonError>>,
    subject_label: impl Fn() -> String,
) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
    let mut declared_descriptors = Vec::new();

    for descriptor in collect_relationships_from_anchors(anchors, subject_label)? {
        declared_descriptors.push(descriptor.try_into_declared_relationship_descriptor()?);
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
    flatten_named_related_members(
        endpoint.holon(),
        CoreRelationshipTypeName::InstanceRelationships,
        "relationship",
        |member| {
            RelationshipDescriptor::from_holon(member.clone())
                .base_relationship_name()
                .map(|name| name.to_string())
        },
    )?
    .into_iter()
    .map(|member| {
        RelationshipDescriptor::from_holon(member).try_into_declared_relationship_descriptor()
    })
    .collect()
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
/// [`crate::reference_layer::ReadableHolon::available_relationships`] for a
/// concrete source holon).
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

/// Finds a relationship declaration on a holon's effective relationship surface.
///
/// Ordinary runtime holons draw this surface from `DescribedBy -> Extends*`. Type holons also
/// contribute their own `Extends*` lineage before the describing-type lineage.
pub fn effective_relationship_declaration(
    source_holon: &HolonReference,
    name: impl ToRelationshipName,
) -> Result<RelationshipDescriptor, HolonError> {
    let requested_name = name.to_relationship_name();
    let requested = requested_name.to_string();

    let declared_relationships = collect_relationships_from_anchors(
        effective_descriptor_lineage(source_holon)?.into_iter().map(Ok),
        || accessor_helpers::descriptor_label(source_holon),
    )?;

    for descriptor in declared_relationships {
        if descriptor.base_relationship_name()? == requested_name {
            return Ok(descriptor);
        }
    }

    Err(HolonError::DescriptorDeclarationNotFound {
        kind: "relationship".to_string(),
        name: requested,
        descriptor: accessor_helpers::descriptor_label(source_holon),
    })
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
    fn declared_name_is_not_effective_outbound_from_target_type() -> Result<(), HolonError> {
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
    fn effective_declared_relationships_includes_direct_and_inherited() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "effective-declared-lineage",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let target_type =
            new_holon_type_descriptor(&context, "effective-declared-target", "TargetType")?;
        let declared_type = new_descriptor_holon(
            &context,
            "effective-declared-member-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut child =
            new_holon_type_descriptor(&context, "effective-declared-child", "ChildBookType")?;
        let mut tagged_with = new_relationship_descriptor_holon(
            &context,
            "effective-declared-tagged-with",
            "TaggedWith",
            HolonReference::from(&child),
            HolonReference::from(&target_type),
        )?;

        tagged_with
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&fixture.source_type)],
        )?;
        child.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![tagged_with.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(child.into());
        let names = descriptor
            .effective_declared_relationships()?
            .into_iter()
            .map(|declared| declared.base_relationship_name().map(|name| name.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(names, vec!["TaggedWith".to_string(), "AuthoredBy".to_string()]);
        Ok(())
    }

    #[test]
    fn effective_inverse_relationships_returns_typed_inverse_descriptors() -> Result<(), HolonError>
    {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "effective-inverse-typed",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        let inverses = descriptor.effective_inverse_relationships()?;

        assert_eq!(inverses.len(), 1);
        assert_eq!(inverses[0].base_relationship_name()?, relationship_name("Authors"));
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
    fn finds_ordinary_instance_declaration_through_described_by_lineage() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut holon_type =
            new_holon_type_descriptor(&context, "surface-holon-type", "HolonType")?;
        let mut book_type = new_holon_type_descriptor(&context, "surface-book-type", "Book")?;
        let person_type = new_holon_type_descriptor(&context, "surface-person-type", "Person")?;
        let authored_by = new_relationship_descriptor_holon(
            &context,
            "surface-authored-by",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let mut book = crate::descriptors::test_support::new_test_holon(&context, "surface-book")?;

        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&authored_by).into()],
        )?;
        book_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![holon_type.into()])?;
        book.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![book_type.into()])?;

        let declaration =
            effective_relationship_declaration(&(&book).into(), relationship_name("AuthoredBy"))?;

        assert_eq!(declaration.base_relationship_name()?.to_string(), "AuthoredBy");
        Ok(())
    }

    #[test]
    fn finds_descriptor_holon_declaration_through_own_extends_lineage() -> Result<(), HolonError> {
        let context = build_context();
        let mut type_descriptor =
            new_holon_type_descriptor(&context, "TypeDescriptor.HolonType", "TypeDescriptor")?;
        type_descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&type_descriptor)],
        )?;
        let mut meta_relationship_type = new_holon_type_descriptor(
            &context,
            "surface-meta-relationship-type",
            "MetaRelationshipType",
        )?;
        let mut declared_relationship_type = new_descriptor_holon(
            &context,
            "surface-declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let source_type = new_relationship_descriptor_holon(
            &context,
            "surface-source-type",
            "SourceType",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        let target_descriptor =
            new_holon_type_descriptor(&context, "surface-target-type", "Target")?;
        let mut relationship_descriptor = new_relationship_descriptor_holon(
            &context,
            "surface-affords-operator",
            "AffordsOperator",
            (&meta_relationship_type).into(),
            (&target_descriptor).into(),
        )?;

        meta_relationship_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&source_type).into()],
        )?;
        declared_relationship_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&meta_relationship_type).into()],
        )?;
        relationship_descriptor.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![declared_relationship_type.into()],
        )?;
        relationship_descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.into()],
        )?;

        let declaration =
            effective_relationship_declaration(&(&relationship_descriptor).into(), "SourceType")?;

        assert_eq!(declaration.base_relationship_name()?.to_string(), "SourceType");
        Ok(())
    }

    #[test]
    fn shared_tail_lineage_does_not_create_false_duplicate() -> Result<(), HolonError> {
        let context = build_context();
        let mut meta_type_descriptor = new_holon_type_descriptor(
            &context,
            "surface-meta-type-descriptor",
            "MetaTypeDescriptor",
        )?;
        let mut meta_holon_type =
            new_holon_type_descriptor(&context, "surface-meta-holon-type", "MetaHolonType")?;
        let mut holon_type =
            new_holon_type_descriptor(&context, "surface-tail-holon-type", "HolonType")?;
        let mut type_descriptor =
            new_holon_type_descriptor(&context, "TypeDescriptor.HolonType", "TypeDescriptor")?;
        type_descriptor.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![HolonReference::from(&type_descriptor)],
        )?;
        let properties = new_relationship_descriptor_holon(
            &context,
            "surface-properties",
            "Properties",
            (&meta_type_descriptor).into(),
            (&meta_type_descriptor).into(),
        )?;
        let mut descriptor_holon =
            new_holon_type_descriptor(&context, "surface-custom-descriptor", "CustomDescriptor")?;

        meta_type_descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![properties.into()],
        )?;
        meta_holon_type.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&meta_type_descriptor).into()],
        )?;
        holon_type
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![meta_holon_type.into()])?;
        type_descriptor
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![holon_type.into()])?;
        descriptor_holon.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&meta_type_descriptor).into()],
        )?;
        descriptor_holon.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![type_descriptor.into()],
        )?;

        let declaration =
            effective_relationship_declaration(&(&descriptor_holon).into(), "Properties")?;

        assert_eq!(declaration.base_relationship_name()?.to_string(), "Properties");
        Ok(())
    }

    #[test]
    fn duplicate_relationship_declarations_by_base_name_error() -> Result<(), HolonError> {
        let context = build_context();
        let mut parent_type = new_holon_type_descriptor(&context, "surface-parent-type", "Parent")?;
        let mut book_type = new_holon_type_descriptor(&context, "surface-dup-book-type", "Book")?;
        let person_type = new_holon_type_descriptor(&context, "surface-dup-person-type", "Person")?;
        let authored_by_a = new_relationship_descriptor_holon(
            &context,
            "surface-authored-by-a",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let authored_by_b = new_relationship_descriptor_holon(
            &context,
            "surface-authored-by-b",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let mut book =
            crate::descriptors::test_support::new_test_holon(&context, "surface-dup-book")?;

        parent_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![authored_by_a.into()],
        )?;
        book_type
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![parent_type.into()])?;
        book_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![authored_by_b.into()],
        )?;
        book.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![book_type.into()])?;

        assert!(matches!(
            effective_relationship_declaration(&(&book).into(), "AuthoredBy"),
            Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));
        Ok(())
    }
}
