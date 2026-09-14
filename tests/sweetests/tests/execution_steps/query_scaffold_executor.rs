//! Executor for `DanceTestStep::ExecuteQueryScaffold` (QRY1, issue #655).
//!
//! Drives the descriptor-backed Query runtime scaffold over an already committed
//! `Query` definition, either directly (`QueryReference::begin_execution` /
//! `run`) or through the narrow `QueryDance` branch of `execute_dance_v2`
//! (`TransactionAction::DanceV2`). Every runtime record the scaffold creates is
//! transient; nothing here is staged or committed.
//!
//! The input carrier built for the Dance route is a harness-side transport for
//! the explicit runtime input (a transient holon described by
//! `HolonCollection.HolonType` whose `CollectionMembers` are the input), not a
//! query feature.

use std::sync::Arc;

use holons_core::dances::DanceInvocation;
use holons_core::query_layer::query_core::{QueryExecution, QueryReference};
use holons_prelude::prelude::*;
use holons_test::{QueryScaffoldRoute, ResolveBy, TestExecutionState, TestReference};
use map_commands_contract::{MapCommand, TransactionAction, TransactionCommand};
use tracing::info;
use type_names::{
    QueryDanceRelationshipTypeName, QueryPropertyTypeName, QueryRelationshipTypeName,
};

const HOLON_COLLECTION_DESCRIPTOR_KEY: &str = "HolonCollection.HolonType";
const HOLON_SPACE_DESCRIPTOR_KEY: &str = "HolonSpace.HolonType";
const DANCE_INVOCATION_DESCRIPTOR_KEY: &str = "DanceInvocation.HolonType";
const QUERY_DANCE_REQUEST_DESCRIPTOR_KEY: &str = "QueryDanceRequest.HolonType";
const QUERY_DANCE_NAME: &str = "QueryDance";
const DANCE_RESPONSE_KEY: &str = "dance-response";

pub async fn execute_query_scaffold(
    state: &mut TestExecutionState,
    query: TestReference,
    input_members: Vec<TestReference>,
    route: QueryScaffoldRoute,
    expected_error: Option<HolonErrorKind>,
) {
    info!("--- TEST STEP: Execute QRY1 query scaffold via {route:?} ---");
    assert!(
        expected_error.is_some(),
        "QRY1 has no success path; the fixture must state the expected error kind"
    );
    let context = state.context();

    // Tokens come from `LookupSavedHolonByKey`, which records the resolved
    // SmartReference under the token's expected snapshot.
    let query_reference =
        state.resolve_execution_reference(&context, ResolveBy::Expected, &query).unwrap();
    let members =
        state.resolve_execution_references(&context, ResolveBy::Expected, &input_members).unwrap();

    match route {
        QueryScaffoldRoute::Direct => {
            execute_direct(&context, query_reference, members, expected_error)
        }
        QueryScaffoldRoute::QueryDance => {
            execute_query_dance(state, &context, query_reference, members, true, expected_error)
                .await
        }
        QueryScaffoldRoute::QueryDanceWithoutInitialInput => {
            execute_query_dance(state, &context, query_reference, members, false, expected_error)
                .await
        }
    }
}

// ---------------------------------------------------------------------------
// Direct route
// ---------------------------------------------------------------------------

