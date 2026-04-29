use super::test_support::{
    build_context, new_descriptor_holon, new_holon_type_descriptor, new_property_descriptor_holon,
    new_relationship_descriptor_holon,
};
use crate::descriptors::{HolonDescriptor, RelationshipDescriptor};
use crate::reference_layer::{HolonReference, WritableHolon};
use base_types::MapString;
use core_types::{HolonError, PropertyName, RelationshipName};
use type_names::CoreRelationshipTypeName;

#[test]
fn descriptor_wrappers_compose_over_minimal_schema_shaped_graph() -> Result<(), HolonError> {
    let context = build_context();

    let value_type =
        new_descriptor_holon(&context, "string-value-type", "StringValueType", "Value")?;
    let property = new_property_descriptor_holon(
        &context,
        "title-property",
        "TitleProperty",
        "Title",
        true,
        HolonReference::from(&value_type),
    )?;
    let mut book = new_holon_type_descriptor(&context, "book-type", "BookType")?;
    let author = new_holon_type_descriptor(&context, "author-type", "AuthorType")?;

    let declared_type = new_descriptor_holon(
        &context,
        "declared-relationship-type",
        "DeclaredRelationshipType",
        "Relationship",
    )?;
    let inverse_type = new_descriptor_holon(
        &context,
        "inverse-relationship-type",
        "InverseRelationshipType",
        "Relationship",
    )?;
    let mut authored_by = new_relationship_descriptor_holon(
        &context,
        "authored-by",
        "AuthoredBy",
        HolonReference::from(&book),
        HolonReference::from(&author),
    )?;
    authored_by.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&declared_type)],
    )?;
    let mut books_authored = new_relationship_descriptor_holon(
        &context,
        "books-authored",
        "BooksAuthored",
        HolonReference::from(&author),
        HolonReference::from(&book),
    )?;
    books_authored.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&inverse_type)],
    )?;
    books_authored.add_related_holons(
        CoreRelationshipTypeName::InverseOf,
        vec![HolonReference::from(&authored_by)],
    )?;
    authored_by.add_related_holons(
        CoreRelationshipTypeName::HasInverse,
        vec![HolonReference::from(&books_authored)],
    )?;

    book.add_related_holons(
        CoreRelationshipTypeName::InstanceProperties,
        vec![HolonReference::from(&property)],
    )?;
    book.add_related_holons(
        CoreRelationshipTypeName::Properties,
        vec![HolonReference::from(&property)],
    )?;
    book.add_related_holons(
        CoreRelationshipTypeName::InstanceRelationships,
        vec![HolonReference::from(&authored_by)],
    )?;

    let book_descriptor = HolonDescriptor::from_holon(book.into());
    let title_property = book_descriptor
        .get_property_by_name(PropertyName(MapString("TitleProperty".to_string())))?;
    let declared_relationship = book_descriptor
        .get_relationship_by_name(RelationshipName(MapString("AuthoredBy".to_string())))?
        .try_into_declared_relationship_descriptor()?;
    let inverse_relationship = declared_relationship.has_inverse()?.expect("inverse should exist");
    let inverse_of = inverse_relationship.inverse_of()?;

    assert_eq!(book_descriptor.instance_properties()?.len(), 1);
    assert_eq!(book_descriptor.properties()?.len(), 1);
    assert_eq!(title_property.property_name()?.to_string(), "Title");
    assert_eq!(
        title_property.value_type()?.header().type_name()?,
        MapString("StringValueType".to_string())
    );
    assert_eq!(
        declared_relationship.full_relationship_name()?,
        MapString("(BookType)-[AuthoredBy]->(AuthorType)".to_string())
    );
    assert_eq!(
        inverse_relationship.full_relationship_name()?,
        MapString("(AuthorType)-[BooksAuthored]->(BookType)".to_string())
    );
    assert_eq!(inverse_of.header().type_name()?, MapString("AuthoredBy".to_string()));
    assert_eq!(
        RelationshipDescriptor::from_holon(HolonReference::from(&books_authored))
            .try_into_inverse_relationship_descriptor()?
            .header()
            .type_name()?,
        MapString("BooksAuthored".to_string())
    );

    Ok(())
}
