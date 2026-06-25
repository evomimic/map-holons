//! Schema contract verification for core command descriptors and command affordances.
//!
//! This executor is not a command-routing or command-execution test. It loads the
//! current holon collection through an assertion transaction, selects known schema
//! descriptors by key, verifies the stable concrete `CommandType` inventory,
//! checks the `AffordsCommand` / `AffordedBy` descriptor graph, verifies the
//! `HolonSpace` -> `Transaction` transaction-model anchor, and exercises
//! typed command-name lookup through `CoreCommandTypeName` and `CommandName`.
//!
//! The goal is to catch drift between the MAP Core schema JSON, the Rust command
//! type-name inventory, and the descriptor accessor ergonomics introduced by PR4,
//! PR4a, and PR4b.

use holons_core::descriptors::{
    CommandDescriptor, HolonDescriptor, HolonSpaceDescriptor, RelationshipDescriptor,
    TransactionDescriptor,
};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{HOLON_SPACE_TYPE_KEY, SCHEMA_TYPE_KEY, TRANSACTION_TYPE_KEY};
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::info;

// Concrete command descriptor holons that must exist in the loaded MAP Core schema.
// The abstract `CommandType` descriptor is checked separately and intentionally excluded here.
const STABLE_COMMAND_TYPES: &[(&str, CoreCommandTypeName)] = &[
    ("BeginTransaction.CommandType", CoreCommandTypeName::BeginTransaction),
    ("CloneHolon.CommandType", CoreCommandTypeName::CloneHolon),
    ("GetEssentialContent.CommandType", CoreCommandTypeName::GetEssentialContent),
    ("Summarize.CommandType", CoreCommandTypeName::Summarize),
    ("GetHolonId.CommandType", CoreCommandTypeName::GetHolonId),
    ("GetPredecessor.CommandType", CoreCommandTypeName::GetPredecessor),
    ("GetKey.CommandType", CoreCommandTypeName::GetKey),
    ("GetVersionedKey.CommandType", CoreCommandTypeName::GetVersionedKey),
    ("GetPropertyValue.CommandType", CoreCommandTypeName::GetPropertyValue),
    ("GetRelatedHolons.CommandType", CoreCommandTypeName::GetRelatedHolons),
    ("WithPropertyValue.CommandType", CoreCommandTypeName::WithPropertyValue),
    ("RemovePropertyValue.CommandType", CoreCommandTypeName::RemovePropertyValue),
    ("AddRelatedHolons.CommandType", CoreCommandTypeName::AddRelatedHolons),
    ("RemoveRelatedHolons.CommandType", CoreCommandTypeName::RemoveRelatedHolons),
    ("WithDescriptor.CommandType", CoreCommandTypeName::WithDescriptor),
    ("Commit.CommandType", CoreCommandTypeName::Commit),
    ("UndoLast.CommandType", CoreCommandTypeName::UndoLast),
    ("RedoLast.CommandType", CoreCommandTypeName::RedoLast),
    ("UndoToMarker.CommandType", CoreCommandTypeName::UndoToMarker),
    ("RedoToMarker.CommandType", CoreCommandTypeName::RedoToMarker),
    ("LoadHolons.CommandType", CoreCommandTypeName::LoadHolons),
    ("Dance.CommandType", CoreCommandTypeName::Dance),
    ("GetAllHolons.CommandType", CoreCommandTypeName::GetAllHolons),
    ("GetStagedHolonByBaseKey.CommandType", CoreCommandTypeName::GetStagedHolonByBaseKey),
    ("GetStagedHolonsByBaseKey.CommandType", CoreCommandTypeName::GetStagedHolonsByBaseKey),
    ("GetStagedHolonByVersionedKey.CommandType", CoreCommandTypeName::GetStagedHolonByVersionedKey),
    ("GetTransientHolonByBaseKey.CommandType", CoreCommandTypeName::GetTransientHolonByBaseKey),
    (
        "GetTransientHolonByVersionedKey.CommandType",
        CoreCommandTypeName::GetTransientHolonByVersionedKey,
    ),
    ("GetStagedCount.CommandType", CoreCommandTypeName::GetStagedCount),
    ("GetTransientCount.CommandType", CoreCommandTypeName::GetTransientCount),
    ("NewHolon.CommandType", CoreCommandTypeName::NewHolon),
    ("StageNewHolon.CommandType", CoreCommandTypeName::StageNewHolon),
    ("StageNewFromClone.CommandType", CoreCommandTypeName::StageNewFromClone),
    ("StageNewVersion.CommandType", CoreCommandTypeName::StageNewVersion),
    ("StageNewVersionFromId.CommandType", CoreCommandTypeName::StageNewVersionFromId),
    ("DeleteHolon.CommandType", CoreCommandTypeName::DeleteHolon),
];

