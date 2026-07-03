use std::collections::HashSet;

use crate::descriptors::{accessor_helpers, effective_descriptor_lineage, RelationshipDescriptor};
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::HolonError;
use type_names::{CoreRelationshipTypeName, ToRelationshipName};

/// Finds a relationship declaration on a holon's effective relationship surface.
///
/// Ordinary runtime holons draw this surface from `DescribedBy -> Extends*`.
/// Descriptor holons also contribute their own `Extends*` lineage, which is
/// where descriptor-populated relationships like `SourceType`, `TargetType`,
/// `InverseOf`, and `ValueType` are licensed in MAP Type System v1.2.
pub fn effective_relationship_declaration(
    source_holon: &HolonReference,
    name: impl ToRelationshipName,
) -> Result<RelationshipDescriptor, HolonError> {
    let requested_name = name.to_relationship_name();
    let requested = requested_name.to_string();
    let mut seen_declaration_names = HashSet::new();
    let mut seen_declaration_refs = HashSet::new();
    let mut found = None;

    for descriptor in effective_descriptor_lineage(source_holon)? {
        let collection_arc =
            descriptor.related_holons(CoreRelationshipTypeName::InstanceRelationships)?;
        let collection = collection_arc.read().map_err(accessor_helpers::lock_error)?;

        for declaration_ref in collection.get_members() {
            // Repeated references to the same declaration are inherited only once;
            // distinct declarations with the same base name (type_name) remain schema errors.
            if !seen_declaration_refs.insert(declaration_ref.reference_id_string()) {
                continue;
            }

            let relationship_descriptor =
                RelationshipDescriptor::from_holon(declaration_ref.clone());
            let declaration_name = relationship_descriptor.base_relationship_name()?;
            let declaration_label = declaration_name.to_string();
            if !seen_declaration_names.insert(declaration_label.clone()) {
                return Err(HolonError::DuplicateInheritedDeclaration {
                    kind: "relationship".to_string(),
                    name: declaration_label,
                    descriptor: accessor_helpers::descriptor_label(source_holon),
                });
            }

            if declaration_name == requested_name {
                found = Some(relationship_descriptor);
            }
        }
    }

    found.ok_or_else(|| HolonError::DescriptorDeclarationNotFound {
        kind: "relationship".to_string(),
        name: requested,
        descriptor: accessor_helpers::descriptor_label(source_holon),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
        new_relationship_descriptor_holon, new_test_holon,
    };
    use crate::reference_layer::WritableHolon;
    use base_types::MapString;
    use core_types::RelationshipName;
    use type_names::CoreHolonTypeName;

    #[test]
    fn finds_ordinary_instance_declaration_through_described_by_lineage() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut holon_type = new_holon_type_descriptor(&context, "holon-type", "HolonType")?;
        let mut book_type = new_holon_type_descriptor(&context, "book-type", "Book")?;
        let person_type = new_holon_type_descriptor(&context, "person-type", "Person")?;
        let authored_by = new_relationship_descriptor_holon(
            &context,
            "authored-by",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let mut book = new_test_holon(&context, "book")?;

        holon_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&authored_by).into()],
        )?;
        book_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![holon_type.into()])?;
        book.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![book_type.into()])?;

        let declaration = effective_relationship_declaration(
            &(&book).into(),
            RelationshipName(MapString("AuthoredBy".to_string())),
        )?;

        assert_eq!(declaration.base_relationship_name()?.to_string(), "AuthoredBy");
        Ok(())
    }

    #[test]
    fn finds_descriptor_holon_declaration_through_own_extends_lineage() -> Result<(), HolonError> {
        let context = build_context();
        let type_descriptor =
            new_holon_type_descriptor(&context, "type-descriptor", "TypeDescriptor")?;
        let mut meta_relationship_type =
            new_holon_type_descriptor(&context, "meta-relationship-type", "MetaRelationshipType")?;
        let mut declared_relationship_type = new_descriptor_holon(
            &context,
            "declared-relationship-type",
            &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
            "Relationship",
        )?;
        let source_type = new_relationship_descriptor_holon(
            &context,
            "source-type",
            "SourceType",
            (&meta_relationship_type).into(),
            (&type_descriptor).into(),
        )?;
        let target_descriptor = new_holon_type_descriptor(&context, "target-type", "Target")?;
        let mut relationship_descriptor = new_relationship_descriptor_holon(
            &context,
            "affords-operator",
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
        let mut meta_type_descriptor =
            new_holon_type_descriptor(&context, "meta-type-descriptor", "MetaTypeDescriptor")?;
        let mut meta_holon_type =
            new_holon_type_descriptor(&context, "meta-holon-type", "MetaHolonType")?;
        let mut holon_type = new_holon_type_descriptor(&context, "holon-type", "HolonType")?;
        let mut type_descriptor =
            new_holon_type_descriptor(&context, "type-descriptor", "TypeDescriptor")?;
        let properties = new_relationship_descriptor_holon(
            &context,
            "properties",
            "Properties",
            (&meta_type_descriptor).into(),
            (&meta_type_descriptor).into(),
        )?;
        let mut descriptor_holon =
            new_holon_type_descriptor(&context, "custom-descriptor", "CustomDescriptor")?;

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
        let mut parent_type = new_holon_type_descriptor(&context, "parent-type", "Parent")?;
        let mut book_type = new_holon_type_descriptor(&context, "book-type", "Book")?;
        let person_type = new_holon_type_descriptor(&context, "person-type", "Person")?;
        let authored_by_a = new_relationship_descriptor_holon(
            &context,
            "authored-by-a",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let authored_by_b = new_relationship_descriptor_holon(
            &context,
            "authored-by-b",
            "AuthoredBy",
            (&book_type).into(),
            (&person_type).into(),
        )?;
        let mut book = new_test_holon(&context, "book")?;

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
