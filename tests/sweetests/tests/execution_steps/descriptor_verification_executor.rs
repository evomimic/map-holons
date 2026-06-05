use core_types::TypeKind;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::{
    DanceDescriptor, HolonDescriptor, OperatorCategory, OperatorDescriptor, RelationshipDescriptor,
    ValueDescriptor,
};
use holons_core::reference_layer::{HolonReference, TransientReference, WritableHolon};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP_KEY,
    CORE_INSTANCE_PROPERTIES_RELATIONSHIP_KEY, CORE_INSTANCE_PROPERTY_FOR_RELATIONSHIP_KEY,
    DELETION_SEMANTIC_ALLOW_KEY, DELETION_SEMANTIC_BLOCK_KEY, DELETION_SEMANTIC_CASCADE_KEY,
    DELETION_SEMANTIC_KEY, HOLON_TYPE_KEY, OPERATOR_CATEGORY_EQUALITY_KEY, OPERATOR_CATEGORY_KEY,
    OPERATOR_CATEGORY_ORDERING_KEY, PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY, SCHEMA_TYPE_KEY,
    VARIANTS_RELATIONSHIP,
};
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing::info;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, DanceName};

/// Verifies representative foundational descriptor access over loaded MAP core schema data.
pub async fn execute_verify_core_schema_descriptors(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_core_schema_descriptors").await;

    let schema_type = find_holon_by_key(&holons, SCHEMA_TYPE_KEY);
    let schema_type_descriptor = HolonDescriptor::from_holon(schema_type);
    assert_eq!(
        schema_type_descriptor.header().type_name().expect("Schema type_name"),
        MapString("Schema".to_string())
    );
    assert!(!schema_type_descriptor
        .allows_additional_properties()
        .expect("Schema allows_additional_properties"));
    assert!(!schema_type_descriptor
        .allows_additional_relationships()
        .expect("Schema allows_additional_relationships"));

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

    let dance_type = find_holon_by_key(&holons, "DanceType");
    let dance_type_descriptor = HolonDescriptor::from_holon(dance_type.clone());
    let dance_descriptor = DanceDescriptor::from_holon(dance_type.clone());
    assert!(!property_type_names(dance_type_descriptor.instance_properties())
        .contains(&"DanceName".to_string()));
    assert_eq!(
        dance_descriptor.dance_name().expect("DanceType dance_name"),
        DanceName(MapString("DanceType".to_string()))
    );
    let request_type_relationship = dance_type_descriptor
        .get_relationship_by_name(RelationshipName(MapString::from("RequestType")))
        .expect("DanceType.RequestType lookup");
    assert_relationship_shape(
        request_type_relationship.base_relationship_name(),
        request_type_relationship.source_type(),
        request_type_relationship.target_type(),
        request_type_relationship.full_relationship_name(),
        "RequestType",
        "DanceType",
        "HolonType",
        "(DanceType)-[RequestType]->(HolonType)",
    );
    let response_relationship = dance_type_descriptor
        .get_relationship_by_name(RelationshipName(MapString::from("Response")))
        .expect("DanceType.Response lookup");
    assert_relationship_shape(
        response_relationship.base_relationship_name(),
        response_relationship.source_type(),
        response_relationship.target_type(),
        response_relationship.full_relationship_name(),
        "Response",
        "DanceType",
        "DanceResponseType",
        "(DanceType)-[Response]->(DanceResponseType)",
    );
    assert_eq!(dance_descriptor.request_type().expect("DanceType request_type").is_none(), true);
    assert_contains(
        &relationship_base_names(dance_type_descriptor.instance_relationships()),
        "RequestType",
    );
    assert_contains(
        &relationship_base_names(dance_type_descriptor.instance_relationships()),
        "Response",
    );
    let affordance_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(HolonType)-[Affords]->(DanceType)",
    ))
    .try_into_declared_relationship_descriptor()
    .expect("Affords should be a declared relationship descriptor");
    assert_relationship_shape(
        affordance_relationship.base_relationship_name(),
        affordance_relationship.source_type(),
        affordance_relationship.target_type(),
        affordance_relationship.full_relationship_name(),
        "Affords",
        "HolonType",
        "DanceType",
        "(HolonType)-[Affords]->(DanceType)",
    );
    let afforded_by_relationship = RelationshipDescriptor::from_holon(find_holon_by_key(
        &holons,
        "(DanceType)-[AffordedBy]->(HolonType)",
    ))
    .try_into_inverse_relationship_descriptor()
    .expect("AffordedBy should be an inverse relationship descriptor");
    assert_eq!(
        afforded_by_relationship
            .inverse_of()
            .expect("AffordedBy inverse_of")
            .base_relationship_name()
            .expect("AffordedBy inverse_of base name")
            .to_string(),
        "Affords"
    );
    assert_loaded_schema_backed_dance_discovery(state, &holons).await;

    let dance_response_type = find_holon_by_key(&holons, "DanceResponseType");
    let dance_response_descriptor = HolonDescriptor::from_holon(dance_response_type.clone());
    let response_body_relationship = dance_response_descriptor
        .get_relationship_by_name(RelationshipName(MapString::from("ResponseBody")))
        .expect("DanceResponseType.ResponseBody lookup");
    assert_relationship_shape(
        response_body_relationship.base_relationship_name(),
        response_body_relationship.source_type(),
        response_body_relationship.target_type(),
        response_body_relationship.full_relationship_name(),
        "ResponseBody",
        "DanceResponseType",
        "HolonType",
        "(DanceResponseType)-[ResponseBody]->(HolonType)",
    );
    assert_contains(
        &relationship_base_names(dance_response_descriptor.instance_relationships()),
        "Diagnostics",
    );

    let projection = find_holon_by_key(&holons, "Projection");
    let projection_descriptor = HolonDescriptor::from_holon(projection.clone());
    assert_eq!(
        projection_descriptor.header().type_name().expect("Projection type_name"),
        MapString("Projection".to_string())
    );
    assert_contains(&related_holon_keys(&projection, "Extends"), "HolonType");

    let dance_invocation = find_holon_by_key(&holons, "DanceInvocation");
    let dance_invocation_descriptor = HolonDescriptor::from_holon(dance_invocation.clone());
    let invocation_property_names =
        property_type_names(dance_invocation_descriptor.instance_properties());
    assert_contains(&invocation_property_names, "Context");
    let invocation_relationship_names =
        relationship_base_names(dance_invocation_descriptor.instance_relationships());
    assert_contains(&invocation_relationship_names, "InvokesDance");
    assert_contains(&invocation_relationship_names, "Target");
    assert_contains(&invocation_relationship_names, "Request");
    let context_property = dance_invocation_descriptor
        .get_property_by_name(PropertyName(MapString::from("Context")))
        .expect("DanceInvocation.Context lookup");
    assert_eq!(
        context_property
            .value_type()
            .expect("DanceInvocation.Context value_type")
            .header()
            .type_name()
            .expect("DanceInvocation.Context value type_name"),
        MapString("InvocationSource".to_string())
    );

    let dance_diagnostic = find_holon_by_key(&holons, "DanceDiagnostic");
    let dance_diagnostic_descriptor = HolonDescriptor::from_holon(dance_diagnostic);
    let diagnostic_property_names =
        property_type_names(dance_diagnostic_descriptor.instance_properties());
    assert_contains(&diagnostic_property_names, "DanceDiagnosticSeverity");
    assert_contains(&diagnostic_property_names, "DiagnosticCode");
    assert_contains(&diagnostic_property_names, "DiagnosticMessage");

    let invocation_source = find_holon_by_key(&holons, "InvocationSource");
    assert_enum_variants_rewritten_to_declared_side(
        &holons,
        "InvocationSource",
        &[
            "InvocationSource.ClientCommand",
            "InvocationSource.TrustChannel",
            "InvocationSource.Internal",
        ],
    );
    assert_contains(
        &related_holon_keys(&invocation_source, "Variants"),
        "InvocationSource.ClientCommand",
    );

    assert_enum_variants_rewritten_to_declared_side(
        &holons,
        "DanceDiagnosticSeverity",
        &["DanceDiagnosticSeverity.Info", "DanceDiagnosticSeverity.Warning"],
    );

    info!("verified representative core schema descriptor access");
}

