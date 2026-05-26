use super::test_support::{
    build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
    new_property_descriptor_holon, new_relationship_descriptor_holon,
};
use crate::descriptors::{
    CommandDescriptor, Descriptor, HolonDescriptor, HolonSpaceDescriptor, RelationshipDescriptor,
    TransactionDescriptor,
};
use crate::reference_layer::{HolonReference, TransientReference, WritableHolon};
use base_types::MapString;
use core_types::{HolonError, PropertyName, RelationshipName};
use type_names::{CommandName, CoreCommandTypeName, CoreHolonTypeName, CoreRelationshipTypeName};

fn assert_descriptor<T: Descriptor>(descriptor: &T) {
    // Compile-time trait membership plus one trivial runtime use.
    let _ = descriptor.holon().reference_id_string();
}

fn command_type(
    context: &std::sync::Arc<crate::core_shared_objects::transactions::TransactionContext>,
    key: &str,
    command_name: CoreCommandTypeName,
) -> Result<TransientReference, HolonError> {
    new_descriptor_holon(context, key, &command_name.as_command_name().to_string(), "Holon")
}

fn command_names(commands: Vec<CommandDescriptor>) -> Result<Vec<CommandName>, HolonError> {
    commands.into_iter().map(|command| command.command_name()).collect()
}

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
        &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        "Relationship",
    )?;
    let inverse_type = new_descriptor_holon(
        &context,
        "inverse-relationship-type",
        &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
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

#[test]
fn holon_space_descriptor_returns_single_transaction_model() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon_space =
        new_holon_type_descriptor(&context, "holon-space-type", "HolonSpaceType")?;
    let transaction_type =
        new_holon_type_descriptor(&context, "transaction-type", "TransactionType")?;

    holon_space.add_related_holons(
        CoreRelationshipTypeName::AffordsTransactionModel,
        vec![HolonReference::from(&transaction_type)],
    )?;

    let holon_space_descriptor = HolonSpaceDescriptor::from_holon(holon_space.into());
    let transaction_descriptor = holon_space_descriptor.transaction_model()?;

    assert_descriptor(&holon_space_descriptor);
    assert_descriptor(&transaction_descriptor);
    assert_eq!(
        transaction_descriptor.header().type_name()?,
        MapString("TransactionType".to_string())
    );

    Ok(())
}

#[test]
fn transaction_model_errors_when_required_relationship_is_missing() -> Result<(), HolonError> {
    let context = build_context();
    let holon_space = new_holon_type_descriptor(&context, "missing-model", "HolonSpaceType")?;
    let holon_space_descriptor = HolonSpaceDescriptor::from_holon(holon_space.into());

    assert!(matches!(
        holon_space_descriptor.transaction_model(),
        Err(HolonError::MissingRequiredRelationship { relationship, .. })
            if relationship == "AffordsTransactionModel"
    ));

    Ok(())
}

#[test]
fn transaction_model_errors_when_multiple_models_are_related() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon_space = new_holon_type_descriptor(&context, "multiple-models", "HolonSpaceType")?;
    let transaction_type_a =
        new_holon_type_descriptor(&context, "transaction-type-a", "TransactionType")?;
    let transaction_type_b =
        new_holon_type_descriptor(&context, "transaction-type-b", "TransactionType")?;

    holon_space.add_related_holons(
        CoreRelationshipTypeName::AffordsTransactionModel,
        vec![transaction_type_a.into(), transaction_type_b.into()],
    )?;

    let holon_space_descriptor = HolonSpaceDescriptor::from_holon(holon_space.into());

    assert!(matches!(
        holon_space_descriptor.transaction_model(),
        Err(HolonError::MultipleRelatedHolons {
            relationship,
            count,
            ..
        }) if relationship == "AffordsTransactionModel" && count == 2
    ));

    Ok(())
}

