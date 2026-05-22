use holons_core::descriptors::{CommandDescriptor, HolonDescriptor, RelationshipDescriptor};
use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::info;

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
    ("Query.CommandType", CoreCommandTypeName::Query),
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

/// Verifies command descriptor inventory and schema-backed command affordance lookup.
pub async fn execute_verify_core_schema_command_affordances(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_core_schema_command_affordances").await;

    let command_type = CommandDescriptor::from_holon(find_holon_by_key(&holons, "CommandType"));
    assert_eq!(
        command_type.command_name().expect("CommandType command_name"),
        CommandName(MapString("CommandType".to_string()))
    );
    assert!(command_type.header().is_abstract_type().expect("CommandType is_abstract_type"));

    for (key, expected_type_name) in STABLE_COMMAND_TYPES {
        let command = CommandDescriptor::from_holon(find_holon_by_key(&holons, key));
        assert_eq!(
            command.command_name().expect("concrete command command_name"),
            expected_type_name.as_command_name(),
            "{key} should expose its command identity through type_name"
        );
    }

    let affordance_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(HolonType)-[AffordsCommand]->(CommandType)",
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

    let inverse_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(CommandType)-[AffordedBy]->(HolonType)",
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

    let schema_type_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, "SchemaType"));
    assert_command_set_eq(
        command_names(schema_type_descriptor.afforded_commands()),
        HOLON_TYPE_AFFORDED_COMMANDS,
        "SchemaType should inherit HolonType's command affordances",
    );

    let holon_space_type_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, "HolonSpaceType"));
    let mut holon_space_commands: Vec<CoreCommandTypeName> = HOLON_TYPE_AFFORDED_COMMANDS.to_vec();
    holon_space_commands.push(CoreCommandTypeName::BeginTransaction);
    assert_command_set_eq(
        command_names(holon_space_type_descriptor.afforded_commands()),
        &holon_space_commands,
        "HolonSpaceType should afford BeginTransaction and inherit HolonType commands",
    );

    info!(
        "verified core schema command inventory and HolonType command affordances: {:?}",
        HOLON_TYPE_AFFORDED_COMMANDS
    );
}

async fn loaded_holons(state: &mut TestExecutionState, step_name: &str) -> HolonCollection {
    let context = state.open_assertion_context(step_name).await.unwrap_or_else(|error| {
        panic!("{step_name}: failed to open assertion transaction: {error:?}")
    });

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

fn command_names(descriptors: Result<Vec<CommandDescriptor>, HolonError>) -> Vec<CommandName> {
    descriptors
        .expect("command descriptor list")
        .into_iter()
        .map(|descriptor| descriptor.command_name().expect("command_name"))
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
