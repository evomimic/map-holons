use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
};
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
const AUTHORED_BY_EXPRESSION_KEY: &str = "Expand.Qry2AuthoredBy";
const AUTHOR_OF_EXPRESSION_KEY: &str = "Expand.Qry2AuthorOf";
const BOGUS_EXPRESSION_KEY: &str = "Expand.Qry2Bogus";
const SEED_QUERY_KEY: &str = "Query.Qry2Seed";
const CHAIN_QUERY_KEY: &str = "Query.Qry2Chain";
const AUTHORED_BY_QUERY_KEY: &str = "Query.Qry2Expand";
const AUTHOR_OF_QUERY_KEY: &str = "Query.Qry2Inverse";
const BOGUS_QUERY_KEY: &str = "Query.Qry2Bogus";

/// Inverse of `AuthoredBy` in the Book/Person test schema.
const PERSON_TO_BOOK_RELATIONSHIP: &str = "AuthorOf";
const UNKNOWN_RELATIONSHIP: &str = "NoSuchRelationship";

/// Book/Person instances staged for the `Expand` cases. `Qry2.Book.C` has no
/// authors on purpose: a legal zero-target expansion contributes nothing.
const BOOK_A_KEY: &str = "Qry2.Book.A";
const BOOK_B_KEY: &str = "Qry2.Book.B";
const BOOK_C_KEY: &str = "Qry2.Book.C";
const PERSON_1_KEY: &str = "Qry2.Person.1";
const PERSON_2_KEY: &str = "Qry2.Person.2";