// The command surface that every `HolonType` descendant should inherit from the core schema.
const HOLON_TYPE_AFFORDED_COMMANDS: &[CoreCommandTypeName] = &[
    CoreCommandTypeName::CloneHolon,
    CoreCommandTypeName::GetEssentialContent,
    CoreCommandTypeName::Summarize,
    CoreCommandTypeName::GetHolonId,
    CoreCommandTypeName::GetPredecessor,
    CoreCommandTypeName::GetKey,
    CoreCommandTypeName::GetVersionedKey,
    CoreCommandTypeName::GetPropertyValue,
    CoreCommandTypeName::GetRelatedHolons,
    CoreCommandTypeName::WithPropertyValue,
    CoreCommandTypeName::RemovePropertyValue,
    CoreCommandTypeName::AddRelatedHolons,
    CoreCommandTypeName::RemoveRelatedHolons,
    CoreCommandTypeName::WithDescriptor,
];

// The transaction-scope command surface anchored by the schema-backed `Transaction`.
// `BeginTransaction` intentionally stays on `HolonSpace`; it creates access to this model
// but is not itself a command available from an active transaction model.
const TRANSACTION_AFFORDED_COMMANDS: &[CoreCommandTypeName] = &[
    CoreCommandTypeName::Commit,
    CoreCommandTypeName::UndoLast,
    CoreCommandTypeName::RedoLast,
    CoreCommandTypeName::UndoToMarker,
    CoreCommandTypeName::RedoToMarker,
    CoreCommandTypeName::LoadHolons,
    CoreCommandTypeName::Dance,
    CoreCommandTypeName::GetAllHolons,
    CoreCommandTypeName::GetStagedHolonByBaseKey,
    CoreCommandTypeName::GetStagedHolonsByBaseKey,
    CoreCommandTypeName::GetStagedHolonByVersionedKey,
    CoreCommandTypeName::GetTransientHolonByBaseKey,
    CoreCommandTypeName::GetTransientHolonByVersionedKey,
    CoreCommandTypeName::GetStagedCount,
    CoreCommandTypeName::GetTransientCount,
    CoreCommandTypeName::NewHolon,
    CoreCommandTypeName::StageNewHolon,
    CoreCommandTypeName::StageNewFromClone,
    CoreCommandTypeName::StageNewVersion,
    CoreCommandTypeName::StageNewVersionFromId,
    CoreCommandTypeName::DeleteHolon,
];

