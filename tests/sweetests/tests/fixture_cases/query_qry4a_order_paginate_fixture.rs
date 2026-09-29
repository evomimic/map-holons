use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};

use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

const QUERY_DESCRIPTOR_KEY: &str = "Query.HolonType";
const EXPAND_DESCRIPTOR_KEY: &str = "Expand.HolonType";
const SKIP_DESCRIPTOR_KEY: &str = "Skip.HolonType";
const LIMIT_DESCRIPTOR_KEY: &str = "Limit.HolonType";

/// Committed Book/Person data. Authors: A -> [P1, P2], B -> [P1], C -> [],
/// D -> [P2, P1], so `Expand(AuthoredBy)` over [A, B, D] is
/// [P1, P2, P1, P2, P1] — a sequence with repeated occurrences to page over.
const BOOK_A_KEY: &str = "Qry4a.Book.A";
const BOOK_B_KEY: &str = "Qry4a.Book.B";
const BOOK_C_KEY: &str = "Qry4a.Book.C";
const BOOK_D_KEY: &str = "Qry4a.Book.D";
const PERSON_1_KEY: &str = "Qry4a.Person.1";
const PERSON_2_KEY: &str = "Qry4a.Person.2";

/// QRY4a (issue #755), slice 4a-1: transient query definitions, `Skip`, `Limit`,
/// and invocation-binding refusal, on both the direct and QueryDance routes.
///
/// 1. Stage and commit Book/Person data only. No Query definition is committed.
/// 2. In a fresh transaction, author every Query / Expand / Skip / Limit as a
///    **transient** holon (`NewHolon` + `DescribedBy` + relationships; never
///    staged), then execute:
///    - `Expand -> Skip(1) -> Limit(3)` over [A, B, D]: [P2, P1, P2];
///    - `Limit(2) -> Skip(1)` versus `Skip(1) -> Limit(2)` over [A, B, C, A]:
///      authored `Next` order decides the page ([B] versus [B, C]);
///    - an oversized `Skip` and a zero `Limit`: empty results;
///    - `Expand -> Skip` with no `SkipCount`: `EmptyField` charged to the Skip;
///    - a negative `LimitCount` over empty input: `InvalidParameter`;
///    - a root `Skip` with no input: `MissingRequiredRelationship`;
///    - the paging query with a nonempty binding list: `NotImplemented` before
///      any runtime record exists.
pub fn query_qry4a_order_paginate_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_qry4a_order_paginate",
        "QRY4a-1: transient Query graphs page with Skip and Limit through the direct and \
         QueryDance routes; invocation bindings are refused",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data (and nothing else) ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY4a Book/Person data".to_string()),
    )?;
    let book_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_DESCRIPTOR_KEY)?;
    let person_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_DESCRIPTOR_KEY,
    )?;
    let stage = |test_case: &mut DancesTestCase,
                 fixture_holons: &mut FixtureHolons,
                 key: &str,
                 property: &str,
                 value: &str,
                 descriptor: &TestReference,
                 label: &str| {
        stage_described(
            test_case,
            &fixture_context,
            fixture_holons,
            key,
            instance_properties(property, value),
            descriptor,
            label,
        )
    };
    let person_1 = stage(
        &mut test_case,
        &mut fixture_holons,
        PERSON_1_KEY,
        "Name",
        "Qry4a Person One",
        &person_type,
        "Person",
    )?;
    let person_2 = stage(
        &mut test_case,
        &mut fixture_holons,
        PERSON_2_KEY,
        "Name",
        "Qry4a Person Two",
        &person_type,
        "Person",
    )?;
    for (key, title, authors) in [
        (BOOK_A_KEY, "Qry4a Book A", vec![person_1.clone(), person_2.clone()]),
        (BOOK_B_KEY, "Qry4a Book B", vec![person_1.clone()]),
        (BOOK_C_KEY, "Qry4a Book C", vec![]),
        (BOOK_D_KEY, "Qry4a Book D", vec![person_2.clone(), person_1.clone()]),
    ] {
        let book =
            stage(&mut test_case, &mut fixture_holons, key, "Title", title, &book_type, "Book")?;
        if !authors.is_empty() {
            test_case.add_add_related_holons_step(
                &mut fixture_holons,
                book,
                RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
                authors,
                None,
                Some(format!("{key} --AuthoredBy--> its authors")),
            )?;
        }
    }
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit the QRY4a Book/Person data".to_string()),
    )?;
    test_case.add_match_saved_content_step()?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY4a transient query execution".to_string()),
    )?;
    let lookup = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let book_a = lookup(&mut test_case, &mut fixture_holons, BOOK_A_KEY)?;
    let book_b = lookup(&mut test_case, &mut fixture_holons, BOOK_B_KEY)?;
    let book_c = lookup(&mut test_case, &mut fixture_holons, BOOK_C_KEY)?;
    let book_d = lookup(&mut test_case, &mut fixture_holons, BOOK_D_KEY)?;
    let person_1 = lookup(&mut test_case, &mut fixture_holons, PERSON_1_KEY)?;
    let person_2 = lookup(&mut test_case, &mut fixture_holons, PERSON_2_KEY)?;

    let mut authoring = TransientAuthoring {
        query_type: lookup(&mut test_case, &mut fixture_holons, QUERY_DESCRIPTOR_KEY)?,
        expand_type: lookup(&mut test_case, &mut fixture_holons, EXPAND_DESCRIPTOR_KEY)?,
        skip_type: lookup(&mut test_case, &mut fixture_holons, SKIP_DESCRIPTOR_KEY)?,
        limit_type: lookup(&mut test_case, &mut fixture_holons, LIMIT_DESCRIPTOR_KEY)?,
        test_case: &mut test_case,
        fixture_context: &fixture_context,
        fixture_holons: &mut fixture_holons,
    };

    // Expand(AuthoredBy) -> Skip(1) -> Limit(3). Built tail first so each
    // `Next` targets an already-authored successor.
    let page_limit = authoring.limit("Limit.Qry4aPage", Some(3))?;
    let page_skip = authoring.skip("Skip.Qry4aPage", Some(1))?;
    let page_skip = authoring.next(page_skip, page_limit)?;
    let page_expand = authoring.expand("Expand.Qry4aPage", BOOK_TO_PERSON_RELATIONSHIP)?;
    let page_expand = authoring.next(page_expand, page_skip)?;
    let page_query = authoring.query("Query.Qry4aPage", page_expand)?;

    // Authored order decides the page.
    let late_skip = authoring.skip("Skip.Qry4aLimitThenSkip", Some(1))?;
    let early_limit = authoring.limit("Limit.Qry4aLimitThenSkip", Some(2))?;
    let early_limit = authoring.next(early_limit, late_skip)?;
    let limit_then_skip = authoring.query("Query.Qry4aLimitThenSkip", early_limit)?;
    let late_limit = authoring.limit("Limit.Qry4aSkipThenLimit", Some(2))?;
    let early_skip = authoring.skip("Skip.Qry4aSkipThenLimit", Some(1))?;
    let early_skip = authoring.next(early_skip, late_limit)?;
    let skip_then_limit = authoring.query("Query.Qry4aSkipThenLimit", early_skip)?;

    // Count boundaries.
    let oversized_skip = authoring.skip("Skip.Qry4aOversized", Some(9))?;
    let oversized_skip = authoring.query("Query.Qry4aOversizedSkip", oversized_skip)?;
    let zero_limit = authoring.limit("Limit.Qry4aZero", Some(0))?;
    let zero_limit = authoring.query("Query.Qry4aZeroLimit", zero_limit)?;

    // Invalid counts.
    let countless_skip = authoring.skip("Skip.Qry4aCountless", None)?;
    let countless_expand =
        authoring.expand("Expand.Qry4aCountless", BOOK_TO_PERSON_RELATIONSHIP)?;
    let countless_expand = authoring.next(countless_expand, countless_skip)?;
    let countless = authoring.query("Query.Qry4aCountlessSkip", countless_expand)?;
    let negative_limit = authoring.limit("Limit.Qry4aNegative", Some(-1))?;
    let negative_limit = authoring.query("Query.Qry4aNegativeLimit", negative_limit)?;

    // Any holon stands in for a binding: refusal happens before its content
    // could matter, and neither route validates RequestParameters targets.
    let binding = authoring.new_holon("Qry4a.Binding", PropertyMap::new())?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        test_case.add_execute_query_step(
            page_query.clone(),
            QueryInputSpec::Collection(vec![book_a.clone(), book_b.clone(), book_d.clone()]),
            route,
            QueryExpectation::Members(vec![person_2.clone(), person_1.clone(), person_2.clone()]),
            Some(format!("Transient Expand -> Skip(1) -> Limit(3) via {route:?}")),
        )?;

        let repeated = vec![book_a.clone(), book_b.clone(), book_c.clone(), book_a.clone()];
        test_case.add_execute_query_step(
            limit_then_skip.clone(),
            QueryInputSpec::Collection(repeated.clone()),
            route,
            QueryExpectation::Members(vec![book_b.clone()]),
            Some(format!("Limit(2) -> Skip(1) via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            skip_then_limit.clone(),
            QueryInputSpec::Collection(repeated),
            route,
            QueryExpectation::Members(vec![book_b.clone(), book_c.clone()]),
            Some(format!("Skip(1) -> Limit(2) via {route:?}")),
        )?;

        test_case.add_execute_query_step(
            oversized_skip.clone(),
            QueryInputSpec::Collection(vec![book_a.clone(), book_b.clone()]),
            route,
            QueryExpectation::Members(vec![]),
            Some(format!("Skip past the end via {route:?} is empty")),
        )?;
        test_case.add_execute_query_step(
            zero_limit.clone(),
            QueryInputSpec::Collection(vec![book_a.clone(), book_b.clone()]),
            route,
            QueryExpectation::Members(vec![]),
            Some(format!("Limit(0) via {route:?} is empty")),
        )?;

        test_case.add_execute_query_step(
            countless.clone(),
            QueryInputSpec::Collection(vec![book_a.clone()]),
            route,
            QueryExpectation::Error(HolonErrorKind::EmptyField),
            Some(format!("Expand -> Skip without SkipCount via {route:?} fails the Skip")),
        )?;
        test_case.add_execute_query_step(
            negative_limit.clone(),
            QueryInputSpec::Collection(vec![]),
            route,
            QueryExpectation::Error(HolonErrorKind::InvalidParameter),
            Some(format!("A negative LimitCount via {route:?} fails even on empty input")),
        )?;
        test_case.add_execute_query_step(
            oversized_skip.clone(),
            QueryInputSpec::None,
            route,
            QueryExpectation::Error(HolonErrorKind::MissingRequiredRelationship),
            Some(format!("A root Skip via {route:?} requires an input collection")),
        )?;

        test_case.add_execute_query_with_bindings_step(
            page_query.clone(),
            QueryInputSpec::Collection(vec![book_a.clone()]),
            route,
            vec![binding.clone()],
            QueryExpectation::Error(HolonErrorKind::NotImplemented),
            Some(format!("Invocation bindings via {route:?} are refused before any record")),
        )?;
    }

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

/// Authors transient Query definitions: every holon is minted by a `NewHolon`
/// step and related in place — never staged or committed.
struct TransientAuthoring<'a> {
    test_case: &'a mut DancesTestCase,
    fixture_context: &'a std::sync::Arc<TransactionContext>,
    fixture_holons: &'a mut FixtureHolons,
    query_type: TestReference,
    expand_type: TestReference,
    skip_type: TestReference,
    limit_type: TestReference,
}