async fn assert_loaded_schema_backed_dance_discovery(
    state: &mut TestExecutionState,
    holons: &HolonCollection,
) {
    let context = state
        .open_assertion_context("verify_core_schema_dance_affordances")
        .await
        .unwrap_or_else(|error| {
            panic!("verify_core_schema_dance_affordances: failed to open assertion transaction: {error:?}")
        });

    let holon_type = find_holon_by_key(holons, HOLON_TYPE_KEY);
    let dance_type = find_holon_by_key(holons, "DanceType");
    let dance_response_type = find_holon_by_key(holons, "DanceResponseType");
    let projection = find_holon_by_key(holons, "Projection");

    let mut query_response_type =
        new_descriptor_holon(&context, "query-response-type", "QueryResponseType", TypeKind::Holon)
            .expect("query response type");
    query_response_type
        .add_related_holons(CoreRelationshipTypeName::Extends, vec![dance_response_type.clone()])
        .expect("QueryResponseType extends DanceResponseType");
    query_response_type
        .add_related_holons(CoreRelationshipTypeName::ResponseBody, vec![projection.clone()])
        .expect("QueryResponseType response body");

    let mut inspect_response_type = new_descriptor_holon(
        &context,
        "inspect-response-type",
        "InspectResponseType",
        TypeKind::Holon,
    )
    .expect("inspect response type");
    inspect_response_type
        .add_related_holons(CoreRelationshipTypeName::Extends, vec![dance_response_type.clone()])
        .expect("InspectResponseType extends DanceResponseType");
    inspect_response_type
        .add_related_holons(CoreRelationshipTypeName::ResponseBody, vec![projection.clone()])
        .expect("InspectResponseType response body");

    let mut query_dance =
        new_descriptor_holon(&context, "query-dance-type", "Query", TypeKind::Holon)
            .expect("Query dance");
    query_dance
        .add_related_holons(CoreRelationshipTypeName::Extends, vec![dance_type.clone()])
        .expect("Query extends DanceType");
    query_dance
        .add_related_holons(CoreRelationshipTypeName::RequestType, vec![projection.clone()])
        .expect("Query request type");
    query_dance
        .add_related_holons(
            CoreRelationshipTypeName::Response,
            vec![HolonReference::from(&query_response_type)],
        )
        .expect("Query response");

    let mut inspect_dance =
        new_descriptor_holon(&context, "inspect-dance-type", "Inspect", TypeKind::Holon)
            .expect("Inspect dance");
    inspect_dance
        .add_related_holons(CoreRelationshipTypeName::Extends, vec![dance_type.clone()])
        .expect("Inspect extends DanceType");
    inspect_dance
        .add_related_holons(CoreRelationshipTypeName::RequestType, vec![projection.clone()])
        .expect("Inspect request type");
    inspect_dance
        .add_related_holons(
            CoreRelationshipTypeName::Response,
            vec![HolonReference::from(&inspect_response_type)],
        )
        .expect("Inspect response");

    let mut parent_owner =
        new_descriptor_holon(&context, "dance-parent-owner", "DanceParentOwner", TypeKind::Holon)
            .expect("dance parent owner");
    parent_owner
        .add_related_holons(CoreRelationshipTypeName::Extends, vec![holon_type.clone()])
        .expect("DanceParentOwner extends HolonType");
    parent_owner
        .add_related_holons(
            CoreRelationshipTypeName::Affords,
            vec![HolonReference::from(&query_dance)],
        )
        .expect("DanceParentOwner affords Query");

    let mut child_owner =
        new_descriptor_holon(&context, "dance-child-owner", "DanceChildOwner", TypeKind::Holon)
            .expect("dance child owner");
    child_owner
        .add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![HolonReference::from(&parent_owner)],
        )
        .expect("DanceChildOwner extends DanceParentOwner");
    child_owner
        .add_related_holons(
            CoreRelationshipTypeName::Affords,
            vec![HolonReference::from(&inspect_dance)],
        )
        .expect("DanceChildOwner affords Inspect");

    let child_descriptor = HolonDescriptor::from_holon(HolonReference::from(&child_owner));
    assert_eq!(
        dance_type_names(child_descriptor.afforded_dances()),
        vec![
            DanceName(MapString("Inspect".to_string())),
            DanceName(MapString("Query".to_string())),
        ]
    );
    assert_eq!(
        child_descriptor
            .get_dance_by_name("inspect")
            .expect("Inspect lookup")
            .dance_name()
            .expect("Inspect dance_name"),
        DanceName(MapString("Inspect".to_string()))
    );

    let inherited_query = child_descriptor
        .get_dance_by_name("query")
        .expect("Query lookup through inherited Affords");
    assert_eq!(
        inherited_query
            .request_type()
            .expect("Query request_type")
            .expect("Query request type target")
            .header()
            .type_name()
            .expect("Query request type_name"),
        MapString("Projection".to_string())
    );
    assert_eq!(
        inherited_query
            .response()
            .expect("Query response")
            .response_body()
            .expect("Query response_body")
            .expect("Query response body target")
            .header()
            .type_name()
            .expect("Query response body type_name"),
        MapString("Projection".to_string())
    );
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

    assert_enum_variants_rewritten_to_declared_side(
        &holons,
        DELETION_SEMANTIC_KEY,
        &[DELETION_SEMANTIC_ALLOW_KEY, DELETION_SEMANTIC_BLOCK_KEY, DELETION_SEMANTIC_CASCADE_KEY],
    );
    assert_enum_variants_rewritten_to_declared_side(
        &holons,
        OPERATOR_CATEGORY_KEY,
        &[OPERATOR_CATEGORY_EQUALITY_KEY, OPERATOR_CATEGORY_ORDERING_KEY],
    );

    info!("verified core schema descriptor subtype access");
}

