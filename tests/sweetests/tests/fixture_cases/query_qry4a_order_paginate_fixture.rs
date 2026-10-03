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
/// OrderBy schema descriptors. Their runtime lands separately; this slice
/// proves the bootstrap bundle loads them.
const ORDER_BY_SCHEMA_KEYS: [&str; 6] = [
    "OrderBy.HolonType",
    "OrderBySpec.HolonType",
    "PropertyName.PropertyType",
    "SortDirection.PropertyType",
    "NullPlacement.PropertyType",
    "OneToFive.CardinalityConstraint",
];
const ORDER_BY_DESCRIPTOR_KEY: &str = "OrderBy.HolonType";
const ORDER_BY_SPEC_DESCRIPTOR_KEY: &str = "OrderBySpec.HolonType";
const TITLE_PROPERTY_KEY: &str = "Title.PropertyType";
const PAGE_COUNT_PROPERTY_KEY: &str = "PageCount.PropertyType";
const IS_PUBLISHED_PROPERTY_KEY: &str = "IsPublished.PropertyType";
const PUBLICATION_STATUS_PROPERTY_KEY: &str = "PublicationStatus.PropertyType";
const NAME_PROPERTY_KEY: &str = "Name.PropertyType";
/// Inverse of `AuthoredBy` in the Book/Person test schema.
const PERSON_TO_BOOK_RELATIONSHIP: &str = "AuthorOf";

/// Committed Book/Person data. Authors: A -> [P1, P2], B -> [P1], C -> [],
/// D -> [P2, P1], so `Expand(AuthoredBy)` over [A, B, D] is
/// [P1, P2, P1, P2, P1] — a sequence with repeated occurrences to page over.
const BOOK_A_KEY: &str = "Qry4a.Book.A";
const BOOK_B_KEY: &str = "Qry4a.Book.B";
const BOOK_C_KEY: &str = "Qry4a.Book.C";
const BOOK_D_KEY: &str = "Qry4a.Book.D";
const PERSON_1_KEY: &str = "Qry4a.Person.1";
const PERSON_2_KEY: &str = "Qry4a.Person.2";

/// Committed sort data, all authored by `Qry4a.Person.3`: `(key, Title,
/// PageCount)`. Titles repeat (ties) and compare ordinally — "Bravo" < "Charlie"
/// < "Delta" < "alpha" — and two books have no `PageCount` (an optional
/// property, so a missing value).
const SORT_BOOKS: [(&str, &str, Option<i64>); 6] = [
    ("Qry4a.Sort.1", "Delta", Some(300)),
    ("Qry4a.Sort.2", "alpha", Some(100)),
    ("Qry4a.Sort.3", "Charlie", None),
    ("Qry4a.Sort.4", "Bravo", Some(100)),
    ("Qry4a.Sort.5", "Charlie", Some(250)),
    ("Qry4a.Sort.6", "Bravo", None),
];
const PERSON_3_KEY: &str = "Qry4a.Person.3";

