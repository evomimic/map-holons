use holons_core::descriptors::{HolonDescriptor, RelationshipDescriptor};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP_KEY,
    CORE_INSTANCE_PROPERTIES_RELATIONSHIP_KEY, CORE_INSTANCE_PROPERTY_FOR_RELATIONSHIP_KEY,
    HOLON_TYPE_KEY, PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY, SCHEMA_TYPE_KEY,
};
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use tracing::info;

/// Verifies representative foundational descriptor access over loaded MAP core schema data.
pub async fn execute_verify_core_schema_descriptors(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_core_schema_descriptors").await;

    let schema_type = find_holon_by_key(&holons, SCHEMA_TYPE_KEY);
    let schema_type_descriptor = HolonDescriptor::from_holon(schema_type);
    assert_eq!(
        schema_type_descriptor.header().type_name().expect("SchemaType type_name"),
        MapString(SCHEMA_TYPE_KEY.to_string())
    );
    assert!(!schema_type_descriptor
        .allows_additional_properties()
        .expect("SchemaType allows_additional_properties"));
    assert!(!schema_type_descriptor
        .allows_additional_relationships()
        .expect("SchemaType allows_additional_relationships"));

    let holon_type = find_holon_by_key(&holons, HOLON_TYPE_KEY);
    let holon_type_descriptor = HolonDescriptor::from_holon(holon_type);
    assert_eq!(
        holon_type_descriptor.header().type_name().expect("HolonType type_name"),
        MapString("HolonType".to_string())
    );

    let instance_property_names = property_type_names(holon_type_descriptor.instance_properties());
    assert_contains(&instance_property_names, "AllowsAdditionalProperties");
    assert_contains(&instance_property_names, "AllowsAdditionalRelationships");

    let instance_relationship_names =
        relationship_base_names(holon_type_descriptor.instance_relationships());
    assert_contains(&instance_relationship_names, "Properties");
    assert_contains(&instance_relationship_names, "DescribedBy");

    let property = holon_type_descriptor
        .get_property_by_name(PropertyName(MapString::from("AllowsAdditionalProperties")))
        .expect("AllowsAdditionalProperties lookup");
    assert_eq!(
        property
            .value_type()
            .expect("AllowsAdditionalProperties value_type")
            .header()
            .type_name()
            .expect("AllowsAdditionalProperties value type_name"),
        MapString("MapBooleanValueType".to_string())
    );

    let relationship = holon_type_descriptor
        .get_relationship_by_name(RelationshipName(MapString::from("Properties")))
        .expect("Properties relationship lookup");
    assert_relationship_shape(
        relationship.base_relationship_name(),
        relationship.source_type(),
        relationship.target_type(),
        relationship.full_relationship_name(),
        "Properties",
        "HolonType",
        "PropertyType",
        "(HolonType)-[Properties]->(PropertyType)",
    );

    info!("verified representative core schema descriptor access");
}

/// Verifies declared/inverse relationship subtype narrowing over loaded MAP core schema data.
pub async fn execute_verify_core_schema_descriptor_subtypes(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_core_schema_descriptor_subtypes").await;

    let declared = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        CORE_INSTANCE_PROPERTIES_RELATIONSHIP_KEY,
    ))
    .try_into_declared_relationship_descriptor()
    .expect("core declared relationship should narrow");
    assert_relationship_shape(
        declared.base_relationship_name(),
        declared.source_type(),
        declared.target_type(),
        declared.full_relationship_name(),
        "InstanceProperties",
        "TypeDescriptor",
        "PropertyType",
        "(TypeDescriptor)-[InstanceProperties]->(PropertyType)",
    );

    let inverse = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        CORE_INSTANCE_PROPERTY_FOR_RELATIONSHIP_KEY,
    ))
    .try_into_inverse_relationship_descriptor()
    .expect("core inverse relationship should narrow");
    assert_relationship_shape(
        inverse.base_relationship_name(),
        inverse.source_type(),
        inverse.target_type(),
        inverse.full_relationship_name(),
        "InstancePropertyFor",
        "PropertyType",
        "TypeDescriptor",
        "(PropertyType)-[InstancePropertyFor]->(TypeDescriptor)",
    );
    assert_eq!(
        inverse
            .inverse_of()
            .expect("core inverse relationship inverse_of")
            .base_relationship_name()
            .expect("core inverse_of base name")
            .to_string(),
        "InstanceProperties"
    );

    info!("verified core schema descriptor subtype access");
}