fn execute_direct(
    context: &Arc<TransactionContext>,
    query_reference: HolonReference,
    members: Vec<HolonReference>,
    expected_error: Option<HolonErrorKind>,
) {
    let query = QueryReference::new(query_reference.clone())
        .expect("committed Query holon should wrap as a QueryReference");
    let root_expression =
        single_related(&query_reference, QueryRelationshipTypeName::RootExpression);

    let mut input = HolonCollection::new_transient();
    input.add_references(members.clone()).unwrap();
    let input_count = members.len();

    // begin_execution: records exist, are Pending, and carry the definition/input links.
    let execution = query
        .begin_execution(context, input, Vec::new())
        .expect("begin_execution should create the transient runtime records");
    assert_scaffold_shape(&execution, &query_reference, &root_expression, input_count);
    assert_status(execution.instance().clone().into(), "Pending");
    assert_status(execution.root_execution().clone().into(), "Pending");

    // Keep handles: `run` consumes the execution.
    let instance: HolonReference = execution.instance().clone().into();
    let root_execution: HolonReference = execution.root_execution().clone().into();

    // run: reaches the unimplemented operator boundary.
    let result = execution.run(context);
    match result {
        Ok(collection) => panic!(
            "QRY1 run must not succeed; got a collection with {} members",
            collection.get_members().len()
        ),
        Err(error) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected_error,
                "query scaffold (direct): unexpected error {error:?}"
            );
        }
    }

    assert_status(instance.clone(), "Failed");
    assert_status(root_execution.clone(), "Failed");
    assert_related_count(&instance, "ExecutionResult", 0);
    assert_related_count(&root_execution, "Result", 0);
    assert_related_count(&root_execution, "RuntimeParameters", 0);

    // Definition/runtime separation: nothing landed on the reusable definitions.
    assert_no_runtime_state(&query_reference, "Query");
    assert_no_runtime_state(&root_expression, "root QueryExpression");

    info!("Success! QRY1 direct scaffold recorded Pending -> Failed and returned NotImplemented");
}

fn assert_scaffold_shape(
    execution: &QueryExecution,
    query_reference: &HolonReference,
    root_expression: &HolonReference,
    input_count: usize,
) {
    let instance: HolonReference = execution.instance().clone().into();
    let root_execution: HolonReference = execution.root_execution().clone().into();

    assert_same_holon(
        &single_related(&instance, QueryRelationshipTypeName::ExecutesQuery),
        query_reference,
        "ExecutionInstance.ExecutesQuery",
    );
    assert_same_holon(
        &single_related(&instance, QueryRelationshipTypeName::ExpressionExecutions),
        &root_execution,
        "ExecutionInstance.ExpressionExecutions",
    );
    assert_same_holon(
        &single_related(&root_execution, QueryRelationshipTypeName::ExecutesExpression),
        root_expression,
        "QueryExpressionExecution.ExecutesExpression",
    );

    let carrier = single_related(&root_execution, QueryRelationshipTypeName::Input);
    assert!(carrier.is_transient(), "input carrier must be a transient holon");
    assert_related_count(&carrier, CoreRelationshipTypeName::CollectionMembers, input_count);
}

// ---------------------------------------------------------------------------
// QueryDance route
// ---------------------------------------------------------------------------

async fn execute_query_dance(
    state: &mut TestExecutionState,
    context: &Arc<TransactionContext>,
    query_reference: HolonReference,
    members: Vec<HolonReference>,
    with_initial_input: bool,
    expected_error: Option<HolonErrorKind>,
) {
    // Request: RequestedQuery + (optionally) InitialInput carrier.
    let mut request = context
        .mutation()
        .new_holon(Some(MapString("qry1-query-dance-request".to_string())))
        .unwrap();
    request
        .with_descriptor(
            resolve_core_descriptor(context, QUERY_DANCE_REQUEST_DESCRIPTOR_KEY).unwrap(),
        )
        .unwrap();
    request
        .add_related_holons(QueryDanceRelationshipTypeName::RequestedQuery, vec![query_reference])
        .unwrap();
    if with_initial_input {
        let carrier = build_input_carrier(context, members);
        request
            .add_related_holons(QueryDanceRelationshipTypeName::InitialInput, vec![carrier.into()])
            .unwrap();
    }

    // Invocation: DanceName + Request + AffordingHolon (a HolonSpace).
    let mut invocation = context
        .mutation()
        .new_holon(Some(MapString("qry1-query-dance-invocation".to_string())))
        .unwrap();
    invocation
        .with_descriptor(resolve_core_descriptor(context, DANCE_INVOCATION_DESCRIPTOR_KEY).unwrap())
        .unwrap();
    invocation
        .with_property_value(
            CorePropertyTypeName::DanceName,
            MapString(QUERY_DANCE_NAME.to_string()),
        )
        .unwrap()
        .add_related_holons(CoreRelationshipTypeName::Request, vec![request.into()])
        .unwrap()
        .add_related_holons(
            CoreRelationshipTypeName::AffordingHolon,
            vec![affording_space(context)],
        )
        .unwrap();
    let invocation = DanceInvocation::new(invocation.into())
        .expect("transient invocation should wrap as a DanceInvocation");

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::DanceV2 { invocation },
    });
    let result = state.dispatch_command(command, "query_dance_v2").await;

    match result {
        Ok(other) => panic!("QueryDance must not succeed in QRY1; got {other:?}"),
        Err(error) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected_error,
                "query scaffold (QueryDance): unexpected error {error:?}"
            );
        }
    }

    // No QueryDanceResponse was minted.
    assert!(
        matches!(
            context
                .lookup()
                .get_transient_holon_by_base_key(&MapString(DANCE_RESPONSE_KEY.to_string())),
            Err(HolonError::HolonNotFound(_))
        ),
        "no dance-response holon should be minted on the QRY1 failure path"
    );

    info!("Success! QueryDance routed to the QRY1 seam and propagated the expected error");
}