/// QRY4a (issue #755): transient query definitions, `OrderBy`, `Skip`, `Limit`,
/// and invocation-binding refusal, on both the direct and QueryDance routes.
/// The executor also asserts, for every step, that evaluation leaves the whole
/// definition graph (query, expressions, OrderBySpecs) unchanged.
///
/// 1. Stage and commit Book/Person data only. No Query definition is committed.
/// 2. In a fresh transaction, author every Query / Expand / OrderBy /
///    OrderBySpec / Skip / Limit as a **transient** holon (`NewHolon` +
///    `DescribedBy` + relationships; never staged), then execute the paging
///    cases (4a-1):
///    - `Expand -> Skip(1) -> Limit(3)` over [A, B, D]: [P2, P1, P2];
///    - `Limit(2) -> Skip(1)` versus `Skip(1) -> Limit(2)` over [A, B, C, A]:
///      authored `Next` order decides the page ([B] versus [B, C]);
///    - an oversized `Skip` and a zero `Limit`: empty results;
///    - `Expand -> Skip` with no `SkipCount`: `EmptyField` charged to the Skip;
///    - a negative `LimitCount` over empty input: `InvalidParameter`;
///    - a root `Skip` with no input: `MissingRequiredRelationship`;
///    - the paging query with a nonempty binding list: `NotImplemented` before
///      any runtime record exists;
///
///    and the ordering cases (4a-2), over the `SORT_BOOKS` input
///    `[1, 2, 3, 4, 5, 6, 2]` unless noted:
///    - the delivery proof `Expand(AuthorOf) -> OrderBy(PageCount desc, Title)
///      -> Skip(1) -> Limit(3)` from Person.3: `[5, 4, 2]`;
///    - Title with SortDirection and NullPlacement omitted (resolved to
///      Ascending / Missing-Last, never stored): ordinal, stable, duplicates kept;
///    - PageCount Descending + Missing-First; PageCount explicitly Ascending;
///    - Title then PageCount descending (the Title spec holon is shared with
///      other OrderBy expressions);
///    - `Skip(2) -> OrderBy` versus `OrderBy -> Skip(2)`; empty and singleton input;
///    - failures: no specs (even on empty input), a missing / doubled /
///      wrong-kind Property target, an undeclared variant, a string where an enum
///      value belongs, boolean and enum sort domains, a property the members do
///      not declare, and the same failure after a completed `Expand`.
///
/// The OrderBy schema descriptors (`OrderBy`, `OrderBySpec`, `PropertyName`,
/// `SortDirection`, `NullPlacement`, `OneToFive`) are resolved by key to prove
/// the bootstrap bundle loads them.
pub fn query_qry4a_order_paginate_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_qry4a_order_paginate",
        "QRY4a: transient Query graphs order with OrderBy and page with Skip and Limit \
         through the direct and QueryDance routes; invocation bindings are refused",
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
    let person_3 = stage(
        &mut test_case,
        &mut fixture_holons,
        PERSON_3_KEY,
        "Name",
        "Qry4a Person Three",
        &person_type,
        "Person",
    )?;
    for (key, title, page_count) in SORT_BOOKS {
        let mut properties = instance_properties("Title", title);
        if let Some(page_count) = page_count {
            properties
                .insert("PageCount".to_property_name(), MapInteger(page_count).to_base_value());
        }
        let book = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            key,
            properties,
            &book_type,
            "Book",
        )?;
        test_case.add_add_related_holons_step(
            &mut fixture_holons,
            book,
            RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
            vec![person_3.clone()],
            None,
            Some(format!("{key} --AuthoredBy--> Person.3")),
        )?;
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
    let person_3 = lookup(&mut test_case, &mut fixture_holons, PERSON_3_KEY)?;
    let mut sorted_books = Vec::new();
    for (key, _, _) in SORT_BOOKS {
        sorted_books.push(lookup(&mut test_case, &mut fixture_holons, key)?);
    }
    let book = |number: usize| sorted_books[number - 1].clone();
    let books = |numbers: &[usize]| numbers.iter().map(|number| book(*number)).collect::<Vec<_>>();
    let title = lookup(&mut test_case, &mut fixture_holons, TITLE_PROPERTY_KEY)?;
    let page_count = lookup(&mut test_case, &mut fixture_holons, PAGE_COUNT_PROPERTY_KEY)?;
    let is_published = lookup(&mut test_case, &mut fixture_holons, IS_PUBLISHED_PROPERTY_KEY)?;
    let publication_status =
        lookup(&mut test_case, &mut fixture_holons, PUBLICATION_STATUS_PROPERTY_KEY)?;
    let name = lookup(&mut test_case, &mut fixture_holons, NAME_PROPERTY_KEY)?;

    for key in ORDER_BY_SCHEMA_KEYS {
        lookup(&mut test_case, &mut fixture_holons, key)?;
    }

    let mut authoring = TransientAuthoring {
        query_type: lookup(&mut test_case, &mut fixture_holons, QUERY_DESCRIPTOR_KEY)?,
        expand_type: lookup(&mut test_case, &mut fixture_holons, EXPAND_DESCRIPTOR_KEY)?,
        skip_type: lookup(&mut test_case, &mut fixture_holons, SKIP_DESCRIPTOR_KEY)?,
        limit_type: lookup(&mut test_case, &mut fixture_holons, LIMIT_DESCRIPTOR_KEY)?,
        order_by_type: lookup(&mut test_case, &mut fixture_holons, ORDER_BY_DESCRIPTOR_KEY)?,
        spec_type: lookup(&mut test_case, &mut fixture_holons, ORDER_BY_SPEC_DESCRIPTOR_KEY)?,
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

    // --- OrderBy (4a-2) ---
    let descending = Some(enum_value("Descending"));
    let ascending = Some(enum_value("Ascending"));
    let missing_first = Some(enum_value("Missing-First"));

    // Delivery proof: Expand(AuthorOf) -> OrderBy(PageCount desc, Title) ->
    // Skip(1) -> Limit(3). The keys order every book, so the storage order of
    // the inverse expansion cannot leak into the result.
    let proof_limit = authoring.limit("Limit.Qry4aProof", Some(3))?;
    let proof_skip = authoring.skip("Skip.Qry4aProof", Some(1))?;
    let proof_skip = authoring.next(proof_skip, proof_limit)?;
    let proof_pages =
        authoring.spec("Spec.Qry4aProofPages", &[&page_count], descending.clone(), None)?;
    let proof_titles = authoring.spec("Spec.Qry4aProofTitles", &[&title], None, None)?;
    let proof_order = authoring.order_by("OrderBy.Qry4aProof", vec![proof_pages, proof_titles])?;
    let proof_order = authoring.next(proof_order, proof_skip)?;
    let proof_expand = authoring.expand("Expand.Qry4aProof", PERSON_TO_BOOK_RELATIONSHIP)?;
    let proof_expand = authoring.next(proof_expand, proof_order)?;
    let proof_query = authoring.query("Query.Qry4aProof", proof_expand)?;

    // One Title spec with both enum arguments omitted, shared by several
    // OrderBy expressions (an aliased definition holon).
    let title_spec = authoring.spec("Spec.Qry4aTitle", &[&title], None, None)?;
    let by_title = authoring.order_by("OrderBy.Qry4aTitle", vec![title_spec.clone()])?;
    let by_title = authoring.query("Query.Qry4aByTitle", by_title)?;
    let pages_desc_first = authoring.spec(
        "Spec.Qry4aPagesDescFirst",
        &[&page_count],
        descending.clone(),
        missing_first,
    )?;
    let pages_desc_first =
        authoring.order_by("OrderBy.Qry4aPagesDescFirst", vec![pages_desc_first])?;
    let pages_desc_first = authoring.query("Query.Qry4aPagesDescFirst", pages_desc_first)?;
    let pages_asc = authoring.spec("Spec.Qry4aPagesAsc", &[&page_count], ascending, None)?;
    let pages_asc = authoring.order_by("OrderBy.Qry4aPagesAsc", vec![pages_asc])?;
    let pages_asc = authoring.query("Query.Qry4aPagesAsc", pages_asc)?;
    let pages_desc =
        authoring.spec("Spec.Qry4aPagesDesc", &[&page_count], descending.clone(), None)?;
    let title_then_pages =
        authoring.order_by("OrderBy.Qry4aTitleThenPages", vec![title_spec.clone(), pages_desc])?;
    let title_then_pages = authoring.query("Query.Qry4aTitleThenPages", title_then_pages)?;

    // Authored order: Skip(2) -> OrderBy versus OrderBy -> Skip(2).
    let sort_after_skip = authoring.order_by("OrderBy.Qry4aAfterSkip", vec![title_spec.clone()])?;
    let skip_then_sort = authoring.skip("Skip.Qry4aThenSort", Some(2))?;
    let skip_then_sort = authoring.next(skip_then_sort, sort_after_skip)?;
    let skip_then_sort = authoring.query("Query.Qry4aSkipThenSort", skip_then_sort)?;
    let skip_after_sort = authoring.skip("Skip.Qry4aAfterSort", Some(2))?;
    let sort_then_skip = authoring.order_by("OrderBy.Qry4aThenSkip", vec![title_spec.clone()])?;
    let sort_then_skip = authoring.next(sort_then_skip, skip_after_sort)?;
    let sort_then_skip = authoring.query("Query.Qry4aSortThenSkip", sort_then_skip)?;

    // Failures, each rooted at an OrderBy over Books unless noted.
    let mut failing = Vec::new();
    let no_specs = authoring.order_by("OrderBy.Qry4aNoSpecs", vec![])?;
    let no_specs = authoring.query("Query.Qry4aNoSpecs", no_specs)?;
    let failing_spec_cases: [(&str, Vec<&TestReference>, Option<BaseValue>, HolonErrorKind); 8] = [
        ("NoProperty", vec![], None, HolonErrorKind::MissingRequiredRelationship),
        ("TwoProperties", vec![&title, &page_count], None, HolonErrorKind::MultipleRelatedHolons),
        ("BookAsProperty", vec![&sorted_books[0]], None, HolonErrorKind::WrongDescriptorKind),
        (
            "UndeclaredVariant",
            vec![&title],
            Some(enum_value("Sideways")),
            HolonErrorKind::EnumVariantNotInSchema,
        ),
        (
            "StringDirection",
            vec![&title],
            Some(MapString("Descending".to_string()).to_base_value()),
            HolonErrorKind::ValueKindMismatch,
        ),
        ("BooleanKey", vec![&is_published], None, HolonErrorKind::UnsupportedOperator),
        ("EnumKey", vec![&publication_status], None, HolonErrorKind::UnsupportedOperator),
        ("PersonNameOnBooks", vec![&name], None, HolonErrorKind::DescriptorDeclarationNotFound),
    ];
    for (label, targets, direction, kind) in failing_spec_cases {
        let spec = authoring.spec(&format!("Spec.Qry4a{label}"), &targets, direction, None)?;
        let order = authoring.order_by(&format!("OrderBy.Qry4a{label}"), vec![spec])?;
        failing.push((label, authoring.query(&format!("Query.Qry4a{label}"), order)?, kind));
    }
    // The same undeclared-property failure reached after a completed Expand.
    let late_name = authoring.spec("Spec.Qry4aLateName", &[&name], None, None)?;
    let late_order = authoring.order_by("OrderBy.Qry4aLateName", vec![late_name])?;
    let late_expand = authoring.expand("Expand.Qry4aLateName", PERSON_TO_BOOK_RELATIONSHIP)?;
    let late_expand = authoring.next(late_expand, late_order)?;
    let late_failure = authoring.query("Query.Qry4aLateName", late_expand)?;

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

        // --- OrderBy ---
        let input = || QueryInputSpec::Collection(books(&[1, 2, 3, 4, 5, 6, 2]));
        let ordered = |numbers: &[usize]| QueryExpectation::Members(books(numbers));
        test_case.add_execute_query_step(
            proof_query.clone(),
            QueryInputSpec::Collection(vec![person_3.clone()]),
            route,
            ordered(&[5, 4, 2]),
            Some(format!("Delivery proof Expand -> OrderBy -> Skip -> Limit via {route:?}")),
        )?;
        for (query, expected, what) in [
            (&by_title, [4, 6, 3, 5, 1, 2, 2], "Title with defaults (ordinal, stable)"),
            (&pages_desc_first, [3, 6, 1, 5, 2, 4, 2], "PageCount Descending, Missing-First"),
            (&pages_asc, [2, 4, 2, 5, 1, 3, 6], "PageCount Ascending, default Missing-Last"),
            (&title_then_pages, [4, 6, 5, 3, 1, 2, 2], "Title, then PageCount Descending"),
        ] {
            test_case.add_execute_query_step(
                query.clone(),
                input(),
                route,
                ordered(&expected),
                Some(format!("OrderBy {what} via {route:?}")),
            )?;
        }
        test_case.add_execute_query_step(
            skip_then_sort.clone(),
            input(),
            route,
            ordered(&[4, 6, 3, 5, 2]),
            Some(format!("Skip(2) -> OrderBy sorts only the retained input via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            sort_then_skip.clone(),
            input(),
            route,
            ordered(&[3, 5, 1, 2, 2]),
            Some(format!("OrderBy -> Skip(2) slices the sorted result via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            by_title.clone(),
            QueryInputSpec::Collection(vec![]),
            route,
            ordered(&[]),
            Some(format!("OrderBy over empty input via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            by_title.clone(),
            QueryInputSpec::Collection(books(&[3])),
            route,
            ordered(&[3]),
            Some(format!("OrderBy over a singleton via {route:?}")),
        )?;

        test_case.add_execute_query_step(
            no_specs.clone(),
            QueryInputSpec::Collection(vec![]),
            route,
            QueryExpectation::Error(HolonErrorKind::InvalidParameter),
            Some(format!("OrderBy without specs via {route:?} fails even on empty input")),
        )?;
        for (label, query, kind) in &failing {
            test_case.add_execute_query_step(
                query.clone(),
                QueryInputSpec::Collection(books(&[1, 2])),
                route,
                QueryExpectation::Error(*kind),
                Some(format!("OrderBy {label} via {route:?} fails with {kind:?}")),
            )?;
        }
        test_case.add_execute_query_step(
            late_failure.clone(),
            QueryInputSpec::Collection(vec![person_3.clone()]),
            route,
            QueryExpectation::Error(HolonErrorKind::DescriptorDeclarationNotFound),
            Some(format!("A failing OrderBy after a completed Expand via {route:?}")),
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
    order_by_type: TestReference,
    spec_type: TestReference,
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
        self.relate_all(source, relationship, vec![target], description)
    }

    /// Relates `targets` in order; an empty list leaves `source` unchanged.
    fn relate_all(
        &mut self,
        source: TestReference,
        relationship: &str,
        targets: Vec<TestReference>,
        description: String,
    ) -> Result<TestReference, HolonError> {
        if targets.is_empty() {
            return Ok(source);
        }
        self.test_case.add_add_related_holons_step(
            self.fixture_holons,
            source,
            RelationshipName(MapString(relationship.to_string())),
            targets,
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

    /// An `OrderBySpec` over `properties` (normally exactly one PropertyType);
    /// `None` leaves SortDirection / NullPlacement unset.
    fn spec(
        &mut self,
        key: &str,
        properties: &[&TestReference],
        direction: Option<BaseValue>,
        placement: Option<BaseValue>,
    ) -> Result<TestReference, HolonError> {
        let mut values = PropertyMap::new();
        if let Some(direction) = direction {
            values.insert("SortDirection".to_property_name(), direction);
        }
        if let Some(placement) = placement {
            values.insert("NullPlacement".to_property_name(), placement);
        }
        let spec = self.described(key, values, self.spec_type.clone())?;
        let targets = properties.iter().map(|property| (*property).clone()).collect();
        self.relate_all(spec, "Property", targets, format!("{key} --Property--> sort property"))
    }

    /// An `OrderBy` relating `specs` in precedence order.
    fn order_by(
        &mut self,
        key: &str,
        specs: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let order_by = self.described(key, PropertyMap::new(), self.order_by_type.clone())?;
        self.relate_all(order_by, "OrderBySpecs", specs, format!("{key} --OrderBySpecs--> specs"))
    }

    fn next(
        &mut self,
        expression: TestReference,
        successor: TestReference,
    ) -> Result<TestReference, HolonError> {
        self.relate(expression, "Next", successor, "Relate expression --Next--> successor".into())
    }
}

fn enum_value(variant: &str) -> BaseValue {
    MapEnumValue(MapString(variant.to_string())).to_base_value()
}

/// `SkipCount` / `LimitCount` properties; `None` leaves the count unset.
fn count_properties(property_name: &str, count: Option<i64>) -> PropertyMap {
    let mut properties = PropertyMap::new();
    if let Some(count) = count {
        properties.insert(property_name.to_property_name(), MapInteger(count).to_base_value());
    }
    properties
}
