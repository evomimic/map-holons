//! Executor for `DanceTestStep::ExecuteQuery` (QRY1 #655 scaffold, QRY2 #715 operators).
//!
//! Drives a committed `Query` definition through the descriptor-backed Query
//! runtime, either directly (`QueryReference::begin_execution` / `run`) or
//! through the narrow `QueryDance` branch of `execute_dance_v2`
//! (`TransactionAction::DanceV2`), and asserts the step's expectation. Every
//! runtime record the runtime creates is transient; nothing here is staged or
//! committed.
//!
//! Focal space: both routes use the transaction's own `HolonSpace` anchor —
//! directly as a `FocalSpaceReference`, and on the Dance route as the
//! invocation's `AffordingHolon` (the adapter maps it to the same reference).
//!
//! Input: when the step supplies a collection, the harness creates one holon
//! described by `HolonCollection.HolonType` whose `CollectionMembers` are the
//! resolved references (`build_input_collection`) — the caller-side boundary
//! from #655 — and the runtime links `Input` to that same holon by identity.

use std::sync::Arc;

use holons_core::dances::DanceInvocation;
use holons_core::query_layer::query_core::{
    FocalSpaceReference, HolonCollectionReference, QueryExecution, QueryReference,
};
use holons_prelude::prelude::*;
use holons_test::{
    QueryExpectation, QueryInputSpec, QueryRoute, ResolveBy, TestExecutionState, TestReference,
};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::info;
use type_names::{
    QueryDanceRelationshipTypeName, QueryPropertyTypeName, QueryRelationshipTypeName,
};

const HOLON_COLLECTION_DESCRIPTOR_KEY: &str = "HolonCollection.HolonType";
const DANCE_INVOCATION_DESCRIPTOR_KEY: &str = "DanceInvocation.HolonType";
const QUERY_DANCE_REQUEST_DESCRIPTOR_KEY: &str = "QueryDanceRequest.HolonType";
const QUERY_DANCE_NAME: &str = "QueryDance";
const DANCE_RESPONSE_KEY: &str = "dance-response";

pub async fn execute_query(
    state: &mut TestExecutionState,
    query: TestReference,
    input: QueryInputSpec,
    route: QueryRoute,
    expectation: QueryExpectation,
) {
    info!("--- TEST STEP: Execute query via {route:?} ---");
    let context = state.context();

    // Tokens come from `LookupSavedHolonByKey`, which records the resolved
    // SmartReference under the token's expected snapshot.
    let query_reference =
        state.resolve_execution_reference(&context, ResolveBy::Expected, &query).unwrap();
    let resolved_input = match &input {
        QueryInputSpec::None => ResolvedInput::None,
        QueryInputSpec::Collection(members) => ResolvedInput::Collection(
            state.resolve_execution_references(&context, ResolveBy::Expected, members).unwrap(),
        ),
        QueryInputSpec::SingleHolon(member) => ResolvedInput::SingleHolon(
            state.resolve_execution_reference(&context, ResolveBy::Expected, member).unwrap(),
        ),
    };
    let expected = ResolvedExpectation::resolve(state, &context, expectation);

    match route {
        QueryRoute::Direct => execute_direct(&context, query_reference, resolved_input, expected),
        QueryRoute::QueryDance => {
            let members = match resolved_input {
                ResolvedInput::None => None,
                ResolvedInput::Collection(members) => Some(members),
                ResolvedInput::SingleHolon(_) => panic!(
                    "the single-holon convenience is direct-only; the Dance contract is \
                     collection-shaped"
                ),
            };
            execute_query_dance(state, &context, query_reference, members, expected).await
        }
    }
}

/// The step's collection operand with fixture tokens resolved to references.
enum ResolvedInput {
    None,
    Collection(Vec<HolonReference>),
    SingleHolon(HolonReference),
}

/// The step expectation with fixture tokens resolved to references.
enum ResolvedExpectation {
    Members(Vec<HolonReference>),
    OwnsOfFocalSpace { must_include: Vec<HolonReference> },
    Error(HolonErrorKind),
}

impl ResolvedExpectation {
    fn resolve(
        state: &TestExecutionState,
        context: &Arc<TransactionContext>,
        expectation: QueryExpectation,
    ) -> Self {
        let resolve = |tokens: &[TestReference]| {
            state.resolve_execution_references(context, ResolveBy::Expected, tokens).unwrap()
        };
        match expectation {
            QueryExpectation::Members(tokens) => Self::Members(resolve(&tokens)),
            QueryExpectation::OwnsOfFocalSpace { must_include } => {
                Self::OwnsOfFocalSpace { must_include: resolve(&must_include) }
            }
            QueryExpectation::Error(kind) => Self::Error(kind),
        }
    }