fn build_input_carrier(
    context: &Arc<TransactionContext>,
    members: Vec<HolonReference>,
) -> TransientReference {
    let mut carrier =
        context.mutation().new_holon(Some(MapString("qry1-initial-input".to_string()))).unwrap();
    carrier
        .with_descriptor(resolve_core_descriptor(context, HOLON_COLLECTION_DESCRIPTOR_KEY).unwrap())
        .unwrap();
    if !members.is_empty() {
        carrier.add_related_holons(CoreRelationshipTypeName::CollectionMembers, members).unwrap();
    }
    carrier
}

/// The holon affording `QueryDance` (`DanceAffordedBy -> HolonSpace.HolonType`).
///
/// Prefers the transaction's own space holon; falls back to a transient holon
/// described as `HolonSpace` when the anchor is not described.
fn affording_space(context: &Arc<TransactionContext>) -> HolonReference {
    if let Ok(Some(space)) = context.get_space_holon() {
        if let Ok(descriptor) = space.holon_descriptor() {
            if descriptor.header().type_name().map(|name| name.0 == "HolonSpace").unwrap_or(false) {
                info!("QueryDance affording holon: the transaction's HolonSpace anchor");
                return space;
            }
        }
    }
    info!("QueryDance affording holon: transient holon described as HolonSpace (anchor not described)");
    let mut space =
        context.mutation().new_holon(Some(MapString("qry1-affording-space".to_string()))).unwrap();
    space
        .with_descriptor(resolve_core_descriptor(context, HOLON_SPACE_DESCRIPTOR_KEY).unwrap())
        .unwrap();
    space.into()
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

fn related_members<T: ToRelationshipName>(holon: &HolonReference, name: T) -> Vec<HolonReference> {
    holon.related_holons(name).unwrap().read().unwrap().get_members().clone()
}

fn single_related<T: ToRelationshipName + std::fmt::Debug + Clone>(
    holon: &HolonReference,
    name: T,
) -> HolonReference {
    let members = related_members(holon, name.clone());
    assert_eq!(members.len(), 1, "expected exactly one {name:?} member");
    members[0].clone()
}

fn assert_related_count<T: ToRelationshipName + std::fmt::Debug + Clone>(
    holon: &HolonReference,
    name: T,
    expected: usize,
) {
    assert_eq!(related_members(holon, name.clone()).len(), expected, "unexpected {name:?} count");
}

fn assert_same_holon(actual: &HolonReference, expected: &HolonReference, what: &str) {
    assert_eq!(
        actual.reference_id_string(),
        expected.reference_id_string(),
        "{what} should reference the expected holon"
    );
}

fn assert_status(record: HolonReference, expected: &str) {
    let value = record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap();
    let found = match value {
        Some(BaseValue::EnumValue(value)) => value.0 .0,
        Some(BaseValue::StringValue(value)) => value.0,
        other => panic!("ExecutionStatus should be an enum value, found {other:?}"),
    };
    assert_eq!(found, expected, "unexpected ExecutionStatus on {}", record.summarize().unwrap());
}

fn assert_no_runtime_state(definition: &HolonReference, what: &str) {
    assert!(
        definition.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap().is_none(),
        "{what} definition must carry no ExecutionStatus"
    );
    for relationship in ["Input", "Result", "RuntimeParameters", "ExecutionResult"] {
        assert_related_count(definition, relationship, 0);
    }
}