/// Verifies command descriptor inventory and schema-backed command affordance lookup.
pub async fn execute_verify_core_schema_command_affordances(state: &mut TestExecutionState) {
    // Fetch the current collection once; later checks select expected schema descriptors by key.
    let holons = loaded_holons(state, "verify_core_schema_command_affordances").await;

    // `CommandType` is the abstract descriptor family root, not a concrete command inventory item.
    let command_type =
        CommandDescriptor::from_holon(find_holon_by_key(&holons, "CommandType.HolonType"));
    assert_eq!(
        command_type.command_name().expect("CommandType command_name"),
        CommandName(MapString("CommandType".to_string()))
    );
    assert!(command_type.header().is_abstract_type().expect("CommandType is_abstract_type"));

    // Every concrete command descriptor should expose its canonical `CommandName` via `type_name`.
    for (key, expected_type_name) in STABLE_COMMAND_TYPES {
        let command = CommandDescriptor::from_holon(find_holon_by_key(&holons, key));
        assert_eq!(
            command.command_name().expect("concrete command command_name"),
            expected_type_name.as_command_name(),
            "{key} should expose its command identity through type_name"
        );
    }

    // `AffordsCommand` is the declared descriptor edge from holon types to command types.
    let affordance_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(HolonType)-[AffordsCommand]->(CommandType.HolonType)",
    ))
    .try_into_declared_relationship_descriptor()
    .expect("AffordsCommand should be a declared relationship descriptor");
    assert_eq!(
        affordance_relationship
            .base_relationship_name()
            .expect("AffordsCommand base_relationship_name")
            .to_string(),
        "AffordsCommand"
    );
    assert_eq!(
        affordance_relationship
            .source_type()
            .expect("AffordsCommand source_type")
            .header()
            .type_name()
            .expect("AffordsCommand source type_name"),
        MapString("HolonType".to_string())
    );
    assert_eq!(
        affordance_relationship
            .target_type()
            .expect("AffordsCommand target_type")
            .header()
            .type_name()
            .expect("AffordsCommand target type_name"),
        MapString("CommandType".to_string())
    );

    // `AffordedBy` must be the inverse view of the same command affordance relationship.
    let inverse_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(CommandType.HolonType)-[AffordedBy]->(HolonType)",
    ))
    .try_into_inverse_relationship_descriptor()
    .expect("AffordedBy should be an inverse relationship descriptor");
    assert_eq!(
        inverse_relationship
            .inverse_of()
            .expect("AffordedBy inverse_of")
            .base_relationship_name()
            .expect("AffordedBy inverse_of base name")
            .to_string(),
        "AffordsCommand"
    );

    // `HolonType` owns the baseline command affordance set and typed command lookup behavior.
    let holon_type_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, "HolonType"));
    let instance_relationship_names =
        relationship_base_names(holon_type_descriptor.instance_relationships());
    assert_contains(&instance_relationship_names, "AffordsCommand");

    assert_command_set_eq(
        command_names(holon_type_descriptor.afforded_commands()),
        HOLON_TYPE_AFFORDED_COMMANDS,
        "HolonType should afford exactly the PR4 holon-scoped commands",
    );
    assert_eq!(
        holon_type_descriptor
            .get_command_by_name(CoreCommandTypeName::GetKey)
            .expect("GetKey lookup")
            .command_name()
            .expect("resolved GetKey command_name"),
        CoreCommandTypeName::GetKey.as_command_name()
    );
    assert!(matches!(
        holon_type_descriptor.get_command_by_name(MapString("NonexistentCommand".to_string())),
        Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
            if kind == "command" && name == "NonexistentCommand"
    ));

    // Ordinary holon descriptor families should inherit the baseline `HolonType` commands.
    let schema_type_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, SCHEMA_TYPE_KEY));
    assert_command_set_eq(
        command_names(schema_type_descriptor.afforded_commands()),
        HOLON_TYPE_AFFORDED_COMMANDS,
        "Schema.HolonType should inherit HolonType's command affordances",
    );

    // `HolonSpace` extends the baseline holon command set with space-level transaction entry.
    let holon_space_type_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, HOLON_SPACE_TYPE_KEY));
    let mut holon_space_commands: Vec<CoreCommandTypeName> = HOLON_TYPE_AFFORDED_COMMANDS.to_vec();
    holon_space_commands.push(CoreCommandTypeName::BeginTransaction);
    assert_command_set_eq(
        command_names(holon_space_type_descriptor.afforded_commands()),
        &holon_space_commands,
        "HolonSpace should afford BeginTransaction and inherit HolonType commands",
    );

    // `Transaction` is the concrete descriptor home for transaction-scope commands.
    let transaction_type_descriptor =
        TransactionDescriptor::from_holon(find_holon_by_key(&holons, TRANSACTION_TYPE_KEY));
    assert_eq!(
        transaction_type_descriptor.header().type_name().expect("Transaction type_name"),
        MapString("Transaction".to_string())
    );
    assert!(
        !transaction_type_descriptor
            .header()
            .is_abstract_type()
            .expect("Transaction is_abstract_type"),
        "Transaction should be concrete"
    );

    // Descriptor discovery starts at `HolonSpace`, follows exactly one
    // `AffordsTransactionModel` edge, and returns the same `Transaction` model.
    let discovered_transaction_model =
        HolonSpaceDescriptor::from_holon(find_holon_by_key(&holons, HOLON_SPACE_TYPE_KEY))
            .transaction_model()
            .expect("HolonSpace transaction_model");
    assert_eq!(
        discovered_transaction_model
            .header()
            .type_name()
            .expect("discovered transaction model type_name"),
        MapString("Transaction".to_string())
    );

    // Direct `Transaction` affordances are exactly the transaction-scoped command inventory.
    // The descriptor-level accessor below is intentionally broader because it flattens inherited
    // `HolonType` affordances through `Extends`, matching the Rust descriptor contract.
    assert_command_set_eq(
        direct_command_names(
            transaction_type_descriptor.holon(),
            CoreRelationshipTypeName::AffordsCommand,
        ),
        TRANSACTION_AFFORDED_COMMANDS,
        "Transaction should directly afford exactly the transaction-scoped commands",
    );

    // Effective transaction-model command discovery comes from descriptor relationships: the
    // local transaction commands plus inherited baseline `HolonType` commands.
    let mut effective_transaction_commands: Vec<CoreCommandTypeName> =
        HOLON_TYPE_AFFORDED_COMMANDS.to_vec();
    effective_transaction_commands.extend_from_slice(TRANSACTION_AFFORDED_COMMANDS);
    let transaction_command_names = command_names(transaction_type_descriptor.afforded_commands());
    assert_command_set_eq(
        transaction_command_names.clone(),
        &effective_transaction_commands,
        "TransactionDescriptor should flatten inherited HolonType commands plus transaction-scoped commands",
    );
    assert_command_absent(
        &transaction_command_names,
        CoreCommandTypeName::BeginTransaction,
        "Transaction must not afford BeginTransaction",
    );
    assert_eq!(
        transaction_type_descriptor
            .get_command_by_name(CoreCommandTypeName::Commit)
            .expect("Commit lookup through Transaction")
            .command_name()
            .expect("resolved Commit command_name"),
        CoreCommandTypeName::Commit.as_command_name()
    );

    // The transaction model relationship is a singular declared edge from
    // `HolonSpace` to `Transaction`, with an inverse edge back to its owner space type.
    let transaction_model_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(HolonSpace.HolonType)-[AffordsTransactionModel]->(Transaction.HolonType)",
    ))
    .try_into_declared_relationship_descriptor()
    .expect("AffordsTransactionModel should be a declared relationship descriptor");
    assert_eq!(
        transaction_model_relationship
            .base_relationship_name()
            .expect("AffordsTransactionModel base_relationship_name")
            .to_string(),
        "AffordsTransactionModel"
    );
    assert_eq!(
        transaction_model_relationship
            .source_type()
            .expect("AffordsTransactionModel source_type")
            .header()
            .type_name()
            .expect("AffordsTransactionModel source type_name"),
        MapString("HolonSpace".to_string())
    );
    assert_eq!(
        transaction_model_relationship
            .target_type()
            .expect("AffordsTransactionModel target_type")
            .header()
            .type_name()
            .expect("AffordsTransactionModel target type_name"),
        MapString("Transaction".to_string())
    );
    assert_eq!(
        transaction_model_relationship
            .min_cardinality()
            .expect("AffordsTransactionModel min_cardinality"),
        1
    );
    assert_eq!(
        transaction_model_relationship
            .max_cardinality()
            .expect("AffordsTransactionModel max_cardinality"),
        1
    );
    assert!(
        !transaction_model_relationship
            .allows_duplicates()
            .expect("AffordsTransactionModel allows_duplicates"),
        "AffordsTransactionModel should be singular"
    );
    assert_eq!(
        transaction_model_relationship
            .deletion_semantic()
            .expect("AffordsTransactionModel deletion_semantic"),
        Some(MapString("Allow".to_string()))
    );

    let transaction_model_inverse = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(Transaction.HolonType)-[TransactionModelAffordedBy]->(HolonSpace.HolonType)",
    ))
    .try_into_inverse_relationship_descriptor()
    .expect("TransactionModelAffordedBy should be an inverse relationship descriptor");
    assert_eq!(
        transaction_model_inverse
            .inverse_of()
            .expect("TransactionModelAffordedBy inverse_of")
            .base_relationship_name()
            .expect("TransactionModelAffordedBy inverse_of base name")
            .to_string(),
        "AffordsTransactionModel"
    );
    assert_eq!(
        transaction_model_inverse
            .deletion_semantic()
            .expect("TransactionModelAffordedBy deletion_semantic"),
        Some(MapString("Allow".to_string()))
    );

    info!(
        "verified core schema command inventory, HolonType affordances, and Transaction affordances: {:?}",
        TRANSACTION_AFFORDED_COMMANDS
    );
}

