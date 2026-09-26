use holons_prelude::prelude::*;
use holons_test::harness::helpers::UNIMPLEMENTED_QUERY_EXPRESSION_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, QueryExpectation, QueryInputSpec, QueryRoute,
    TestCaseInit,
};

const QUERY_DESCRIPTOR_KEY: &str = "Query.HolonType";
const ROOT_EXPRESSION_KEY: &str = "UnimplementedQueryExpression.Qry1Root";
const QUERY_KEY: &str = "Query.Qry1Scaffold";

/// QRY1 (issue #655) vertical slice: definition/runtime boundary and the
/// `NotImplemented` scaffold, reached directly and through `QueryDance`.
///
/// 1. Load the Sweettest-only `UnimplementedQueryExpression` schema.
/// 2. Resolve the `Query` and test-expression descriptors by bounded key lookup
///    (no `GetAllHolons`), then stage, describe, and commit one expression
///    instance and one `Query` whose `RootExpression` is that instance.
/// 3. In a fresh transaction, resolve the committed Query and expression by key
///    (the #679-style caller-side lookup, outside QueryCore) and drive the
///    scaffold:
///    - directly, with the expression as the explicit input and again with an
///      empty input;
///    - through `QueryDance`, and once more with a request omitting the
///      (optional) `InitialInput`, which still reaches the seam.
pub fn query_qry1_scaffold_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_qry1_scaffold",
        "QRY1: transient ExecutionInstance/QueryExpressionExecution scaffold reaches \
         NotImplemented directly and via QueryDance without touching the Query definition",
    );

    test_case.add_load_query_test_schema_step(None)?;

    // --- Build and commit the Query definition ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY1 Query definition".to_string()),
    )?;

    let query_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(QUERY_DESCRIPTOR_KEY.to_string())))?;
    let query_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        query_type_stub,
        MapString(QUERY_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;
    let expression_type_stub = fixture_context
        .mutation()
        .new_holon(Some(MapString(UNIMPLEMENTED_QUERY_EXPRESSION_DESCRIPTOR_KEY.to_string())))?;
    let expression_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        expression_type_stub,
        MapString(UNIMPLEMENTED_QUERY_EXPRESSION_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;

    // Root expression instance (test-only concrete QueryExpression).
    let expression_source =
        fixture_context.mutation().new_holon(Some(MapString(ROOT_EXPRESSION_KEY.to_string())))?;
    let expression_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        expression_source,
        PropertyMap::new(),
        Some(MapString(ROOT_EXPRESSION_KEY.to_string())),
        None,
        None,
    )?;
    let expression_token =
        test_case.add_stage_holon_step(&mut fixture_holons, expression_token, None, None)?;
    let expression_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        expression_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![expression_type_token],
        None,
        Some("Describe root expression by UnimplementedQueryExpression.HolonType".to_string()),
    )?;

    // Query definition with its required QueryName and RootExpression.
    let query_source =
        fixture_context.mutation().new_holon(Some(MapString(QUERY_KEY.to_string())))?;
    let mut query_properties = PropertyMap::new();
    query_properties.insert(
        "QueryName".to_property_name(),
        MapString("QRY1 scaffold query".to_string()).to_base_value(),
    );
    let query_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        query_source,
        query_properties,
        Some(MapString(QUERY_KEY.to_string())),
        None,
        None,
    )?;
    let query_token =
        test_case.add_stage_holon_step(&mut fixture_holons, query_token, None, None)?;
    let query_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        query_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![query_type_token],
        None,
        Some("Describe Query by Query.HolonType".to_string()),
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        query_token,
        RelationshipName(MapString("RootExpression".to_string())),
        vec![expression_token],
        None,
        Some("Relate Query --RootExpression--> root expression".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit the QRY1 Query definition".to_string()),
    )?;
    test_case.add_match_saved_content_step()?;

    // --- Drive the scaffold in a fresh transaction over the committed definition ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY1 scaffold execution".to_string()),
    )?;

    let saved_query_stub =
        fixture_context.mutation().new_holon(Some(MapString(QUERY_KEY.to_string())))?;
    let saved_query = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        saved_query_stub,
        MapString(QUERY_KEY.to_string()),
        None,
        Some(
            "Resolve the committed Query by key (caller-side lookup, outside QueryCore)"
                .to_string(),
        ),
    )?;
    let saved_expression_stub =
        fixture_context.mutation().new_holon(Some(MapString(ROOT_EXPRESSION_KEY.to_string())))?;
    let saved_expression = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        saved_expression_stub,
        MapString(ROOT_EXPRESSION_KEY.to_string()),
        None,
        Some("Resolve a committed holon by key to use as explicit query input".to_string()),
    )?;

    test_case.add_execute_query_step(
        saved_query.clone(),
        QueryInputSpec::Collection(vec![saved_expression.clone()]),
        QueryRoute::Direct,
        QueryExpectation::Error(HolonErrorKind::NotImplemented),
        Some(
            "Direct seam with one explicit input member: Pending -> Failed, NotImplemented"
                .to_string(),
        ),
    )?;
    test_case.add_execute_query_step(
        saved_query.clone(),
        QueryInputSpec::Collection(vec![]),
        QueryRoute::Direct,
        QueryExpectation::Error(HolonErrorKind::NotImplemented),
        Some(
            "Direct seam with an empty explicit input is valid runtime input (no enumeration)"
                .to_string(),
        ),
    )?;
    test_case.add_execute_query_step(
        saved_query.clone(),
        QueryInputSpec::Collection(vec![saved_expression]),
        QueryRoute::QueryDance,
        QueryExpectation::Error(HolonErrorKind::NotImplemented),
        Some("QueryDance routes to the same seam and propagates NotImplemented".to_string()),
    )?;
    // InitialInput is optional since QRY2 (source roots take none); an unknown
    // concrete expression passes the absent operand through and still fails at
    // the operator boundary rather than at the adapter.
    test_case.add_execute_query_step(
        saved_query,
        QueryInputSpec::None,
        QueryRoute::QueryDance,
        QueryExpectation::Error(HolonErrorKind::NotImplemented),
        Some(
            "QueryDance request without InitialInput reaches the seam and fails NotImplemented"
                .to_string(),
        ),
    )?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
