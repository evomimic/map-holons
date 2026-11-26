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
//!   See [`crate::descriptors::EffectiveHolonDescriptor::available_relationships`].
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
    accessor_helpers, inheritance::effective_relationship_targets, DeclaredRelationshipDescriptor,
    Descriptor, HolonDescriptor, InverseRelationshipDescriptor, RelationshipDescriptor,
    RelationshipDirection,
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

/// Enumerates effective declared relationships for `endpoint`.
///
/// This is staged-safe because it uses the forward `InstanceRelationships`
/// declaration surface on the endpoint's `Extends` lineage.
pub(crate) fn effective_declared_relationships(
    endpoint: &HolonDescriptor,
) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
    let members = effective_relationship_targets(
        endpoint.holon(),
        CoreRelationshipTypeName::InstanceRelationships,
    )?;
    collect_declared_from_relationship_members(members, || {
        accessor_helpers::descriptor_label(endpoint.holon())
    })
}

/// Discovers the canonical SourceType vocabulary from a resolved target lineage.
/// Missing indexes on unsaved definitions are expected; actual graph errors propagate.
pub(crate) fn navigable_occurrences_in_lineage(
    lineage: &[HolonReference],
    mut result: Vec<super::EffectiveContribution>,
) -> Result<Vec<super::EffectiveContribution>, HolonError> {
    let lineage_ids = lineage
        .iter()
        .map(|reference| {
            crate::DefinitionIdentity::for_optional_context(
                reference,
                reference.transaction_context().as_ref(),
            )
        })
        .collect::<Result<HashSet<_>, HolonError>>()?;
    let mut applicability = std::collections::HashMap::new();
    let mut applicable = Vec::new();
    // Applicability is a projection operation, not descriptor validation. Resolve
    // SourceType once per definition while preserving every selected occurrence.
    for occurrence in result.drain(..) {
        let identity = crate::DefinitionIdentity::for_optional_context(
            &occurrence.member,
            occurrence.member.transaction_context().as_ref(),
        )?;
        let applies = match applicability.get(&identity) {
            Some(applies) => *applies,
            None => {
                let source = accessor_helpers::require_single_related(
                    &occurrence.member,
                    CoreRelationshipTypeName::SourceType,
                )?;
                let source_id = crate::DefinitionIdentity::for_optional_context(
                    &source,
                    source.transaction_context().as_ref(),
                )?;
                let applies = lineage_ids.contains(&source_id);
                applicability.insert(identity, applies);
                applies
            }
        };
        if applies {
            applicable.push(occurrence);
        }
    }
    // Rediscovery through SourceOf is not another declaration occurrence.
    let mut discovered = HashSet::new();
    for member in &applicable {
        discovered.insert(crate::DefinitionIdentity::for_optional_context(
            &member.member,
            member.member.transaction_context().as_ref(),
        )?);
    }
    for ancestor in lineage {
        let collection = ancestor.related_holons(CoreRelationshipTypeName::SourceOf)?;
        let candidates =
            collection.read().map_err(accessor_helpers::lock_error)?.get_members().clone();
        for (local_position, candidate) in candidates.into_iter().enumerate() {
            let identity = crate::DefinitionIdentity::for_optional_context(
                &candidate,
                candidate.transaction_context().as_ref(),
            )?;
            if discovered.contains(&identity) {
                continue;
            }
            let applies = match applicability.get(&identity) {
                Some(applies) => *applies,
                None => {
                    let source = accessor_helpers::require_single_related(
                        &candidate,
                        CoreRelationshipTypeName::SourceType,
                    )?;
                    let source_id = crate::DefinitionIdentity::for_optional_context(
                        &source,
                        source.transaction_context().as_ref(),
                    )?;
                    let applies = lineage_ids.contains(&source_id);
                    applicability.insert(identity.clone(), applies);
                    applies
                }
            };
            if applies {
                discovered.insert(identity);
                applicable.push(super::EffectiveContribution {
                    member: candidate,
                    declared_on: ancestor.clone(),
                    source_collection: CoreRelationshipTypeName::SourceOf,
                    local_position,
                });
            }
        }
    }
    let result = applicable;
    Ok(result)
}