async fn loaded_holons(state: &mut TestExecutionState, step_name: &str) -> HolonCollection {
    let context = state.open_assertion_context(step_name).await.unwrap_or_else(|error| {
        panic!("{step_name}: failed to open assertion transaction: {error:?}")
    });

    // GetAllHolons may return more than schema descriptors; callers below pick the relevant
    // loaded schema holons by stable keys and ignore unrelated entries.
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::GetAllHolons,
    });
    let result = state
        .dispatch_command(command, step_name)
        .await
        .unwrap_or_else(|error| panic!("{step_name}: get_all_holons failed: {error:?}"));

    match result {
        MapResult::Collection(collection) => collection,
        other => panic!("{step_name}: expected Collection, got {other:?}"),
    }
}

fn find_holon_by_key(holons: &HolonCollection, key: &str) -> HolonReference {
    holons
        .get_by_key(&MapString::from(key))
        .unwrap_or_else(|error| panic!("key lookup for {key} failed: {error:?}"))
        .unwrap_or_else(|| panic!("expected loaded holon with key {key}"))
}

fn relationship_base_names(
    descriptors: Result<Vec<RelationshipDescriptor>, HolonError>,
) -> Vec<String> {
    descriptors
        .expect("relationship descriptor list")
        .into_iter()
        .map(|descriptor| {
            descriptor
                .base_relationship_name()
                .expect("relationship descriptor base name")
                .to_string()
        })
        .collect()
}

