use super::test_support::{
    build_context, core_holon_type_name, new_descriptor_holon, new_holon_type_descriptor,
    new_property_descriptor_holon, new_relationship_descriptor_holon,
};
use crate::descriptors::{
    CommandDescriptor, DanceDescriptor, Descriptor, HolonDescriptor, HolonSpaceDescriptor,
    RelationshipDescriptor, TransactionDescriptor,
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

fn dance_names(dances: Vec<DanceDescriptor>) -> Result<Vec<type_names::DanceName>, HolonError> {
    dances.into_iter().map(|dance| dance.dance_name()).collect()
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
    let inverse_relationship = declared_relationship.required_inverse()?;

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
fn declared_relationship_requires_exactly_one_has_inverse_target() -> Result<(), HolonError> {
    let context = build_context();
    let declared_type = new_descriptor_holon(
        &context,
        "declared-relationship-type-required-inverse",
        &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        "Relationship",
    )?;
    let inverse_type = new_descriptor_holon(
        &context,
        "inverse-relationship-type-required-inverse",
        &core_holon_type_name(CoreHolonTypeName::InverseRelationshipType),
        "Relationship",
    )?;
    let book = new_holon_type_descriptor(&context, "book-type-required-inverse", "BookType")?;
    let author = new_holon_type_descriptor(&context, "author-type-required-inverse", "AuthorType")?;

    let mut authored_by = new_relationship_descriptor_holon(
        &context,
        "authored-by-missing-inverse",
        "AuthoredBy",
        HolonReference::from(&book),
        HolonReference::from(&author),
    )?;
    authored_by.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&declared_type)],
    )?;
    let descriptor = RelationshipDescriptor::from_holon(HolonReference::from(&authored_by))
        .try_into_declared_relationship_descriptor()?;

    assert!(matches!(
        descriptor.required_inverse(),
        Err(HolonError::MissingRequiredRelationship { relationship, .. })
            if relationship == "HasInverse"
    ));

    let mut inverse_a = new_relationship_descriptor_holon(
        &context,
        "books-authored-a",
        "BooksAuthoredA",
        HolonReference::from(&author),
        HolonReference::from(&book),
    )?;
    inverse_a.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&inverse_type)],
    )?;
    let mut inverse_b = new_relationship_descriptor_holon(
        &context,
        "books-authored-b",
        "BooksAuthoredB",
        HolonReference::from(&author),
        HolonReference::from(&book),
    )?;
    inverse_b.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&inverse_type)],
    )?;
    let mut authored_by_multiple = new_relationship_descriptor_holon(
        &context,
        "authored-by-multiple-inverses",
        "AuthoredBy",
        HolonReference::from(&book),
        HolonReference::from(&author),
    )?;
    authored_by_multiple.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&declared_type)],
    )?;
    authored_by_multiple.add_related_holons(
        CoreRelationshipTypeName::HasInverse,
        vec![HolonReference::from(&inverse_a), HolonReference::from(&inverse_b)],
    )?;
    let descriptor =
        RelationshipDescriptor::from_holon(HolonReference::from(&authored_by_multiple))
            .try_into_declared_relationship_descriptor()?;

    assert!(matches!(
        descriptor.required_inverse(),
        Err(HolonError::MultipleRelatedHolons { relationship, count, .. })
            if relationship == "HasInverse" && count == 2
    ));

    Ok(())
}

#[test]
fn declared_relationship_has_inverse_target_must_be_inverse_relationship_type(
) -> Result<(), HolonError> {
    let context = build_context();
    let declared_type = new_descriptor_holon(
        &context,
        "declared-relationship-type-inverse-kind",
        &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        "Relationship",
    )?;
    let book = new_holon_type_descriptor(&context, "book-type-inverse-kind", "BookType")?;
    let author = new_holon_type_descriptor(&context, "author-type-inverse-kind", "AuthorType")?;

    let mut wrong_kind_target = new_relationship_descriptor_holon(
        &context,
        "wrong-kind-has-inverse-target",
        "WrongKindTarget",
        HolonReference::from(&author),
        HolonReference::from(&book),
    )?;
    wrong_kind_target.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&declared_type)],
    )?;

    let mut authored_by = new_relationship_descriptor_holon(
        &context,
        "authored-by-wrong-inverse-kind",
        "AuthoredBy",
        HolonReference::from(&book),
        HolonReference::from(&author),
    )?;
    authored_by.add_related_holons(
        CoreRelationshipTypeName::Extends,
        vec![HolonReference::from(&declared_type)],
    )?;
    authored_by.add_related_holons(
        CoreRelationshipTypeName::HasInverse,
        vec![HolonReference::from(&wrong_kind_target)],
    )?;

    let descriptor = RelationshipDescriptor::from_holon(HolonReference::from(&authored_by))
        .try_into_declared_relationship_descriptor()?;

    assert!(matches!(
        descriptor.required_inverse(),
        Err(HolonError::WrongDescriptorKind { expected, found, .. })
            if expected == core_holon_type_name(CoreHolonTypeName::InverseRelationshipType)
                && found == "WrongKindTarget"
    ));

    Ok(())
}