#[test]
fn transaction_descriptor_afforded_commands_returns_flattened_command_set() -> Result<(), HolonError>
{
    let context = build_context();
    let inherited_command =
        command_type(&context, "inherited-commit-command", CoreCommandTypeName::Commit)?;
    let local_command =
        command_type(&context, "local-staged-count-command", CoreCommandTypeName::GetStagedCount)?;
    let mut parent = new_holon_type_descriptor(&context, "transaction-parent", "ParentType")?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "transaction-command-owner", "TransactionType")?;

    parent.add_related_holons(
        CoreRelationshipTypeName::AffordsCommand,
        vec![inherited_command.into()],
    )?;
    transaction_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
    transaction_type
        .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![local_command.into()])?;

    let transaction_descriptor = TransactionDescriptor::from_holon(transaction_type.into());

    assert_eq!(
        command_names(transaction_descriptor.afforded_commands()?)?,
        vec![
            CommandName(MapString("GetStagedCount".to_string())),
            CommandName(MapString("Commit".to_string())),
        ]
    );

    Ok(())
}

#[test]
fn transaction_descriptor_get_command_by_core_name_resolves_commit() -> Result<(), HolonError> {
    let context = build_context();
    let command = command_type(&context, "commit-command", CoreCommandTypeName::Commit)?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "transaction-with-commit", "TransactionType")?;

    transaction_type
        .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![command.into()])?;

    let transaction_descriptor = TransactionDescriptor::from_holon(transaction_type.into());

    assert_eq!(
        transaction_descriptor.get_command_by_name(CoreCommandTypeName::Commit)?.command_name()?,
        CommandName(MapString("Commit".to_string()))
    );

    Ok(())
}

#[test]
fn transaction_descriptor_get_command_by_string_canonicalizes_name() -> Result<(), HolonError> {
    let context = build_context();
    let command =
        command_type(&context, "get-staged-count-command", CoreCommandTypeName::GetStagedCount)?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "transaction-with-staged-count", "TransactionType")?;

    transaction_type
        .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![command.into()])?;

    let transaction_descriptor = TransactionDescriptor::from_holon(transaction_type.into());

    assert_eq!(
        transaction_descriptor.get_command_by_name("get_staged_count")?.command_name()?,
        CommandName(MapString("GetStagedCount".to_string()))
    );

    Ok(())
}

#[test]
fn transaction_descriptor_get_command_by_name_detects_duplicate_inherited_declarations(
) -> Result<(), HolonError> {
    let context = build_context();
    let inherited_command =
        command_type(&context, "inherited-duplicate-command", CoreCommandTypeName::Commit)?;
    let local_command =
        command_type(&context, "local-duplicate-command", CoreCommandTypeName::Commit)?;
    let mut parent =
        new_holon_type_descriptor(&context, "duplicate-transaction-parent", "ParentType")?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "duplicate-transaction-type", "TransactionType")?;

    parent.add_related_holons(
        CoreRelationshipTypeName::AffordsCommand,
        vec![inherited_command.into()],
    )?;
    transaction_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
    transaction_type
        .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![local_command.into()])?;

    let transaction_descriptor = TransactionDescriptor::from_holon(transaction_type.into());

    assert!(matches!(
        transaction_descriptor.get_command_by_name(CoreCommandTypeName::Commit),
        Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
            if kind == "command" && name == "Commit"
    ));

    Ok(())
}

#[test]
fn transaction_descriptor_get_command_by_name_reports_missing_command() -> Result<(), HolonError> {
    let context = build_context();
    let command = command_type(&context, "query-command", CoreCommandTypeName::Query)?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "transaction-missing-command", "TransactionType")?;

    transaction_type
        .add_related_holons(CoreRelationshipTypeName::AffordsCommand, vec![command.into()])?;

    let transaction_descriptor = TransactionDescriptor::from_holon(transaction_type.into());

    assert!(matches!(
        transaction_descriptor.get_command_by_name(CoreCommandTypeName::Commit),
        Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
            if kind == "command" && name == "Commit"
    ));

    Ok(())
}