    fn expected_error(&self) -> Option<HolonErrorKind> {
        match self {
            Self::Error(kind) => Some(*kind),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Direct route
// ---------------------------------------------------------------------------

fn execute_direct(
    context: &Arc<TransactionContext>,
    query_reference: HolonReference,
    input: ResolvedInput,
    expected: ResolvedExpectation,
) {
    let query = QueryReference::new(query_reference.clone())
        .expect("committed Query holon should wrap as a QueryReference");
    let root_expression =
        single_related(&query_reference, QueryRelationshipTypeName::RootExpression);
    let space = affording_space(context);
    let focal_space = FocalSpaceReference::new(space.clone())
        .expect("the transaction's space holon should wrap as a FocalSpaceReference");

    // The harness builds the collection holon for explicit inputs (the
    // caller-side boundary); the convenience builds its own singleton inside
    // QueryCore, so there is no harness-held holon to compare by identity there.
    let single_source = match &input {
        ResolvedInput::SingleHolon(source) => Some(source.clone()),
        _ => None,
    };
    let input_collection: Option<HolonReference> = match input {
        ResolvedInput::None | ResolvedInput::SingleHolon(_) => None,
        ResolvedInput::Collection(members) => {
            let count = members.len();
            let collection: HolonReference = build_input_collection(context, members).into();
            assert_related_count(&collection, CoreRelationshipTypeName::CollectionMembers, count);
            Some(collection)
        }
    };
    let collection_input = input_collection.clone().map(|collection| {
        HolonCollectionReference::new(collection)
            .expect("harness collection holon should wrap as a HolonCollectionReference")
    });

    // begin_execution enforces the root input contract before any record exists.
    let begun = match single_source.clone() {
        Some(source) => query.begin_execution_for_holon(context, focal_space, source, Vec::new()),
        None => query.begin_execution(context, focal_space, collection_input, Vec::new()),
    };
    let execution = match begun {
        Ok(execution) => execution,
        Err(error) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected.expected_error(),
                "query (direct): begin_execution failed unexpectedly: {error:?}"
            );
            assert_no_runtime_state(&query_reference, "Query");
            assert_no_runtime_state(&root_expression, "root QueryExpression");
            info!("Success! begin_execution rejected the input contract violation");
            return;
        }
    };
    assert_shape(&execution, &query_reference, &root_expression, &space, input_collection.as_ref());
    if let Some(source) = &single_source {
        assert_singleton_input(&execution, source);
    }
    assert_status(execution.instance().clone().into(), "Pending");
    assert_status(execution.root_execution().clone().into(), "Pending");

    // Keep a handle: `run` consumes the execution. The per-step records are
    // reached through the instance's `ExpressionExecutions`, since a chain
    // creates them as it goes.
    let instance: HolonReference = execution.instance().clone().into();

    match (execution.run(), expected) {
        (Ok(result), ResolvedExpectation::Error(kind)) => panic!(
            "query (direct): expected {kind:?}, got a result with {} member(s)",
            collection_members(result.as_holon_reference()).len()
        ),
        (Ok(result), expected) => {
            assert_success(context, &instance, result.as_holon_reference(), expected);
        }
        (Err(error), expected) => {
            assert_failure(&instance, &error, expected.expected_error(), "direct");
        }
    }

    // Definition/runtime separation: nothing landed on the reusable definitions.
    assert_no_runtime_state(&query_reference, "Query");
    assert_no_runtime_state(&root_expression, "root QueryExpression");
    info!("Success! direct query route matched the expectation");
}

fn assert_shape(
    execution: &QueryExecution,
    query_reference: &HolonReference,
    root_expression: &HolonReference,
    space: &HolonReference,
    input_collection: Option<&HolonReference>,
) {
    let instance: HolonReference = execution.instance().clone().into();
    let root_execution: HolonReference = execution.root_execution().clone().into();

    assert_same_holon(
        &single_related(&instance, QueryRelationshipTypeName::ExecutesQuery),
        query_reference,
        "ExecutionInstance.ExecutesQuery",
    );
    assert_same_holon(
        &single_related(&instance, QueryRelationshipTypeName::FocalSpace),
        space,
        "ExecutionInstance.FocalSpace",
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

    // Identity: Input is the caller's collection holon, not a copy of it — or absent.
    if let Some(collection) = input_collection {
        let input = single_related(&root_execution, QueryRelationshipTypeName::Input);
        assert_same_holon(&input, collection, "QueryExpressionExecution.Input");
    }
}

/// The single-holon convenience must record a one-member collection holon as
/// `Input` — never the source holon itself.
fn assert_singleton_input(execution: &QueryExecution, source: &HolonReference) {
    let root_execution: HolonReference = execution.root_execution().clone().into();
    let input = single_related(&root_execution, QueryRelationshipTypeName::Input);
    assert_ne!(
        input.reference_id_string(),
        source.reference_id_string(),
        "Input must be a collection holon, not the source holon"
    );
    assert_eq!(
        input.holon_descriptor().unwrap().header().type_name().unwrap().0,
        "HolonCollection",
        "the convenience wraps its source in a HolonCollection holon"
    );
    let members = related_members(&input, CoreRelationshipTypeName::CollectionMembers);
    assert_eq!(ids_of(&members), ids_of(&[source.clone()]), "singleton member is the source");
}

// ---------------------------------------------------------------------------
// QueryDance route
// ---------------------------------------------------------------------------

async fn execute_query_dance(
    state: &mut TestExecutionState,
    context: &Arc<TransactionContext>,
    query_reference: HolonReference,
    input_members: Option<Vec<HolonReference>>,
    expected: ResolvedExpectation,
) {
    // Request: RequestedQuery + (optionally) the InitialInput collection holon.
    let mut request =
        context.mutation().new_holon(Some(MapString("query-dance-request".to_string()))).unwrap();
    request
        .with_descriptor(
            resolve_core_descriptor(context, QUERY_DANCE_REQUEST_DESCRIPTOR_KEY).unwrap(),
        )
        .unwrap();
    request
        .add_related_holons(QueryDanceRelationshipTypeName::RequestedQuery, vec![query_reference])
        .unwrap();
    if let Some(members) = input_members {
        let collection = build_input_collection(context, members);
        request
            .add_related_holons(
                QueryDanceRelationshipTypeName::InitialInput,
                vec![collection.into()],
            )
            .unwrap();
    }

    // Invocation: DanceName + Request + AffordingHolon (a HolonSpace).
    let space = affording_space(context);
    let mut invocation = context
        .mutation()
        .new_holon(Some(MapString("query-dance-invocation".to_string())))
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
        .add_related_holons(CoreRelationshipTypeName::AffordingHolon, vec![space.clone()])
        .unwrap();
    let invocation = DanceInvocation::new(invocation.into())
        .expect("transient invocation should wrap as a DanceInvocation");

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::DanceV2 { invocation },
    });
    let result = state.dispatch_command(command, "query_dance_v2").await;

