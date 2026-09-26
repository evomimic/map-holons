use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};

const QUERY_DESCRIPTOR_KEY: &str = "Query.HolonType";
const SEED_HOLONS_DESCRIPTOR_KEY: &str = "SeedHolons.HolonType";
const EXPAND_DESCRIPTOR_KEY: &str = "Expand.HolonType";

const SEED_EXPRESSION_KEY: &str = "SeedHolons.Qry2Seed";
const CHAINED_SEED_EXPRESSION_KEY: &str = "SeedHolons.Qry2Chained";
const CHAINED_EXPAND_EXPRESSION_KEY: &str = "Expand.Qry2ChainedAuthoredBy";
const SEED_QUERY_KEY: &str = "Query.Qry2Seed";
const CHAIN_QUERY_KEY: &str = "Query.Qry2Chain";

/// QRY2a (issue #715) vertical slice: `SeedHolons` executes end to end.
///
/// 1. Resolve the regenerated `Query`, `SeedHolons`, and `Expand` descriptors by
///    bounded key lookup (proves the bootstrap carries the QRY2 schema; no
///    `GetAllHolons`), then stage, describe, and commit:
///    - `SeedHolons.Qry2Seed` and `Query.Qry2Seed` rooted at it;
///    - `SeedHolons.Qry2Chained -Next-> Expand.Qry2ChainedAuthoredBy` and
///      `Query.Qry2Chain` rooted at the seed.
/// 2. In a fresh transaction, resolve the committed definitions by key (the
///    caller-side lookup outside QueryCore) and execute, on both routes:
///    - `Query.Qry2Seed` with no input: `Complete`, result = the focal space's
///      `Owns` targets in storage order, including the committed definitions;
///    - `Query.Qry2Seed` with an input collection: the contract error
///      (`InvalidParameter`), not an ignored operand;
///    - `Query.Qry2Chain` with no input: `NotImplemented` — a root carrying
///      `Next` is refused rather than truncated to its root.
pub fn query_qry2_seed_expand_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_qry2_seed_expand",
        "QRY2a: SeedHolons expands the focal space's Owns through the direct and QueryDance \
         routes; supplied input and Next are refused explicitly",
    );

    // --- Build and commit the Query definitions ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY2 Query definitions".to_string()),
    )?;

    let query_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, QUERY_DESCRIPTOR_KEY)?;
    let seed_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        SEED_HOLONS_DESCRIPTOR_KEY,
    )?;
    let expand_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        EXPAND_DESCRIPTOR_KEY,
    )?;

    let seed_expression = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        SEED_EXPRESSION_KEY,
        PropertyMap::new(),
        &seed_type,
        "SeedHolons",
    )?;
    stage_query(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        SEED_QUERY_KEY,
        "QRY2 seed query",
        &query_type,
        seed_expression,
    )?;

    let mut expand_properties = PropertyMap::new();
    expand_properties.insert(
        "ExpansionRelationshipName".to_property_name(),
        MapString("AuthoredBy".to_string()).to_base_value(),
    );
    let chained_expand = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        CHAINED_EXPAND_EXPRESSION_KEY,
        expand_properties,
        &expand_type,
        "Expand",
    )?;
    let chained_seed = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        CHAINED_SEED_EXPRESSION_KEY,
        PropertyMap::new(),
        &seed_type,
        "SeedHolons",
    )?;
    let chained_seed = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        chained_seed,
        RelationshipName(MapString("Next".to_string())),
        vec![chained_expand],
        None,
        Some("Relate chained seed --Next--> Expand (chaining is refused in QRY2)".to_string()),
    )?;
    stage_query(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        CHAIN_QUERY_KEY,
        "QRY2 chained query",
        &query_type,
        chained_seed,
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit the QRY2 Query definitions".to_string()),
    )?;
    test_case.add_match_saved_content_step()?;

    // --- Execute over the committed definitions in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY2 query execution".to_string()),
    )?;

    let saved_seed_query =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, SEED_QUERY_KEY)?;
    let saved_chain_query =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, CHAIN_QUERY_KEY)?;
    let saved_seed_expression =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, SEED_EXPRESSION_KEY)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        test_case.add_execute_query_step(
            saved_seed_query.clone(),
            QueryInputSpec::None,
            route,
            QueryExpectation::OwnsOfFocalSpace {
                must_include: vec![saved_seed_query.clone(), saved_seed_expression.clone()],
            },
            Some(format!(
                "SeedHolons via {route:?}: Complete, result = focal space Owns in storage order"
            )),
        )?;
        test_case.add_execute_query_step(
            saved_seed_query.clone(),
            QueryInputSpec::Collection(vec![saved_seed_expression.clone()]),
            route,
            QueryExpectation::Error(HolonErrorKind::InvalidParameter),
            Some(format!("SeedHolons via {route:?} with a supplied input is a contract error")),
        )?;
        test_case.add_execute_query_step(
            saved_chain_query.clone(),
            QueryInputSpec::None,
            route,
            QueryExpectation::Error(HolonErrorKind::NotImplemented),
            Some(format!("Root with Next via {route:?} is refused with NotImplemented")),
        )?;
    }

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

fn lookup_by_key(
    test_case: &mut DancesTestCase,
    fixture_context: &std::sync::Arc<TransactionContext>,
    fixture_holons: &mut FixtureHolons,
    key: &str,
) -> Result<TestReference, HolonError> {
    let stub = fixture_context.mutation().new_holon(Some(MapString(key.to_string())))?;
    test_case.add_lookup_saved_holon_by_key_step(
        fixture_holons,
        stub,
        MapString(key.to_string()),
        None,
        None,
    )
}

/// Creates, stages, and describes one definition holon.
fn stage_described(
    test_case: &mut DancesTestCase,
    fixture_context: &std::sync::Arc<TransactionContext>,
    fixture_holons: &mut FixtureHolons,
    key: &str,
    properties: PropertyMap,
    descriptor: &TestReference,
    descriptor_label: &str,
) -> Result<TestReference, HolonError> {
    let source = fixture_context.mutation().new_holon(Some(MapString(key.to_string())))?;
    let token = test_case.add_new_holon_step(
        fixture_holons,
        source,
        properties,
        Some(MapString(key.to_string())),
        None,
        None,
    )?;
    let token = test_case.add_stage_holon_step(fixture_holons, token, None, None)?;
    test_case.add_add_related_holons_step(
        fixture_holons,
        token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![descriptor.clone()],
        None,
        Some(format!("Describe {key} by {descriptor_label}.HolonType")),
    )
}

/// Creates, stages, and describes a `Query` rooted at `root_expression`.
fn stage_query(
    test_case: &mut DancesTestCase,
    fixture_context: &std::sync::Arc<TransactionContext>,
    fixture_holons: &mut FixtureHolons,
    key: &str,
    name: &str,
    query_type: &TestReference,
    root_expression: TestReference,
) -> Result<TestReference, HolonError> {
    let mut properties = PropertyMap::new();
    properties.insert("QueryName".to_property_name(), MapString(name.to_string()).to_base_value());
    let token = stage_described(
        test_case,
        fixture_context,
        fixture_holons,
        key,
        properties,
        query_type,
        "Query",
    )?;
    test_case.add_add_related_holons_step(
        fixture_holons,
        token,
        RelationshipName(MapString("RootExpression".to_string())),
        vec![root_expression],
        None,
        Some(format!("Relate {key} --RootExpression--> root expression")),
    )
}
