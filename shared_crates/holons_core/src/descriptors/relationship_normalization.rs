use crate::descriptors::{
    accessor_helpers, inheritance::equals_or_extends, inheritance::flatten_related_members,
    DeclaredRelationshipDescriptor, Descriptor, HolonDescriptor, QualifiedRelationship,
    RelationshipDescriptor, RelationshipDirection, TraversalDirection,
};
use crate::reference_layer::HolonReference;
use core_types::{HolonError, RelationshipName};
use type_names::CoreRelationshipTypeName;

struct TraversalMatch {
    declared_ref: HolonReference,
    requested_ref: HolonReference,
    descriptor_direction: RelationshipDirection,
}

pub(crate) fn allows_relationship(
    endpoint: &HolonDescriptor,
    relationship_name: RelationshipName,
    direction: TraversalDirection,
) -> Result<QualifiedRelationship, HolonError> {
    let resolved = resolve_unique(endpoint, relationship_name, direction)?;

    Ok(QualifiedRelationship {
        descriptor: RelationshipDescriptor::from_holon(resolved.requested_ref),
        descriptor_direction: resolved.descriptor_direction,
        traversal_direction: direction,
    })
}

pub(crate) fn normalize_relationship(
    endpoint: &HolonDescriptor,
    relationship_name: RelationshipName,
    direction: TraversalDirection,
) -> Result<DeclaredRelationshipDescriptor, HolonError> {
    let resolved = resolve_unique(endpoint, relationship_name, direction)?;
    DeclaredRelationshipDescriptor::try_from_holon(resolved.declared_ref)
}