/// Verifies value-descriptor semantic dispatch over loaded MAP core schema data.
pub async fn execute_verify_core_schema_value_semantics(state: &mut TestExecutionState) {
    let holons = loaded_holons(state, "verify_core_schema_value_semantics").await;

    let equals = OperatorDescriptor::from_holon(find_holon_by_key(&holons, "EqualsOperator"));
    let less_than = OperatorDescriptor::from_holon(find_holon_by_key(&holons, "LessThanOperator"));

    assert_eq!(equals.arity().expect("EqualsOperator arity"), 2);
    assert_eq!(
        equals.operator_category().expect("EqualsOperator operator_category"),
        OperatorCategory::Equality
    );

    let integer = ValueDescriptor::from_holon(find_holon_by_key(&holons, "IntegerValueType"));
    let integer_operator_names = operator_type_names(integer.supported_operators());
    assert_contains(&integer_operator_names, "EqualsOperator");
    assert_contains(&integer_operator_names, "LessThanOperator");
    assert!(integer.supports_operator(&equals).expect("IntegerValueType supports EqualsOperator"));
    assert!(integer
        .supports_operator(&less_than)
        .expect("IntegerValueType supports LessThanOperator"));
    assert!(integer
        .apply_operator(
            &equals,
            &BaseValue::IntegerValue(MapInteger(3)),
            &BaseValue::IntegerValue(MapInteger(3)),
        )
        .expect("IntegerValueType EqualsOperator execution"));
    assert!(integer
        .apply_operator(
            &less_than,
            &BaseValue::IntegerValue(MapInteger(2)),
            &BaseValue::IntegerValue(MapInteger(5)),
        )
        .expect("IntegerValueType LessThanOperator execution"));

    let boolean = ValueDescriptor::from_holon(find_holon_by_key(&holons, "BooleanValueType"));
    assert!(!boolean
        .supports_operator(&less_than)
        .expect("BooleanValueType does not support LessThanOperator"));
    assert!(matches!(
        boolean.apply_operator(
            &less_than,
            &BaseValue::BooleanValue(MapBoolean(false)),
            &BaseValue::BooleanValue(MapBoolean(true)),
        ),
        Err(HolonError::UnsupportedOperator { operator, value_type, .. })
            if operator == "LessThanOperator" && value_type == "BooleanValueType"
    ));

    let operator_category =
        ValueDescriptor::from_holon(find_holon_by_key(&holons, "OperatorCategory"));
    operator_category
        .is_valid(&BaseValue::EnumValue(MapEnumValue(MapString("Equality".to_string()))))
        .expect("OperatorCategory Equality variant should validate");
    assert!(matches!(
        operator_category.is_valid(&BaseValue::EnumValue(MapEnumValue(MapString(
            "NotARealVariant".to_string()
        )))),
        Err(HolonError::EnumVariantNotInSchema { variant, value_type, .. })
            if variant == "NotARealVariant" && value_type == "OperatorCategory"
    ));

    info!("verified core schema value semantics");
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

fn new_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    type_kind: TypeKind,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = context.mutation().new_holon(Some(MapString(key.to_string())))?;
    descriptor
        .with_property_value(CorePropertyTypeName::TypeName, type_name)?
        .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
        .with_property_value(CorePropertyTypeName::TypeKind, type_kind.as_schema_key())?
        .with_property_value(CorePropertyTypeName::AllowsAdditionalProperties, false)?
        .with_property_value(CorePropertyTypeName::AllowsAdditionalRelationships, false)?;
    Ok(descriptor)
}

