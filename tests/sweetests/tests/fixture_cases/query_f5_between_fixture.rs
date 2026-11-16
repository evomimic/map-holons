use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};
use type_names::QueryRelationshipTypeName;

use super::query_f1_filter_fixture::{query_authoring, PredicateAuthoring, PredicateTypes};
use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

/// Committed Book/Person data. Titles in ordinal (code point) order are
/// `Atlas` < `Huge` < `Roads` < `STRASSE` < `Straße` < `Zebra` < `atlas notes`.
/// `Unknown` has no PageCount (an optional property), and `Huge` carries the
/// largest integer. Only `Inside` carries `RangeTarget` in its key, for the
/// SeedPredicate range. Seed predicates scan every saved holon in the shared
/// conductor, so the marker avoids the keys other query fixtures seed.
const BOOK_SHORT_KEY: &str = "QryF5.Book.Short";
const BOOK_LOWER_KEY: &str = "QryF5.Book.Lower";
const BOOK_INSIDE_KEY: &str = "QryF5.RangeTarget.Book.Inside";
const BOOK_UPPER_KEY: &str = "QryF5.Book.Upper";
const BOOK_LONG_KEY: &str = "QryF5.Book.Long";
const BOOK_UNKNOWN_KEY: &str = "QryF5.Book.Unknown";
const BOOK_HUGE_KEY: &str = "QryF5.Book.Huge";
/// (key, Title, PageCount)
const BOOKS: [(&str, &str, Option<i64>); 7] = [
    (BOOK_SHORT_KEY, "Atlas", Some(50)),
    (BOOK_LOWER_KEY, "Roads", Some(100)),
    (BOOK_INSIDE_KEY, "Straße", Some(150)),
    (BOOK_UPPER_KEY, "STRASSE", Some(200)),
    (BOOK_LONG_KEY, "atlas notes", Some(300)),
    (BOOK_UNKNOWN_KEY, "Zebra", None),
    (BOOK_HUGE_KEY, "Huge", Some(i64::MAX)),
];
/// Authors: Short -> [Ada, Bob], Lower -> [Cy], Long -> [Bob].
const PERSON_ADA_KEY: &str = "QryF5.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF5.Person.Bob";
const PERSON_CY_KEY: &str = "QryF5.Person.Cy";