fn assert_contains(values: &[String], expected: &str) {
    assert!(
        values.iter().any(|actual| actual == expected),
        "expected {values:?} to contain {expected}"
    );
}

fn assert_command_absent(values: &[CommandName], unexpected: CoreCommandTypeName, message: &str) {
    let unexpected = unexpected.as_command_name();
    assert!(
        !values.iter().any(|actual| actual == &unexpected),
        "{message}: found unexpected command {unexpected}"
    );
}

fn command_names(descriptors: Result<Vec<CommandDescriptor>, HolonError>) -> Vec<CommandName> {
    descriptors
        .expect("command descriptor list")
        .into_iter()
        .map(|descriptor| descriptor.command_name().expect("command_name"))
        .collect()
}

fn direct_command_names(
    descriptor: &HolonReference,
    relationship_name: CoreRelationshipTypeName,
) -> Vec<CommandName> {
    let collection =
        descriptor.related_holons(relationship_name).expect("direct command relationship");
    let collection = collection.read().expect("direct command relationship lock");

    collection
        .get_members()
        .into_iter()
        .cloned()
        .map(CommandDescriptor::from_holon)
        .map(|descriptor| descriptor.command_name().expect("direct command_name"))
        .collect()
}

fn assert_command_set_eq(
    actual: Vec<CommandName>,
    expected: &[CoreCommandTypeName],
    message: &str,
) {
    let mut actual = actual;
    let mut expected =
        expected.iter().map(CoreCommandTypeName::as_command_name).collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "{message}");
}