fn resolve_unique(
    endpoint: &HolonDescriptor,
    relationship_name: RelationshipName,
    direction: TraversalDirection,
) -> Result<TraversalMatch, HolonError> {
    let mut matches = Vec::new();
    let mut name_matched_wrong_form = false;
    let mut missing_inverse_for_inverse_traversal = None;

    collect_source_owned_matches(
        endpoint,
        &relationship_name,
        direction,
        &mut matches,
        &mut name_matched_wrong_form,
        &mut missing_inverse_for_inverse_traversal,
    )?;

    let target_status = collect_target_owned_matches(endpoint, &relationship_name, direction)?;
    matches.extend(target_status.matches);
    name_matched_wrong_form |= target_status.name_matched_wrong_form;
    if missing_inverse_for_inverse_traversal.is_none() {
        missing_inverse_for_inverse_traversal = target_status.missing_inverse_for_inverse_traversal;
    }

    let mut distinct_declared = Vec::<TraversalMatch>::new();
    for candidate in matches {
        if distinct_declared.iter().any(|existing| {
            existing.declared_ref.reference_id_string()
                == candidate.declared_ref.reference_id_string()
        }) {
            continue;
        }
        distinct_declared.push(candidate);
    }

    if distinct_declared.len() == 1 {
        return Ok(distinct_declared.remove(0));
    }

    if distinct_declared.len() > 1 {
        return Err(HolonError::AmbiguousRelationshipTraversal {
            relationship: relationship_name.to_string(),
            direction: traversal_direction_label(direction),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    if name_matched_wrong_form {
        return Err(HolonError::IllegalRelationshipTraversal {
            relationship: relationship_name.to_string(),
            direction: traversal_direction_label(direction),
            descriptor: accessor_helpers::descriptor_label(endpoint.holon()),
        });
    }

    if let Some(declared_ref) = missing_inverse_for_inverse_traversal {
        return match DeclaredRelationshipDescriptor::try_from_holon(declared_ref)?
            .required_inverse()
        {
            Ok(_) => unreachable!("missing inverse marker should not resolve"),
            Err(error) => Err(error),
        };
    }

    if target_status.requires_materialized_target_index {
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

fn collect_source_owned_matches(
    endpoint: &HolonDescriptor,
    relationship_name: &RelationshipName,
    direction: TraversalDirection,
    matches: &mut Vec<TraversalMatch>,
    name_matched_wrong_form: &mut bool,
    missing_inverse_for_inverse_traversal: &mut Option<crate::reference_layer::HolonReference>,
) -> Result<(), HolonError> {
    for descriptor in endpoint.instance_relationships()? {
        let declared = descriptor.try_into_declared_relationship_descriptor()?;
        let declared_name = declared.base_relationship_name()?;
        let inverse = declared.has_inverse()?;
        let inverse_name =
            inverse.as_ref().map(|descriptor| descriptor.base_relationship_name()).transpose()?;

        match direction {
            TraversalDirection::Outbound if declared_name == *relationship_name => {
                matches.push(TraversalMatch {
                    declared_ref: declared.holon().clone(),
                    requested_ref: declared.holon().clone(),
                    descriptor_direction: RelationshipDirection::Declared,
                });
            }
            TraversalDirection::Inbound if inverse_name.as_ref() == Some(relationship_name) => {
                let inverse = declared.required_inverse()?;
                matches.push(TraversalMatch {
                    declared_ref: declared.holon().clone(),
                    requested_ref: inverse.holon().clone(),
                    descriptor_direction: RelationshipDirection::Inverse,
                });
            }
            TraversalDirection::Inbound if declared_name == *relationship_name => {
                *name_matched_wrong_form = true;
            }
            TraversalDirection::Outbound if inverse_name.as_ref() == Some(relationship_name) => {
                *name_matched_wrong_form = true;
            }
            TraversalDirection::Inbound if inverse.is_none() => {
                if missing_inverse_for_inverse_traversal.is_none() {
                    *missing_inverse_for_inverse_traversal = Some(declared.holon().clone());
                }
            }
            _ => {}
        }
    }

    Ok(())
}

struct TargetTraversalStatus {
    matches: Vec<TraversalMatch>,
    name_matched_wrong_form: bool,
    missing_inverse_for_inverse_traversal: Option<crate::reference_layer::HolonReference>,
    requires_materialized_target_index: bool,
}

fn collect_target_owned_matches(
    endpoint: &HolonDescriptor,
    relationship_name: &RelationshipName,
    direction: TraversalDirection,
) -> Result<TargetTraversalStatus, HolonError> {
    let target_of_members =
        flatten_related_members(endpoint.holon(), CoreRelationshipTypeName::TargetOf)?;
    let mut matches = Vec::new();
    let mut name_matched_wrong_form = false;
    let mut missing_inverse_for_inverse_traversal = None;

    for member in &target_of_members {
        let declared = DeclaredRelationshipDescriptor::try_from_holon(member.clone())?;
        if !target_endpoint_is_compatible(endpoint, &declared)? {
            continue;
        }
        if !source_licenses_declared_relationship(&declared)? {
            continue;
        }

        let declared_name = declared.base_relationship_name()?;
        let inverse = declared.has_inverse()?;
        let inverse_name =
            inverse.as_ref().map(|descriptor| descriptor.base_relationship_name()).transpose()?;

        match direction {
            TraversalDirection::Inbound if declared_name == *relationship_name => {
                matches.push(TraversalMatch {
                    declared_ref: declared.holon().clone(),
                    requested_ref: declared.holon().clone(),
                    descriptor_direction: RelationshipDirection::Declared,
                });
            }
            TraversalDirection::Outbound if inverse_name.as_ref() == Some(relationship_name) => {
                let inverse = declared.required_inverse()?;
                matches.push(TraversalMatch {
                    declared_ref: declared.holon().clone(),
                    requested_ref: inverse.holon().clone(),
                    descriptor_direction: RelationshipDirection::Inverse,
                });
            }
            TraversalDirection::Outbound if declared_name == *relationship_name => {
                name_matched_wrong_form = true;
            }
            TraversalDirection::Inbound if inverse_name.as_ref() == Some(relationship_name) => {
                name_matched_wrong_form = true;
            }
            TraversalDirection::Outbound if inverse.is_none() => {
                if missing_inverse_for_inverse_traversal.is_none() {
                    missing_inverse_for_inverse_traversal = Some(declared.holon().clone());
                }
            }
            _ => {}
        }
    }

    Ok(TargetTraversalStatus {
        matches,
        name_matched_wrong_form,
        missing_inverse_for_inverse_traversal,
        // A staged/transient endpoint cannot rely on the materialized `TargetOf`
        // inverse index, so an empty index there means "target-owned traversal is
        // not answerable yet" rather than "no such relationship". This deliberately
        // absorbs genuine not-found cases for unsaved endpoints (e.g. a typo on an
        // outbound source request that also finds nothing target-owned): they
        // surface as `UnsupportedStagedTraversal` instead of
        // `DescriptorDeclarationNotFound`. A saved endpoint has a trustworthy index,
        // so an empty result there is a true not-found.
        requires_materialized_target_index: target_of_members.is_empty()
            && !endpoint.holon().is_saved(),
    })
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
    let licensed = match source_type.get_relationship_by_name(declared_name) {
        Ok(licensed) => licensed,
        Err(HolonError::DescriptorDeclarationNotFound { .. }) => return Ok(false),
        Err(error) => return Err(error),
    };

    Ok(licensed.holon().reference_id_string() == declared.holon().reference_id_string())
}

fn traversal_direction_label(direction: TraversalDirection) -> String {
    match direction {
        TraversalDirection::Inbound => "Inbound",
        TraversalDirection::Outbound => "Outbound",
    }
    .to_string()
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

    fn assert_declared_name(
        descriptor: DeclaredRelationshipDescriptor,
        expected: &str,
    ) -> Result<(), HolonError> {
        assert_eq!(descriptor.base_relationship_name()?, relationship_name(expected));
        Ok(())
    }

    #[test]
    fn normalizes_source_owned_declared_outbound() -> Result<(), HolonError> {
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

        assert_declared_name(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Outbound)?,
            "AuthoredBy",
        )?;
        let qualified =
            descriptor.allows_relationship("authored_by", TraversalDirection::Outbound)?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Declared);
        assert_eq!(qualified.traversal_direction, TraversalDirection::Outbound);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("AuthoredBy"));

        Ok(())
    }

    #[test]
    fn normalizes_source_owned_inverse_inbound_and_qualifies_requested_descriptor(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "source-inverse",
            "AuthoredBy",
            "Authors",
            true,
            false,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.source_type.into());

        assert_declared_name(
            descriptor.normalize_relationship("authors", TraversalDirection::Inbound)?,
            "AuthoredBy",
        )?;
        let qualified = descriptor.allows_relationship("authors", TraversalDirection::Inbound)?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Inverse);
        assert_eq!(qualified.traversal_direction, TraversalDirection::Inbound);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("Authors"));

        Ok(())
    }

    #[test]
    fn inverse_traversal_missing_has_inverse_fails_clearly() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = new_descriptor_holon(
            &context,
            "missing-inverse-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let mut source_type =
            new_holon_type_descriptor(&context, "missing-inverse-source", "BookType")?;
        let target_type =
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
            vec![declared.into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(source_type.into());

        assert!(matches!(
            descriptor.normalize_relationship("authors", TraversalDirection::Inbound),
            Err(HolonError::MissingRequiredRelationship { relationship, .. })
                if relationship == "HasInverse"
        ));

        Ok(())
    }

    #[test]
    fn normalizes_target_owned_declared_inbound() -> Result<(), HolonError> {
        let context = build_context();
        let fixture = build_relationship_pair(
            &context,
            "target-declared",
            "AuthoredBy",
            "Authors",
            true,
            true,
        )?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        assert_declared_name(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Inbound)?,
            "AuthoredBy",
        )?;
        let qualified =
            descriptor.allows_relationship("authored_by", TraversalDirection::Inbound)?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Declared);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("AuthoredBy"));

        Ok(())
    }

    #[test]
    fn normalizes_target_owned_inverse_outbound() -> Result<(), HolonError> {
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

        assert_declared_name(
            descriptor.normalize_relationship("authors", TraversalDirection::Outbound)?,
            "AuthoredBy",
        )?;
        let qualified = descriptor.allows_relationship("authors", TraversalDirection::Outbound)?;
        assert_eq!(qualified.descriptor_direction, RelationshipDirection::Inverse);
        assert_eq!(qualified.descriptor.base_relationship_name()?, relationship_name("Authors"));

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

        assert_declared_name(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Outbound)?,
            "AuthoredBy",
        )?;

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

        assert_declared_name(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Inbound)?,
            "AuthoredBy",
        )?;

        Ok(())
    }

    #[test]
    fn target_of_candidate_without_source_license_is_rejected() -> Result<(), HolonError> {
        let context = build_context();
        let fixture =
            build_relationship_pair(&context, "unlicensed", "AuthoredBy", "Authors", false, true)?;
        let descriptor = HolonDescriptor::from_holon(fixture.target_type.into());

        assert!(matches!(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Inbound),
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "AuthoredBy"
        ));

        Ok(())
    }

    #[test]
    fn ambiguous_distinct_target_matches_fail() -> Result<(), HolonError> {
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
            descriptor.normalize_relationship("authors", TraversalDirection::Outbound),
            Err(HolonError::AmbiguousRelationshipTraversal { relationship, direction, .. })
                if relationship == "Authors" && direction == "Outbound"
        ));

        Ok(())
    }

    #[test]
    fn illegal_direction_for_known_surface_form_fails_clearly() -> Result<(), HolonError> {
        let context = build_context();
        let fixture =
            build_relationship_pair(&context, "illegal-form", "AuthoredBy", "Authors", true, true)?;
        let descriptor = HolonDescriptor::from_holon(fixture.source_type.into());

        assert!(matches!(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Inbound),
            Err(HolonError::IllegalRelationshipTraversal { relationship, direction, .. })
                if relationship == "AuthoredBy" && direction == "Inbound"
        ));

        Ok(())
    }

    #[test]
    fn staged_source_owned_normalization_uses_forward_relationships() -> Result<(), HolonError> {
        let context = build_context();
        let declared_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            "staged-source-declared-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?)?;
        let inverse_type = context.mutation().stage_new_holon(new_descriptor_holon(
            &context,
            "staged-source-inverse-type",
            &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
            "Relationship",
        )?)?;
        let mut staged_source = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "staged-source-type",
            "BookType",
        )?)?;
        let target_type = context.mutation().stage_new_holon(new_holon_type_descriptor(
            &context,
            "staged-target-type",
            "PersonType",
        )?)?;
        let declared_transient = new_relationship_descriptor_holon(
            &context,
            "staged-authored-by",
            "AuthoredBy",
            HolonReference::from(&staged_source),
            HolonReference::from(&target_type),
        )?;
        let inverse_transient = new_relationship_descriptor_holon(
            &context,
            "staged-authors",
            "Authors",
            HolonReference::from(&target_type),
            HolonReference::from(&staged_source),
        )?;
        let mut declared = context.mutation().stage_new_holon(declared_transient)?;
        let mut inverse = context.mutation().stage_new_holon(inverse_transient)?;

        declared
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&declared_type).into()])?;
        inverse
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![(&inverse_type).into()])?;
        declared
            .add_related_holons(CoreRelationshipTypeName::HasInverse, vec![(&inverse).into()])?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&declared).into()],
        )?;

        let descriptor = HolonDescriptor::from_holon(staged_source.into());

        assert_declared_name(
            descriptor.normalize_relationship("authored_by", TraversalDirection::Outbound)?,
            "AuthoredBy",
        )?;

        Ok(())
    }

    #[test]
    fn staged_target_owned_without_target_of_is_rejected() -> Result<(), HolonError> {
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
            descriptor.normalize_relationship("authored_by", TraversalDirection::Inbound),
            Err(HolonError::UnsupportedStagedTraversal { relationship, .. })
                if relationship == "AuthoredBy"
        ));

        Ok(())
    }
}