fn dance_type_names(descriptors: Result<Vec<DanceDescriptor>, HolonError>) -> Vec<DanceName> {
    descriptors
        .expect("dance descriptor list")
        .into_iter()
        .map(|descriptor| descriptor.dance_name().expect("dance_name"))
        .collect()
}

fn property_type_names(descriptors: Result<Vec<PropertyDescriptor>, HolonError>) -> Vec<String> {
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

fn operator_type_names(descriptors: Result<Vec<OperatorDescriptor>, HolonError>) -> Vec<String> {
    descriptors
        .expect("operator descriptor list")
        .into_iter()
        .map(|descriptor| descriptor.operator_name().expect("operator descriptor name").to_string())
        .collect()
}

fn related_holon_keys(holon: &HolonReference, relationship_name: &str) -> Vec<String> {
    let members_handle = holon
        .related_holons(RelationshipName(MapString::from(relationship_name)))
        .unwrap_or_else(|error| panic!("related_holons({relationship_name}) failed: {error:?}"));
    let members = members_handle.read().unwrap_or_else(|error| {
        panic!("related_holons({relationship_name}) lock failed: {error:?}")
    });

    members
        .get_members()
        .iter()
        .map(|member| {
            member
                .key()
                .unwrap_or_else(|error| {
                    panic!("related_holons({relationship_name}) member key failed: {error:?}")
                })
                .unwrap_or_else(|| panic!("related_holons({relationship_name}) member missing key"))
                .0
        })
        .collect()
}

fn assert_enum_variants_rewritten_to_declared_side(
    holons: &HolonCollection,
    enum_value_key: &str,
    expected_variant_keys: &[&str],
) {
    let enum_value = find_holon_by_key(holons, enum_value_key);
    let variant_keys = related_holon_keys(&enum_value, VARIANTS_RELATIONSHIP);
    for expected_variant_key in expected_variant_keys {
        assert_contains(&variant_keys, expected_variant_key);
    }
}

fn assert_contains(values: &[String], expected: &str) {
    assert!(
        values.iter().any(|actual| actual == expected),
        "expected {values:?} to contain {expected}"
    );
}

fn assert_description_contains(
    description: Result<Option<MapString>, HolonError>,
    expected_fragment: &str,
) {
    let description = description.expect("descriptor description lookup").unwrap_or_else(|| {
        panic!("expected descriptor description containing {expected_fragment}")
    });
    assert!(
        description.0.contains(expected_fragment),
        "expected descriptor description {:?} to contain {:?}",
        description,
        expected_fragment
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