/// QRY2 (issue #715) vertical slice: `SeedHolons` and `Expand` execute end to end.
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
///      `Next` is refused rather than truncated to its root;
///    - `Query.Qry2Expand` (declared `AuthoredBy`) over `[A, B, C]`: authors in
///      source order then storage order, duplicates kept, C contributing none;
///    - `Query.Qry2Expand` with no input: the transform-root contract error;
///    - `Query.Qry2Inverse` (inverse `AuthorOf`) over `[Person.1]`: both books;
///    - `Query.Qry2Bogus`: the descriptor resolver's own error;
///    - `Query.Qry2Expand` over a single holon (direct only): the convenience
///      records a singleton collection as `Input`, never the holon itself.
pub fn query_qry2_seed_expand_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_qry2_seed_expand",
        "QRY2a: SeedHolons expands the focal space's Owns through the direct and QueryDance \
         routes; supplied input and Next are refused explicitly",
    );

    // Book/Person supplies a declared relationship (`AuthoredBy`) and its
    // inverse (`AuthorOf`) over committed instances — the two navigation
    // directions `Expand` resolves through each member's HolonDescriptor.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Build and commit the Query definitions and Expand test data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY2 Query definitions".to_string()),
    )?;

    let book_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_DESCRIPTOR_KEY)?;
    let person_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_DESCRIPTOR_KEY,
    )?;
    let person_1 = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_1_KEY,
        PropertyMap::new(),
        &person_type,
        "Person",
    )?;
    let person_2 = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_2_KEY,
        PropertyMap::new(),
        &person_type,
        "Person",
    )?;
    let book_a = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        BOOK_A_KEY,
        PropertyMap::new(),
        &book_type,
        "Book",
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_a,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_1.clone(), person_2.clone()],
        None,
        Some(format!("{BOOK_A_KEY} --AuthoredBy--> [Person.1, Person.2]")),
    )?;
    let book_b = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        BOOK_B_KEY,
        PropertyMap::new(),
        &book_type,
        "Book",
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_b,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_1.clone()],
        None,
        Some(format!("{BOOK_B_KEY} --AuthoredBy--> [Person.1]")),
    )?;
    stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        BOOK_C_KEY,
        PropertyMap::new(),
        &book_type,
        "Book",
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

    let chained_expand = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        CHAINED_EXPAND_EXPRESSION_KEY,
        expansion_properties(BOOK_TO_PERSON_RELATIONSHIP),
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

    for (expression_key, query_key, relationship_name, query_name) in [
        (
            AUTHORED_BY_EXPRESSION_KEY,
            AUTHORED_BY_QUERY_KEY,
            BOOK_TO_PERSON_RELATIONSHIP,
            "QRY2 declared-name expand query",
        ),
        (
            AUTHOR_OF_EXPRESSION_KEY,
            AUTHOR_OF_QUERY_KEY,
            PERSON_TO_BOOK_RELATIONSHIP,
            "QRY2 inverse-name expand query",
        ),
        (
            BOGUS_EXPRESSION_KEY,
            BOGUS_QUERY_KEY,
            UNKNOWN_RELATIONSHIP,
            "QRY2 unknown-name expand query",
        ),
    ] {
        let expression = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            expression_key,
            expansion_properties(relationship_name),
            &expand_type,
            "Expand",
        )?;
        stage_query(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            query_key,
            query_name,
            &query_type,
            expression,
        )?;
    }

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit the QRY2 Query definitions and Book/Person instances".to_string()),
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
    let saved_authored_by_query = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        AUTHORED_BY_QUERY_KEY,
    )?;
    let saved_author_of_query =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, AUTHOR_OF_QUERY_KEY)?;
    let saved_bogus_query =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOGUS_QUERY_KEY)?;
    let saved_book_a =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_A_KEY)?;
    let saved_book_b =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_B_KEY)?;
    let saved_book_c =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_C_KEY)?;
    let saved_person_1 =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, PERSON_1_KEY)?;
    let saved_person_2 =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, PERSON_2_KEY)?;

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

        // Declared name: source order then storage order, duplicate Person.1
        // retained across sources, Book.C contributing nothing.
        test_case.add_execute_query_step(
            saved_authored_by_query.clone(),
            QueryInputSpec::Collection(vec![
                saved_book_a.clone(),
                saved_book_b.clone(),
                saved_book_c.clone(),
            ]),
            route,
            QueryExpectation::Members(vec![
                saved_person_1.clone(),
                saved_person_2.clone(),
                saved_person_1.clone(),
            ]),
            Some(format!("Expand(AuthoredBy) via {route:?} preserves order and duplicates")),
        )?;
        test_case.add_execute_query_step(
            saved_authored_by_query.clone(),
            QueryInputSpec::None,
            route,
            QueryExpectation::Error(HolonErrorKind::MissingRequiredRelationship),
            Some(format!("Root Expand via {route:?} requires exactly one input collection")),
        )?;
        // Inverse name: resolved through the same descriptor authority.
        test_case.add_execute_query_step(
            saved_author_of_query.clone(),
            QueryInputSpec::Collection(vec![saved_person_1.clone()]),
            route,
            QueryExpectation::Members(vec![saved_book_a.clone(), saved_book_b.clone()]),
            Some(format!("Expand(AuthorOf) via {route:?} navigates the inverse name")),
        )?;
        test_case.add_execute_query_step(
            saved_bogus_query.clone(),
            QueryInputSpec::Collection(vec![saved_book_a.clone()]),
            route,
            QueryExpectation::Error(HolonErrorKind::DescriptorDeclarationNotFound),
            Some(format!(
                "Expand over an unknown name via {route:?} propagates the descriptor error"
            )),
        )?;
    }

    // Direct-only: the Dance contract stays collection-shaped.
    test_case.add_execute_query_step(
        saved_authored_by_query,
        QueryInputSpec::SingleHolon(saved_book_a),
        QueryRoute::Direct,
        QueryExpectation::Members(vec![saved_person_1, saved_person_2]),
        Some(
            "Single-holon convenience records a transient singleton collection as Input"
                .to_string(),
        ),
    )?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

/// Properties for an `Expand` definition navigating `relationship_name`.
fn expansion_properties(relationship_name: &str) -> PropertyMap {
    let mut properties = PropertyMap::new();
    properties.insert(
        "ExpansionRelationshipName".to_property_name(),
        MapString(relationship_name.to_string()).to_base_value(),
    );
    properties
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