fn collect_declared_from_relationship_members(
    members: Vec<crate::descriptors::inheritance::EffectiveRelationshipMember>,
    subject_label: impl Fn() -> String,
) -> Result<Vec<DeclaredRelationshipDescriptor>, HolonError> {
    let mut declared_descriptors = Vec::new();
    let mut seen_declaration_names = HashSet::new();

    for member in members {
        let _declared_on = member.declared_on;
        let relationship_descriptor = RelationshipDescriptor::from_holon(member.member);
        let declaration_name = relationship_descriptor.base_relationship_name()?;
        let declaration_label = declaration_name.to_string();
        if !seen_declaration_names.insert(declaration_label.clone()) {
            return Err(HolonError::DuplicateInheritedDeclaration {
                kind: "relationship".to_string(),
                name: declaration_label,
                descriptor: subject_label(),
            });
        }
        declared_descriptors
            .push(relationship_descriptor.try_into_declared_relationship_descriptor()?);
    }

    Ok(declared_descriptors)
}

/// Enumerates effective inverse relationships for `endpoint`.
///
/// This uses the materialized `SourceOf` index and therefore keeps the staged
/// guard when the endpoint is unsaved and the index is empty.
#[cfg(test)]
pub(crate) fn effective_inverse_relationships(
    endpoint: &HolonDescriptor,
) -> Result<Vec<InverseRelationshipDescriptor>, HolonError> {
    let set = collect_source_index_candidates(endpoint)?;

    if set.requires_materialized_source_index {
        return Err(HolonError::UnsupportedStagedTraversal {
            relationship: CoreRelationshipTypeName::SourceOf.to_relationship_name().to_string(),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    Ok(set.inverse_descriptors)
}

/// Finds the declared relationship that licenses `name` on `source_holon`.
///
/// Candidates come exclusively from the contract that describes the source
/// holon: `EffectiveValues(D(H), InstanceRelationships)`. This applies uniformly
/// to ordinary and descriptor holons. A descriptor holon's own
/// `InstanceRelationships` declarations license its instances, not the
/// descriptor holon itself.
pub fn effective_relationship_declaration(
    source_holon: &HolonReference,
    name: impl ToRelationshipName,
) -> Result<RelationshipDescriptor, HolonError> {
    let descriptor =
        source_holon.effective_descriptor()?.get_declared_relationship_by_name(name)?;
    Ok(RelationshipDescriptor::from_holon(descriptor.holon().clone()))
}

/// Resolves an authored declaration while assembling schema or classifying staged changes.
/// These operations construct preparation requests and must not recursively acquire a product.
/// A published product may be reused, but its absence never initiates construction here.
pub(crate) fn relationship_declaration_for_assembly(
    source_holon: &HolonReference,
    name: impl ToRelationshipName,
) -> Result<RelationshipDescriptor, HolonError> {
    let requested_name = name.to_relationship_name();
    let requested = requested_name.to_string();

    let source_type = source_holon.holon_descriptor()?;
    if let Some(prepared) = super::EffectiveHolonDescriptor::find_prepared(source_type.holon())? {
        let descriptor = prepared.get_declared_relationship_by_name(&requested_name)?;
        return Ok(RelationshipDescriptor::from_holon(descriptor.holon().clone()));
    }
    let declarations = effective_declared_relationships(&source_type)?;
    for descriptor in declarations {
        if descriptor.base_relationship_name()? == requested_name {
            return Ok(RelationshipDescriptor::from_holon(descriptor.holon().clone()));
        }
    }

    Err(HolonError::DescriptorDeclarationNotFound {
        kind: "relationship".to_string(),
        name: requested,
        descriptor: accessor_helpers::descriptor_label(source_holon),
    })
}

/// Binds fetched membership to an exact relationship version in the retrieval guest.
/// Includes the describing contract and descriptor endpoint ancestry. This is not
/// a cache callback: it may read schema, entirely within the existing guest request.
pub(crate) fn bind_relationship_policy(
    source: &HolonReference,
    name: &RelationshipName,
) -> Result<crate::core_shared_objects::relationship_fetch::RelationshipPolicyBinding, HolonError> {
    let source_type = source.holon_descriptor()?;
    let mut candidates = collect_candidates(&source_type)?;
    let parents = source.related_holons(CoreRelationshipTypeName::Extends)?;
    let has_parent = !parents
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .is_empty();
    if has_parent || source.key()? == Some("TypeDescriptor".into()) {
        for ancestor in super::ancestors(source)? {
            let members = ancestor
                .related_holons(CoreRelationshipTypeName::SourceOf)?
                .read()
                .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                .get_members()
                .clone();
            for member in members {
                let descriptor = RelationshipDescriptor::from_holon(member);
                let candidate_name = descriptor.base_relationship_name()?;
                if candidate_name != *name {
                    continue;
                }
                let descriptor_id = descriptor.holon().reference_id_string();
                if candidates.iter().any(|candidate| candidate.descriptor_id == descriptor_id) {
                    continue;
                }
                candidates.push(NavigationCandidate {
                    name: candidate_name,
                    descriptor_id,
                    qualified: QualifiedRelationship {
                        descriptor_direction: descriptor.direction()?,
                        descriptor,
                    },
                });
            }
        }
    }
    let mut matches = candidates.into_iter().filter(|entry| entry.name == *name);
    let matched = matches.next().ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
        kind: "relationship".into(),
        name: name.to_string(),
        descriptor: source_type.holon().full_reference_id_string(),
    })?;
    if matches.next().is_some() {
        return Err(HolonError::AmbiguousRelationshipTraversal {
            relationship: name.to_string(),
            descriptor: source.full_reference_id_string(),
        });
    }
    let descriptor = matched.qualified.descriptor;
    Ok(crate::core_shared_objects::relationship_fetch::RelationshipPolicyBinding {
        descriptor_id: descriptor.holon().holon_id()?,
        policy: descriptor.membership_cache_policy()?,
    })
}

/// Combines the declared contract with both directions of the indexed outbound
/// vocabulary, matching the relationships exposed by prepared navigation.
fn collect_candidates(endpoint: &HolonDescriptor) -> Result<Vec<NavigationCandidate>, HolonError> {
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

    let inverse_set = collect_source_index_candidates(endpoint)?;
    // SourceOf contributes both directions to outbound navigation. A declared
    // relationship indexed on an ancestor can be navigable without appearing in
    // this type's authored InstanceRelationships (for example InstanceKeyRule).
    for declared in inverse_set.declared_descriptors {
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

    Ok(candidates)
}

struct SourceIndexCandidates {
    declared_descriptors: Vec<DeclaredRelationshipDescriptor>,
    inverse_descriptors: Vec<InverseRelationshipDescriptor>,
    #[cfg(test)]
    requires_materialized_source_index: bool,
}

fn collect_source_index_candidates(
    endpoint: &HolonDescriptor,
) -> Result<SourceIndexCandidates, HolonError> {
    let mut declared_descriptors = Vec::new();
    let mut inverse_descriptors = Vec::new();
    let mut seen_inverse_refs = HashSet::new();

    // SourceOf is the materialized inverse of each descriptor's SourceType.
    // Read each source ancestor's local index: SourceOf membership itself is
    // not inherited, but relationships may accept instances of subtypes.
    #[cfg(test)]
    let mut has_index = false;
    for ancestor in crate::descriptors::walk_extends_chain(endpoint.holon()) {
        let members = ancestor?.related_holons(CoreRelationshipTypeName::SourceOf)?;
        let members = members
            .read()
            .map_err(|_| HolonError::InvalidState("SourceOf collection lock poisoned".into()))?
            .get_members()
            .clone();
        #[cfg(test)]
        {
            has_index |= !members.is_empty();
        }
        for member in members {
            let inverse = match InverseRelationshipDescriptor::try_from_holon(member.clone()) {
                Ok(inverse) => inverse,
                Err(error @ HolonError::WrongDescriptorKind { .. }) => {
                    match DeclaredRelationshipDescriptor::try_from_holon(member) {
                        Ok(declared) => {
                            declared_descriptors.push(declared);
                            continue;
                        }
                        Err(_) => return Err(error),
                    }
                }
                Err(error) => return Err(error),
            };
            if seen_inverse_refs.insert(inverse.holon().reference_id_string()) {
                inverse_descriptors.push(inverse);
            }
        }
    }
    Ok(SourceIndexCandidates {
        declared_descriptors,
        inverse_descriptors,
        #[cfg(test)]
        requires_materialized_source_index: !has_index && !endpoint.holon().is_saved(),
    })
}

fn push_unique(candidates: &mut Vec<NavigationCandidate>, candidate: NavigationCandidate) {
    if candidates.iter().any(|existing| existing.descriptor_id == candidate.descriptor_id) {
        return;
    }
    candidates.push(candidate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_declared_relationship_descriptor_holon,
        new_descriptor_holon, new_holon_type_descriptor, new_relationship_descriptor_holon,
    };
    use crate::reference_layer::{HolonReference, TransientReference, WritableHolon};
    use base_types::MapString;
    use core_types::RelationshipName;
    use std::sync::Arc;
    use type_names::{CoreHolonTypeName, CoreRelationshipTypeName};

    #[test]
    fn retrieval_binds_complete_meta_contract_and_own_inherited_inverse() -> Result<(), HolonError>
    {
        use crate::core_shared_objects::SavedHolon;
        use crate::descriptors::test_support::build_context_with_saved_holons;
        use base_types::{BaseValue, MapInteger};
        use core_types::{HolonId, LocalId, PropertyMap, PropertyName};
        use std::collections::HashMap;
        let id = |n| HolonId::Local(LocalId(vec![n; 39]));
        let saved = |n, key: &str, name: &str, definitional: bool| {
            SavedHolon::new(
                LocalId(vec![n; 39]),
                PropertyMap::from([
                    (PropertyName("Key".into()), BaseValue::StringValue(key.into())),
                    (PropertyName("TypeName".into()), BaseValue::StringValue(name.into())),
                    (
                        PropertyName("IsDefinitional".into()),
                        BaseValue::BooleanValue(definitional.into()),
                    ),
                ]),
                None,
                MapInteger(1),
            )
        };
        let context = build_context_with_saved_holons(
            vec![
                saved(1, "Concrete.TypeDescriptor", "Concrete", false),
                saved(2, "Meta.HolonType", "Meta", false),
                saved(3, "ValueType.RelationshipType", "ValueType", true),
                saved(4, "DeclaredRelationshipType.HolonType", "DeclaredRelationshipType", false),
                saved(5, "ValueTypeFor.RelationshipType", "ValueTypeFor", true),
                saved(6, "InverseRelationshipType.HolonType", "InverseRelationshipType", false),
                saved(7, "Parent.TypeDescriptor", "Parent", false),
                saved(8, "Conflicting.RelationshipType", "ValueType", false),
            ],
            HashMap::from([
                ((id(1), relationship_name("DescribedBy")), vec![id(2)]),
                ((id(1), relationship_name("Extends")), vec![id(7)]),
                ((id(2), relationship_name("InstanceRelationships")), vec![id(3)]),
                ((id(3), relationship_name("Extends")), vec![id(4)]),
                // The declaration's endpoint is outside L(D(H)); the explicit contract still applies.
                ((id(3), relationship_name("SourceType")), vec![id(7)]),
                ((id(5), relationship_name("Extends")), vec![id(6)]),
                ((id(7), relationship_name("SourceOf")), vec![id(5)]),
                ((id(8), relationship_name("Extends")), vec![id(6)]),
                ((id(1), relationship_name("ValueType")), vec![id(7)]),
            ]),
        );
        let source = HolonReference::smart_from_id(context.space_read_handle(), id(1));
        let declared = bind_relationship_policy(&source, &relationship_name("ValueType"))?;
        assert_eq!(declared.descriptor_id, id(3));
        assert_eq!(declared.policy, crate::RelationshipCachePolicy::Reuse);
        let inverse = bind_relationship_policy(&source, &relationship_name("ValueTypeFor"))?;
        assert_eq!(inverse.descriptor_id, id(5));
        // IsDefinitional on the inverse never grants immutable membership reuse.
        assert_eq!(inverse.policy, crate::RelationshipCachePolicy::Fresh);
        assert!(matches!(
            bind_relationship_policy(&source, &relationship_name("Missing")),
            Err(HolonError::DescriptorDeclarationNotFound { .. })
        ));
        let mut expression =
            crate::query_layer::QueryExpression::new(relationship_name("ValueType"));
        expression.include_relationship_descriptor = true;
        let request = crate::dances::DanceRequest::new(
            "query_relationships".into(),
            crate::dances::DanceType::QueryMethod(crate::query_layer::NodeCollection {
                members: vec![crate::query_layer::Node::new(source.clone(), None)],
                query_spec: None,
            }),
            crate::dances::RequestBody::QueryExpression(expression),
        );
        let crate::dances::ResponseBody::SavedRelationship(fetched) =
            crate::dances::holon_dance_adapter::query_relationships_dance(&context, request)?
        else {
            panic!("membership and descriptor must travel together");
        };
        assert_eq!(fetched.descriptor.unwrap(), declared);
        assert_eq!(fetched.collection.get_members()[0].holon_id()?, id(7));
        crate::descriptors::test_support::replace_saved_relationship(
            &context,
            id(7),
            relationship_name("SourceOf"),
            vec![id(5), id(8)],
        );
        assert!(matches!(
            bind_relationship_policy(&source, &relationship_name("ValueType")),
            Err(HolonError::AmbiguousRelationshipTraversal { .. })
        ));
        Ok(())
    }

    #[test]
    fn policy_binding_accepts_declared_source_of_navigation_on_the_describing_lineage(
    ) -> Result<(), HolonError> {
        use crate::core_shared_objects::holon::SavedHolon;
        use crate::descriptors::test_support::build_context_with_saved_holons;
        use base_types::{BaseValue, MapInteger};
        use core_types::{HolonId, LocalId, PropertyMap, PropertyName};
        use std::collections::HashMap;
        let id = |n| HolonId::Local(LocalId(vec![n; 39]));
        let saved = |n, key: &str, name: &str| {
            SavedHolon::new(
                LocalId(vec![n; 39]),
                PropertyMap::from([
                    (PropertyName("Key".into()), BaseValue::StringValue(key.into())),
                    (PropertyName("TypeName".into()), BaseValue::StringValue(name.into())),
                    (PropertyName("IsDefinitional".into()), BaseValue::BooleanValue(true.into())),
                ]),
                None,
                MapInteger(1),
            )
        };
        let context = build_context_with_saved_holons(
            vec![
                saved(1, "Book.Example", "Book.Example"),
                saved(2, "Book.HolonType", "Book"),
                saved(3, "InstanceKeyRule.RelationshipType", "InstanceKeyRule"),
                saved(4, "DeclaredRelationshipType.HolonType", "DeclaredRelationshipType"),
                saved(7, "HolonType.TypeDescriptor", "HolonType"),
            ],
            HashMap::from([
                ((id(1), relationship_name("DescribedBy")), vec![id(2)]),
                ((id(2), relationship_name("Extends")), vec![id(7)]),
                ((id(7), relationship_name("SourceOf")), vec![id(3)]),
                ((id(3), relationship_name("Extends")), vec![id(4)]),
                ((id(3), relationship_name("SourceType")), vec![id(7)]),
            ]),
        );
        let source = HolonReference::smart_from_id(context.space_read_handle(), id(1));
        let computed =
            super::super::ComputedEffectiveSchema::resolve(source.holon_descriptor()?.holon())?;
        assert!(computed
            .navigable_relationships()?
            .iter()
            .any(|entry| entry.member.holon_id().is_ok_and(|member| member == id(3))));
        let binding = bind_relationship_policy(&source, &relationship_name("InstanceKeyRule"))?;
        assert_eq!(binding.descriptor_id, id(3));
        assert_eq!(binding.policy, crate::RelationshipCachePolicy::Reuse);
        Ok(())
    }

    struct RelationshipPairFixture {
        source_type: TransientReference,
        target_type: TransientReference,
        declared: TransientReference,
        inverse: TransientReference,
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
        populate_source_of: bool,
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
        if populate_source_of {
            target_type
                .add_related_holons(CoreRelationshipTypeName::SourceOf, vec![(&inverse).into()])?;
        }

        Ok(RelationshipPairFixture { source_type, target_type, declared, inverse })
    }

    #[test]
    fn resolved_navigation_includes_source_index_only_declarations_and_deduplicates_paths(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut fixture = build_relationship_pair(
            &context,
            "resolved-navigation",
            "AuthoredBy",
            "Authors",
            false,
            false,
        )?;
        fixture.source_type.add_related_holons(
            CoreRelationshipTypeName::SourceOf,
            vec![(&fixture.declared).into()],
        )?;
        let source: HolonReference = (&fixture.source_type).into();
        assert_eq!(navigable_occurrences_in_lineage(&[source.clone()], vec![])?.len(), 1);
        let contribution = crate::descriptors::EffectiveContribution {
            member: (&fixture.declared).into(),
            declared_on: source.clone(),
            source_collection: CoreRelationshipTypeName::InstanceRelationships,
            local_position: 0,
        };
        assert_eq!(navigable_occurrences_in_lineage(&[source], vec![contribution])?.len(), 1);
        Ok(())
    }

    #[test]
    fn resolved_navigation_tolerates_missing_unsaved_indexes_and_keeps_available_ancestor_inverses(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "resolved-staged",
            "AuthoredBy",
            "Authors",
            false,
            true,
        )?;
        let mut child = new_holon_type_descriptor(&context, "resolved-child", "Child")?;
        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![(&fixture.target_type).into()],
        )?;
        let child: HolonReference = child.into();
        let lineage = crate::descriptors::ancestors(&child)?;
        let members = navigable_occurrences_in_lineage(&lineage, vec![])?;
        assert_eq!(members.len(), 1);
        assert!(crate::descriptors::same_definition(&members[0].member, &fixture.inverse.into())?);
        assert!(navigable_occurrences_in_lineage(&[child], vec![])?.is_empty());
        Ok(())
    }

    #[test]
    fn effective_inverse_relationships_ignores_declared_descriptors_in_source_of_index(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut fixture = build_relationship_pair(
            &context,
            "mixed-source-of",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;

        // A committed schema index contains relationship descriptors of both
        // directions sourced at this endpoint. Only inverse descriptors
        // contribute to the inverse navigation surface.
        fixture.target_type.add_related_holons(
            CoreRelationshipTypeName::SourceOf,
            vec![HolonReference::from(&fixture.declared)],
        )?;

        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());
        let relationships = descriptor.effective_inverse_relationships()?;
        assert_eq!(relationships.len(), 1);
        assert_eq!(relationships[0].base_relationship_name()?, relationship_name("Authors"));

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

        assert_eq!(names, vec!["AuthoredBy".to_string(), "TaggedWith".to_string()]);
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
    fn finds_ordinary_instance_declaration_through_described_by_lineage() -> Result<(), HolonError>
    {
        let context = build_context();
        let holon_type = new_holon_type_descriptor(&context, "surface-holon-type", "HolonType")?;
        let mut book_type = new_holon_type_descriptor(&context, "surface-book-type", "Book")?;
        let person_type = new_holon_type_descriptor(&context, "surface-person-type", "Person")?;
        let authored_by = new_declared_relationship_descriptor_holon(
            &context,
            "surface-authored-by",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let mut book = crate::descriptors::test_support::new_test_holon(&context, "surface-book")?;

        book_type.add_related_holons(
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
    fn descriptor_own_instance_relationships_do_not_license_the_descriptor_holon(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let type_descriptor =
            new_holon_type_descriptor(&context, "surface-type-descriptor", "TypeDescriptor")?;
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
        let source_type = new_declared_relationship_descriptor_holon(
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

        assert!(matches!(
            effective_relationship_declaration(&(&relationship_descriptor).into(), "SourceType"),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "SourceType"
        ));
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
            new_holon_type_descriptor(&context, "surface-tail-type-descriptor", "TypeDescriptor")?;
        let properties = new_declared_relationship_descriptor_holon(
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
        let authored_by_a = new_declared_relationship_descriptor_holon(
            &context,
            "surface-authored-by-a",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let authored_by_b = new_declared_relationship_descriptor_holon(
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