    match (result, expected) {
        (Ok(other), ResolvedExpectation::Error(kind)) => {
            panic!("query (QueryDance): expected {kind:?}, got {other:?}")
        }
        (Ok(MapResult::Reference(response)), expected) => {
            // The transient runtime records carry fixed keys and are not reachable
            // from the result holon (inverse links materialize only on commit), so
            // this route asserts through the response body alone; record shape and
            // status are covered by the direct route and the QueryCore unit tests,
            // and both routes share `begin_execution` / `run`.
            assert_eq!(
                response.key().unwrap(),
                Some(MapString(DANCE_RESPONSE_KEY.to_string())),
                "DanceV2 should return the dance-response holon"
            );
            let body = single_related(&response, CoreRelationshipTypeName::ResponseBody);
            assert_result_members(context, &body, expected);
            info!("Success! QueryDance returned the result collection as the response body");
        }
        (Ok(other), _) => {
            panic!("query (QueryDance): expected MapResult::Reference, got {other:?}")
        }
        (Err(error), expected) => {
            let actual = HolonErrorKind::from(&error);
            assert_eq!(
                Some(actual),
                expected.expected_error(),
                "query (QueryDance): unexpected error {error:?}"
            );
            info!("Success! QueryDance propagated the expected error");
        }
    }
}

// ---------------------------------------------------------------------------
// Shared outcome assertions
// ---------------------------------------------------------------------------