/// QRY-F5 (issue #865): Between predicates, a subject with two ordered
/// authored bounds, through `Filter`, `SeedPredicate` and
/// `ExpansionPredicate`, alone and composed, on both the direct and QueryDance
/// routes. Every definition is transient. Range semantics in detail (signed
/// extremes, metadata, extension operators, every operand position) are owned
/// by unit tests; this fixture proves the loaded Core operator executes
/// through every entry form and route, and that a reversed range fails before
/// any candidate is read.
///
/// Cases, over [Short, Lower, Inside, Upper, Long, Unknown, Huge] unless noted:
/// - `PageCount between [100, 200]`: [Lower, Inside, Upper] — both bounds
///   included, and the absent optional PageCount never matches;
/// - `PageCount between [150, 150]`: [Inside] — equal bounds;
/// - `PageCount between [200, i64::MAX]`: [Upper, Long, Huge];
/// - `Title between ["Roads", "Straße"]`: [Lower, Inside, Upper] — ordinal and
///   unfolded; `Title between ["A", "Z"]`: [Short, Lower, Inside, Upper, Huge]
///   — case-sensitive, and `Zebra` orders after `Z`;
/// - `Filter(PageCount between [100, 200])` over [Upper, Short, Upper]:
///   [Upper, Upper];
/// - `SeedHolons` with `SeedPredicate(Key between ["QryF5.RangeTarget.",
///   "QryF5.RangeTarget/"])`: [Inside];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(Name between ["B", "C"])`
///   over [Short, Lower, Long]: [Bob, Bob];
/// - `Filter(AllOf(PageCount between [100, 300], Not(Title between ["A",
///   "Z"])))`: [Long];
/// - failures, on empty input unless noted: reversed bounds
///   (`InvalidAuthoredOperands`) in a Filter, a SeedPredicate and an
///   ExpansionPredicate, over [Unknown] (absent optional subject), beside a
///   true branch (over [Short]), and after a completed `Distinct` (over
///   [Short]); Between on a Boolean (`UnsupportedOperator`); an upper bound
///   declared outside the subject type (`IncompatibleLiteralType`); a single
///   bound (`OperandCountMismatch`).
pub fn query_f5_between_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f5_between",
        "QRY-F5: Between predicates with two ordered authored bounds filter through Filter, \
         SeedPredicate and ExpansionPredicate, alone and composed, through the direct and \
         QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F5 Book/Person data".to_string()),
    )?;
    let book_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_DESCRIPTOR_KEY)?;
    let person_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_DESCRIPTOR_KEY,
    )?;
    let mut people = Vec::new();
    for (key, name) in
        [(PERSON_ADA_KEY, "Ada Lovelace"), (PERSON_BOB_KEY, "Bob"), (PERSON_CY_KEY, "Cy")]
    {
        people.push(stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            key,
            instance_properties("Name", name),
            &person_type,
            "Person",
        )?);
    }
    let [ada, bob, cy] = [people[0].clone(), people[1].clone(), people[2].clone()];
    for (key, title, page_count) in BOOKS {
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
        let authors = match key {
            BOOK_SHORT_KEY => vec![ada.clone(), bob.clone()],
            BOOK_LOWER_KEY => vec![cy.clone()],
            BOOK_LONG_KEY => vec![bob.clone()],
            _ => vec![],
        };
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
        Some("Commit the QRY-F5 Book/Person data".to_string()),
    )?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F5 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let short = saved(&mut test_case, &mut fixture_holons, BOOK_SHORT_KEY)?;
    let lower = saved(&mut test_case, &mut fixture_holons, BOOK_LOWER_KEY)?;
    let inside = saved(&mut test_case, &mut fixture_holons, BOOK_INSIDE_KEY)?;
    let upper = saved(&mut test_case, &mut fixture_holons, BOOK_UPPER_KEY)?;
    let long = saved(&mut test_case, &mut fixture_holons, BOOK_LONG_KEY)?;
    let unknown = saved(&mut test_case, &mut fixture_holons, BOOK_UNKNOWN_KEY)?;
    let huge = saved(&mut test_case, &mut fixture_holons, BOOK_HUGE_KEY)?;
    let bob = saved(&mut test_case, &mut fixture_holons, PERSON_BOB_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF5" };
    let string = |value: &str| MapString(value.to_string()).to_base_value();
    let integer = |value: i64| MapInteger(value).to_base_value();
    let (map_string, map_integer, map_boolean) =
        (types.map_string.clone(), types.map_integer.clone(), types.map_boolean.clone());
    let between_operator = types.between.clone();
    // One Filter per range.
    let mut filtered = Vec::new();
    for (label, property, declared, bounds) in [
        ("Pages", "PageCount", &map_integer, [integer(100), integer(200)]),
        ("PagesPoint", "PageCount", &map_integer, [integer(150), integer(150)]),
        ("PagesToMax", "PageCount", &map_integer, [integer(200), integer(i64::MAX)]),
        ("TitleOrdinal", "Title", &map_string, [string("Roads"), string("Straße")]),
        ("TitleCase", "Title", &map_string, [string("A"), string("Z")]),
    ] {
        let leaf = between_leaf(&mut author, &between_operator, label, property, declared, bounds)?;
        let filter = author.filter(label, leaf)?;
        filtered.push(author.query(label, filter)?);
    }
    let [pages, pages_point, pages_to_max, title_ordinal, title_case] =
        <[TestReference; 5]>::try_from(filtered).expect("one query per range");

    // Attachments.
    let seed_range = between_leaf(
        &mut author,
        &between_operator,
        "SeedRange",
        "Key",
        &map_string,
        [string("QryF5.RangeTarget."), string("QryF5.RangeTarget/")],
    )?;
    let seed = author.seed("SeedRange", Some(seed_range))?;
    let seed_query = author.query("SeedRange", seed)?;

    let named_b = between_leaf(
        &mut author,
        &between_operator,
        "NamedB",
        "Name",
        &map_string,
        [string("B"), string("C")],
    )?;
    let expand_named_b =
        author.authoring.expand("Expand.QryF5NamedB", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_named_b = author.relate(
        expand_named_b,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![named_b],
    )?;
    let expansion_query = author.query("NamedBExpansion", expand_named_b)?;

    // Composition with Not and AllOf.
    let mid_pages = between_leaf(
        &mut author,
        &between_operator,
        "MixedPages",
        "PageCount",
        &map_integer,
        [integer(100), integer(300)],
    )?;
    let capitalized = between_leaf(
        &mut author,
        &between_operator,
        "MixedTitle",
        "Title",
        &map_string,
        [string("A"), string("Z")],
    )?;
    let not_capitalized = author.not("MixedTitle", vec![capitalized])?;
    let mixed = author.all_of("Mixed", vec![mid_pages, not_capitalized])?;
    let mixed = author.filter("Mixed", mixed)?;
    let mixed_query = author.query("Mixed", mixed)?;

    // Failures: reversed bounds wherever a predicate can appear.
    let reversed = reversed_pages(&mut author, &between_operator, &map_integer, "Reversed")?;
    let reversed = author.filter("Reversed", reversed)?;
    let reversed_query = author.query("Reversed", reversed)?;

    let reversed_seed = between_leaf(
        &mut author,
        &between_operator,
        "ReversedSeed",
        "Key",
        &map_string,
        [string("QryF5.RangeTarget/"), string("QryF5.RangeTarget.")],
    )?;
    let reversed_seed = author.seed("ReversedSeed", Some(reversed_seed))?;
    let reversed_seed_query = author.query("ReversedSeed", reversed_seed)?;

    let reversed_names = between_leaf(
        &mut author,
        &between_operator,
        "ReversedNames",
        "Name",
        &map_string,
        [string("C"), string("B")],
    )?;
    let reversed_expansion =
        author.authoring.expand("Expand.QryF5ReversedNames", BOOK_TO_PERSON_RELATIONSHIP)?;
    let reversed_expansion = author.relate(
        reversed_expansion,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![reversed_names],
    )?;
    let reversed_expansion_query = author.query("ReversedExpansion", reversed_expansion)?;

    let any_pages = between_leaf(
        &mut author,
        &between_operator,
        "DecidedPages",
        "PageCount",
        &map_integer,
        [integer(i64::MIN), integer(i64::MAX)],
    )?;
    let decided_reversed =
        reversed_pages(&mut author, &between_operator, &map_integer, "DecidedReversed")?;
    let decided = author.any_of("Decided", vec![any_pages, decided_reversed])?;
    let decided = author.filter("Decided", decided)?;
    let decided_query = author.query("Decided", decided)?;

    let late_reversed =
        reversed_pages(&mut author, &between_operator, &map_integer, "LateReversed")?;
    let late_reversed = author.filter("LateReversed", late_reversed)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF5LateReversed")?;
    let late_distinct = author.authoring.next(late_distinct, late_reversed)?;
    let late_reversed_query = author.query("LateReversed", late_distinct)?;

    // Failures: domain, literal type and operand count.
    let boolean = between_leaf(
        &mut author,
        &between_operator,
        "Boolean",
        "IsPublished",
        &map_boolean,
        [false.to_base_value(), true.to_base_value()],
    )?;
    let boolean = author.filter("Boolean", boolean)?;
    let boolean_query = author.query("Boolean", boolean)?;

    let integer_lower = author.literal("StringUpperLower", &map_integer, integer(1))?;
    let string_upper = author.literal("StringUpperUpper", &map_string, string("2"))?;
    let string_upper = author.comparison(
        "StringUpper",
        "PageCount",
        &between_operator,
        vec![integer_lower, string_upper],
    )?;
    let string_upper = author.filter("StringUpper", string_upper)?;
    let string_upper_query = author.query("StringUpper", string_upper)?;

    let single_bound = author.literal("SingleBound", &map_integer, integer(1))?;
    let single_bound =
        author.comparison("SingleBound", "PageCount", &between_operator, vec![single_bound])?;
    let single_bound = author.filter("SingleBound", single_bound)?;
    let single_bound_query = author.query("SingleBound", single_bound)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;
        let all = [&short, &lower, &inside, &upper, &long, &unknown, &huge];

        for (query, input, expected, what) in [
            (
                &pages,
                books(&all),
                members(&[&lower, &inside, &upper]),
                "Filter(PageCount between 100 and 200) includes both bounds",
            ),
            (
                &pages_point,
                books(&all),
                members(&[&inside]),
                "Filter(PageCount between 150 and 150) matches only an equal value",
            ),
            (
                &pages_to_max,
                books(&all),
                members(&[&upper, &long, &huge]),
                "Filter(PageCount between 200 and i64::MAX)",
            ),
            (
                &title_ordinal,
                books(&all),
                members(&[&lower, &inside, &upper]),
                "Filter(Title between Roads and Straße) is ordinal and unfolded",
            ),
            (
                &title_case,
                books(&all),
                members(&[&short, &lower, &inside, &upper, &huge]),
                "Filter(Title between A and Z) is case-sensitive",
            ),
            (
                &pages,
                books(&[&upper, &short, &upper]),
                members(&[&upper, &upper]),
                "Filter(PageCount between 100 and 200) keeps order and duplicates",
            ),
            (
                &seed_query,
                QueryInputSpec::None,
                members(&[&inside]),
                "SeedHolons filtered by a Key range",
            ),
            (
                &expansion_query,
                books(&[&short, &lower, &long]),
                members(&[&bob, &bob]),
                "Expand filtered by ExpansionPredicate(Name between B and C)",
            ),
            (&mixed_query, books(&all), members(&[&long]), "Filter(AllOf(between, Not(between)))"),
        ] {
            test_case.add_execute_query_step(
                query.clone(),
                input,
                route,
                expected,
                Some(format!("{what} via {route:?}")),
            )?;
        }

        for (query, input, kind, what) in [
            (
                &reversed_query,
                books(&[]),
                HolonErrorKind::InvalidAuthoredOperands,
                "reversed bounds on empty input",
            ),
            (
                &reversed_query,
                books(&[&unknown]),
                HolonErrorKind::InvalidAuthoredOperands,
                "reversed bounds over an absent optional subject",
            ),
            (
                &reversed_seed_query,
                QueryInputSpec::None,
                HolonErrorKind::InvalidAuthoredOperands,
                "a reversed SeedPredicate range",
            ),
            (
                &reversed_expansion_query,
                books(&[]),
                HolonErrorKind::InvalidAuthoredOperands,
                "a reversed ExpansionPredicate range",
            ),
            (
                &decided_query,
                books(&[&short]),
                HolonErrorKind::InvalidAuthoredOperands,
                "reversed bounds beside a true branch",
            ),
            (
                &late_reversed_query,
                books(&[&short]),
                HolonErrorKind::InvalidAuthoredOperands,
                "reversed bounds after a completed Distinct",
            ),
            (
                &boolean_query,
                books(&[]),
                HolonErrorKind::UnsupportedOperator,
                "Between on a Boolean",
            ),
            (
                &string_upper_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "a string-typed upper bound on PageCount",
            ),
            (
                &single_bound_query,
                books(&[]),
                HolonErrorKind::OperandCountMismatch,
                "a single authored bound",
            ),
        ] {
            test_case.add_execute_query_step(
                query.clone(),
                input,
                route,
                error(kind),
                Some(format!("{what} fails with {kind:?} via {route:?}")),
            )?;
        }
    }

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

/// A Between leaf over `property` with both bounds declared as `declared`, in
/// authored order.
fn between_leaf(
    author: &mut PredicateAuthoring<'_, '_>,
    between: &TestReference,
    label: &str,
    property: &str,
    declared: &TestReference,
    [lower, upper]: [BaseValue; 2],
) -> Result<TestReference, HolonError> {
    let lower = author.literal(&format!("{label}Lower"), declared, lower)?;
    let upper = author.literal(&format!("{label}Upper"), declared, upper)?;
    author.comparison(label, property, between, vec![lower, upper])
}

/// A Between leaf over PageCount whose lower bound is ordered after its upper.
fn reversed_pages(
    author: &mut PredicateAuthoring<'_, '_>,
    between: &TestReference,
    map_integer: &TestReference,
    label: &str,
) -> Result<TestReference, HolonError> {
    let bounds = [MapInteger(200).to_base_value(), MapInteger(100).to_base_value()];
    between_leaf(author, between, label, "PageCount", map_integer, bounds)
}