/// Verifies representative descriptor access over the loaded Book/Person inverse schema.
pub async fn execute_verify_book_person_descriptors(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_book_person_descriptors").await;

    let book_descriptor =
        HolonDescriptor::from_holon(find_holon_by_key(&holons, BOOK_DESCRIPTOR_KEY));
    assert_eq!(
        book_descriptor.header().type_name().expect("Book type_name"),
        MapString("Book".to_string())
    );
    assert!(!book_descriptor
        .allows_additional_properties()
        .expect("Book allows_additional_properties"));
    assert!(!book_descriptor
        .allows_additional_relationships()
        .expect("Book allows_additional_relationships"));

    let instance_property_names = property_type_names(book_descriptor.instance_properties());
    assert_contains(&instance_property_names, "Title");
    assert_contains(&instance_property_names, "AllowsAdditionalProperties");

    let instance_relationship_names =
        relationship_base_names(book_descriptor.instance_relationships());
    assert_contains(&instance_relationship_names, "AuthoredBy");
    assert_contains(&instance_relationship_names, "Properties");

    let title = book_descriptor
        .get_property_by_name(PropertyName(MapString::from("Title")))
        .expect("Title property lookup");
    assert_eq!(
        title
            .value_type()
            .expect("Title value_type")
            .header()
            .type_name()
            .expect("Title value type_name"),
        MapString("MapStringValueType".to_string())
    );

    let authored_by = book_descriptor
        .get_relationship_by_name(RelationshipName(MapString::from("AuthoredBy")))
        .expect("AuthoredBy relationship lookup");
    assert_relationship_shape(
        authored_by.base_relationship_name(),
        authored_by.source_type(),
        authored_by.target_type(),
        authored_by.full_relationship_name(),
        "AuthoredBy",
        "Book",
        "Person",
        "(Book)-[AuthoredBy]->(Person)",
    );

    let inverse = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY,
    ))
    .try_into_inverse_relationship_descriptor()
    .expect("Book/Person inverse relationship should narrow");
    assert_eq!(
        inverse
            .inverse_of()
            .expect("Book/Person inverse_of")
            .base_relationship_name()
            .expect("Book/Person inverse_of base name")
            .to_string(),
        "AuthoredBy"
    );

    let declared = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        BOOK_TO_PERSON_RELATIONSHIP_KEY,
    ))
    .try_into_declared_relationship_descriptor()
    .expect("Book/Person declared relationship should narrow");
    assert_relationship_shape(
        declared.base_relationship_name(),
        declared.source_type(),
        declared.target_type(),
        declared.full_relationship_name(),
        "AuthoredBy",
        "Book",
        "Person",
        "(Book)-[AuthoredBy]->(Person)",
    );

    info!("verified representative Book/Person descriptor access");
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

fn property_type_names(
    descriptors: Result<Vec<holons_core::descriptors::PropertyDescriptor>, HolonError>,
) -> Vec<String> {
    descriptors
        .expect("property descriptor list")
        .into_iter()
        .map(|descriptor| descriptor.header().type_name().expect("property descriptor type_name").0)
        .collect()
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

fn assert_relationship_shape(
    base_relationship_name: Result<RelationshipName, HolonError>,
    source_type: Result<HolonDescriptor, HolonError>,
    target_type: Result<HolonDescriptor, HolonError>,
    full_relationship_name: Result<MapString, HolonError>,
    expected_base_name: &str,
    expected_source_type: &str,
    expected_target_type: &str,
    expected_full_name: &str,
) {
    assert_eq!(
        base_relationship_name.expect("base relationship name").to_string(),
        expected_base_name
    );
    assert_eq!(
        source_type.expect("source_type").header().type_name().expect("source type_name"),
        MapString(expected_source_type.to_string())
    );
    assert_eq!(
        target_type.expect("target_type").header().type_name().expect("target type_name"),
        MapString(expected_target_type.to_string())
    );
    assert_eq!(
        full_relationship_name.expect("full relationship name"),
        MapString(expected_full_name.to_string())
    );
}