fn assert_success(
    context: &Arc<TransactionContext>,
    instance: &HolonReference,
    result: &HolonReference,
    expected: ResolvedExpectation,
) {
    assert_status(instance.clone(), "Complete");

    // One record per executed step, in chain order (`ExpressionExecutions` is
    // ordered). A single-step execution is just a chain of length one.
    let steps = related_members(instance, QueryRelationshipTypeName::ExpressionExecutions);
    assert!(!steps.is_empty(), "a successful execution records at least one step");
    for step in &steps {
        assert_status(step.clone(), "Complete");
    }

    // Each non-root step consumes its predecessor's Result holon by identity.
    for pair in steps.windows(2) {
        assert_same_holon(
            &single_related(&pair[1], QueryRelationshipTypeName::Input),
            &single_related(&pair[0], QueryRelationshipTypeName::Result),
            "non-root QueryExpressionExecution.Input",
        );
    }

    // The value returned is the final step's Result, and the instance records it.
    let last_step = steps.last().expect("checked non-empty");
    assert_same_holon(
        &single_related(last_step, QueryRelationshipTypeName::Result),
        result,
        "final QueryExpressionExecution.Result",
    );
    assert_same_holon(
        &single_related(instance, QueryRelationshipTypeName::ExecutionResult),
        result,
        "ExecutionInstance.ExecutionResult",
    );
    assert_result_members(context, result, expected);
}

fn assert_result_members(
    context: &Arc<TransactionContext>,
    result: &HolonReference,
    expected: ResolvedExpectation,
) {
    let members = collection_members(result);
    match expected {
        ResolvedExpectation::Members(expected_members) => {
            assert_eq!(
                ids_of(&members),
                ids_of(&expected_members),
                "result members should match the expectation in order"
            );
        }
        ResolvedExpectation::OwnsOfFocalSpace { must_include } => {
            // Oracle: the host's own view of the space's Owns relationship. The
            // conductor is not reachable from an execution step, so this checks
            // that the runtime neither reorders nor dedupes what storage returned
            // and that the committed fixture holons are present.
            let space = affording_space(context);
            let owns = related_members(&space, CoreRelationshipTypeName::Owns);
            assert!(!members.is_empty(), "SeedHolons over a populated space is not empty");
            assert_eq!(ids_of(&members), ids_of(&owns), "SeedHolons result must be Owns in order");
            let member_ids = ids_of(&members);
            for required in must_include {
                let id = required.holon_id().unwrap();
                assert!(member_ids.contains(&id), "SeedHolons result must include {id:?}");
            }
        }
        ResolvedExpectation::Error(kind) => panic!("expected {kind:?}, got a successful result"),
    }
}

fn assert_failure(
    instance: &HolonReference,
    error: &HolonError,
    expected_kind: Option<HolonErrorKind>,
    route: &str,
) {
    let actual = HolonErrorKind::from(error);
    assert_eq!(Some(actual), expected_kind, "query ({route}): unexpected error {error:?}");
    assert_status(instance.clone(), "Failed");
    assert_related_count(instance, QueryRelationshipTypeName::ExecutionResult, 0);

    // Only the step that failed is Failed; steps that legitimately completed
    // earlier in the chain keep their Complete status.
    let steps = related_members(instance, QueryRelationshipTypeName::ExpressionExecutions);
    if let Some(failed_step) = steps.last() {
        assert_status(failed_step.clone(), "Failed");
        assert_related_count(failed_step, QueryRelationshipTypeName::Result, 0);
    }
}

/// Creates the caller-side explicit input: a transient holon described by
/// `HolonCollection.HolonType` whose `CollectionMembers` are `members`.
fn build_input_collection(
    context: &Arc<TransactionContext>,
    members: Vec<HolonReference>,
) -> TransientReference {
    let mut collection = context
        .mutation()
        .new_holon(Some(MapString("harness-input-collection".to_string())))
        .unwrap();
    collection
        .with_descriptor(resolve_core_descriptor(context, HOLON_COLLECTION_DESCRIPTOR_KEY).unwrap())
        .unwrap();
    if !members.is_empty() {
        collection
            .add_related_holons(CoreRelationshipTypeName::CollectionMembers, members)
            .unwrap();
    }
    collection
}

/// The holon affording `QueryDance` (`DanceAffordedBy -> HolonSpace.HolonType`)
/// and the direct route's focal space: the transaction's own HolonSpace anchor.
fn affording_space(context: &Arc<TransactionContext>) -> HolonReference {
    context
        .get_space_holon()
        .expect("space holon lookup should succeed")
        .expect("transaction should have a HolonSpace anchor")
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

fn related_members<T: ToRelationshipName>(holon: &HolonReference, name: T) -> Vec<HolonReference> {
    holon.related_holons(name).unwrap().read().unwrap().get_members().clone()
}

fn collection_members(collection: &HolonReference) -> Vec<HolonReference> {
    related_members(collection, CoreRelationshipTypeName::CollectionMembers)
}

fn ids_of(members: &[HolonReference]) -> Vec<HolonId> {
    members.iter().map(|member| member.holon_id().unwrap()).collect()
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
    for relationship in ["Input", "Result", "RuntimeParameters", "ExecutionResult", "FocalSpace"] {
        assert_related_count(definition, relationship, 0);
    }
}