#[test]
fn holon_space_descriptor_returns_single_transaction_model() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon_space = new_holon_type_descriptor(&context, "holon-space-type", "HolonSpace")?;
    let transaction_type = new_holon_type_descriptor(&context, "transaction-type", "Transaction")?;

    holon_space.add_related_holons(
        CoreRelationshipTypeName::AffordsTransactionModel,
        vec![HolonReference::from(&transaction_type)],
    )?;

    let holon_space_descriptor = HolonSpaceDescriptor::from_holon(holon_space.into());
    let transaction_descriptor = holon_space_descriptor.transaction_model()?;

    assert_descriptor(&holon_space_descriptor);
    assert_descriptor(&transaction_descriptor);
    assert_eq!(transaction_descriptor.header().type_name()?, MapString("Transaction".to_string()));

    Ok(())
}

#[test]
fn transaction_model_errors_when_required_relationship_is_missing() -> Result<(), HolonError> {
    let context = build_context();
    let holon_space = new_holon_type_descriptor(&context, "missing-model", "HolonSpace")?;
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
    let mut holon_space = new_holon_type_descriptor(&context, "multiple-models", "HolonSpace")?;
    let transaction_type_a =
        new_holon_type_descriptor(&context, "transaction-type-a", "Transaction")?;
    let transaction_type_b =
        new_holon_type_descriptor(&context, "transaction-type-b", "Transaction")?;

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
        new_holon_type_descriptor(&context, "transaction-command-owner", "Transaction")?;

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
        new_holon_type_descriptor(&context, "transaction-with-commit", "Transaction")?;

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
        new_holon_type_descriptor(&context, "transaction-with-staged-count", "Transaction")?;

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
        new_holon_type_descriptor(&context, "duplicate-transaction-type", "Transaction")?;

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
    let command =
        command_type(&context, "get-staged-count-command", CoreCommandTypeName::GetStagedCount)?;
    let mut transaction_type =
        new_holon_type_descriptor(&context, "transaction-missing-command", "Transaction")?;

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

#[test]
fn dance_descriptor_afforded_dances_return_flattened_dance_set() -> Result<(), HolonError> {
    let context = build_context();
    let inherited_dance =
        new_descriptor_holon(&context, "inherited-query-dance", "Query", "Holon")?;
    let local_dance = new_descriptor_holon(&context, "local-dance", "Dance", "Holon")?;
    let mut parent = new_holon_type_descriptor(&context, "dance-parent", "ParentType")?;
    let mut dance_type = new_holon_type_descriptor(&context, "dance-owner", "DanceType")?;

    parent.add_related_holons(CoreRelationshipTypeName::Affords, vec![inherited_dance.into()])?;
    dance_type.add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.into()])?;
    dance_type.add_related_holons(CoreRelationshipTypeName::Affords, vec![local_dance.into()])?;

    let dance_descriptor = HolonDescriptor::from_holon(dance_type.into());

    assert_eq!(
        dance_names(dance_descriptor.afforded_dances()?)?,
        vec![
            type_names::DanceName(MapString("Dance".to_string())),
            type_names::DanceName(MapString("Query".to_string())),
        ]
    );

    Ok(())
}

#[test]
fn dance_descriptor_get_dance_by_name_resolves_type_name() -> Result<(), HolonError> {
    let context = build_context();
    let dance = new_descriptor_holon(&context, "commit-dance", "Commit", "Holon")?;
    let mut dance_type = new_holon_type_descriptor(&context, "dance-with-commit", "DanceType")?;

    dance_type.add_related_holons(CoreRelationshipTypeName::Affords, vec![dance.into()])?;

    let dance_descriptor = HolonDescriptor::from_holon(dance_type.into());

    assert_eq!(
        dance_descriptor.get_dance_by_name("commit")?.dance_name()?,
        type_names::DanceName(MapString("Commit".to_string()))
    );

    Ok(())
}

#[test]
fn dance_descriptor_get_dance_by_name_errors_when_duplicate_inherited_declarations_exist(
) -> Result<(), HolonError> {
    let context = build_context();
    let duplicate_root = new_descriptor_holon(&context, "duplicate-root-dance", "Query", "Holon")?;
    let duplicate_leaf = new_descriptor_holon(&context, "duplicate-leaf-dance", "Query", "Holon")?;
    let mut root = new_holon_type_descriptor(&context, "duplicate-dance-root", "ParentType")?;
    let mut leaf = new_holon_type_descriptor(&context, "duplicate-dance-leaf", "DanceType")?;

    root.add_related_holons(CoreRelationshipTypeName::Affords, vec![duplicate_root.into()])?;
    leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
    leaf.add_related_holons(CoreRelationshipTypeName::Affords, vec![duplicate_leaf.into()])?;

    let dance_descriptor = HolonDescriptor::from_holon(leaf.into());

    assert!(matches!(
        dance_descriptor.get_dance_by_name("Query"),
        Err(HolonError::DuplicateInheritedDeclaration { kind, name, .. })
            if kind == "dance" && name == "Query"
    ));

    Ok(())
}