impl TransientAuthoring<'_> {
    fn new_holon(
        &mut self,
        key: &str,
        properties: PropertyMap,
    ) -> Result<TestReference, HolonError> {
        let source = self.fixture_context.mutation().new_holon(Some(MapString(key.to_string())))?;
        self.test_case.add_new_holon_step(
            self.fixture_holons,
            source,
            properties,
            Some(MapString(key.to_string())),
            None,
            Some(format!("Author transient {key}")),
        )
    }

    fn relate(
        &mut self,
        source: TestReference,
        relationship: &str,
        target: TestReference,
        description: String,
    ) -> Result<TestReference, HolonError> {
        self.test_case.add_add_related_holons_step(
            self.fixture_holons,
            source,
            RelationshipName(MapString(relationship.to_string())),
            vec![target],
            None,
            Some(description),
        )
    }

    fn described(
        &mut self,
        key: &str,
        properties: PropertyMap,
        descriptor: TestReference,
    ) -> Result<TestReference, HolonError> {
        let token = self.new_holon(key, properties)?;
        self.relate(token, "DescribedBy", descriptor, format!("Describe transient {key}"))
    }

    fn query(&mut self, key: &str, root: TestReference) -> Result<TestReference, HolonError> {
        let properties = instance_properties("QueryName", key);
        let query = self.described(key, properties, self.query_type.clone())?;
        self.relate(query, "RootExpression", root, format!("{key} --RootExpression--> root"))
    }

    fn expand(&mut self, key: &str, relationship: &str) -> Result<TestReference, HolonError> {
        let properties = instance_properties("ExpansionRelationshipName", relationship);
        self.described(key, properties, self.expand_type.clone())
    }

    fn skip(&mut self, key: &str, count: Option<i64>) -> Result<TestReference, HolonError> {
        self.described(key, count_properties("SkipCount", count), self.skip_type.clone())
    }

    fn limit(&mut self, key: &str, count: Option<i64>) -> Result<TestReference, HolonError> {
        self.described(key, count_properties("LimitCount", count), self.limit_type.clone())
    }

    fn next(
        &mut self,
        expression: TestReference,
        successor: TestReference,
    ) -> Result<TestReference, HolonError> {
        self.relate(expression, "Next", successor, "Relate expression --Next--> successor".into())
    }
}

/// `SkipCount` / `LimitCount` properties; `None` leaves the count unset.
fn count_properties(property_name: &str, count: Option<i64>) -> PropertyMap {
    let mut properties = PropertyMap::new();
    if let Some(count) = count {
        properties.insert(property_name.to_property_name(), MapInteger(count).to_base_value());
    }
    properties
}
