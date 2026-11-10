use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, CONTENT_DIGEST_VALUE_TYPE_KEY,
    PERSON_DESCRIPTOR_KEY, PUBLICATION_STATUS_VALUE_TYPE_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};
use type_names::QueryRelationshipTypeName;

use super::query_f1_filter_fixture::{query_authoring, PredicateAuthoring, PredicateTypes};
use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

/// Committed Book/Person data. Titles in ordinal (code point) order are
/// `Atlas` < `Roads` < `STRASSE` < `Straße` < `atlas notes`: uppercase
/// precedes lowercase, and `Straße` is not folded to `STRASSE`. `Unknown` has no
/// PageCount (an optional property); `Huge` carries the largest integer. Only
/// `Long` carries `SeedTarget` in its key, for the SeedPredicate range.
const BOOK_SHORT_KEY: &str = "QryF4.Book.Short";
const BOOK_EXACT_KEY: &str = "QryF4.Book.Exact";
const BOOK_LONG_KEY: &str = "QryF4.SeedTarget.Book.Long";
const BOOK_UNKNOWN_KEY: &str = "QryF4.Book.Unknown";
const BOOK_HUGE_KEY: &str = "QryF4.Book.Huge";
/// (key, Title, PageCount)
const BOOKS: [(&str, &str, Option<i64>); 5] = [
    (BOOK_SHORT_KEY, "Atlas", Some(50)),
    (BOOK_EXACT_KEY, "Roads", Some(100)),
    (BOOK_LONG_KEY, "atlas notes", Some(300)),
    (BOOK_UNKNOWN_KEY, "Straße", None),
    (BOOK_HUGE_KEY, "STRASSE", Some(i64::MAX)),
];
/// Authors: Short -> [Ada, Bob], Exact -> [Cy], Long -> [Bob].
const PERSON_ADA_KEY: &str = "QryF4.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF4.Person.Bob";
const PERSON_CY_KEY: &str = "QryF4.Person.Cy";

/// QRY-F4 (issue #850): ordered comparison predicates (LessThan, GreaterThan,
/// LessThanOrEqual, GreaterThanOrEqual) through `Filter`, `SeedPredicate` and
/// `ExpansionPredicate`, alone and composed with Equals and Contains, on both
/// the direct and QueryDance routes. Every definition is transient.
///
/// Cases, over [Short, Exact, Long, Unknown, Huge] unless noted:
/// - `PageCount < 100`: [Short]; `> 100`: [Long, Huge]; `<= 100`:
///   [Short, Exact]; `>= 100`: [Exact, Long, Huge]: strict operators exclude
///   the boundary, inclusive ones include it, and the absent optional
///   PageCount never matches;
/// - `Title < "Roads"`: [Short]; `Title > "STRASSE"`: [Long, Unknown] — ordinal
///   and unfolded; `Title <= "Atlas"`: [Short] — lowercase sorts after;
/// - `Filter(PageCount > 100)` over [Long, Short, Long]: [Long, Long];
/// - `SeedHolons` with `SeedPredicate(AllOf(Key >= "QryF4.SeedTarget.", Key <
///   "QryF4.SeedTarget/"))`, a key-prefix range: [Long];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(Name >= "Bob")` over
///   [Short, Exact, Long]: [Bob, Cy, Bob];
/// - `Filter(AnyOf(AllOf(PageCount >= 100, Not(Title contains "atlas")),
///   Title = "Straße"))`: [Exact, Unknown, Huge];
/// - `Filter(PageCount <= 100) -> Distinct` and the reverse, over
///   [Short, Exact, Short]: [Short, Exact];
/// - failures, on empty input unless noted: ordered operators on a Boolean, an
///   enum, and an Equals-affording bytes type (`UnsupportedOperator`); literals
///   declared outside the subject type (`IncompatibleLiteralType`); selectors
///   the declared Book, root and Person types lack
///   (`DescriptorDeclarationNotFound`); the Boolean case after a completed
///   `Distinct` and beside a true branch (over [Short]).
///
/// Signed extrema below `i64::MAX`, required absence, and stored
/// representation errors are covered by unit tests: Commit rejects such Books.
pub fn query_f4_ordering_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f4_ordering",
        "QRY-F4: ordered comparison predicates filter through Filter, SeedPredicate and \
         ExpansionPredicate, alone and composed with Equals and Contains, through the direct \
         and QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F4 Book/Person data".to_string()),
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
            BOOK_EXACT_KEY => vec![cy.clone()],
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
        Some("Commit the QRY-F4 Book/Person data".to_string()),
    )?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F4 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let short = saved(&mut test_case, &mut fixture_holons, BOOK_SHORT_KEY)?;
    let exact = saved(&mut test_case, &mut fixture_holons, BOOK_EXACT_KEY)?;
    let long = saved(&mut test_case, &mut fixture_holons, BOOK_LONG_KEY)?;
    let unknown = saved(&mut test_case, &mut fixture_holons, BOOK_UNKNOWN_KEY)?;
    let huge = saved(&mut test_case, &mut fixture_holons, BOOK_HUGE_KEY)?;
    let bob = saved(&mut test_case, &mut fixture_holons, PERSON_BOB_KEY)?;
    let cy = saved(&mut test_case, &mut fixture_holons, PERSON_CY_KEY)?;
    let publication_status =
        saved(&mut test_case, &mut fixture_holons, PUBLICATION_STATUS_VALUE_TYPE_KEY)?;
    let content_digest = saved(&mut test_case, &mut fixture_holons, CONTENT_DIGEST_VALUE_TYPE_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF4" };
    let string = |value: &str| MapString(value.to_string()).to_base_value();
    let integer = |value: i64| MapInteger(value).to_base_value();
    let (map_string, map_integer, map_boolean) =
        (types.map_string.clone(), types.map_integer.clone(), types.map_boolean.clone());
    let (less_than, greater_than, less_than_or_equal, greater_than_or_equal) = (
        types.less_than.clone(),
        types.greater_than.clone(),
        types.less_than_or_equal.clone(),
        types.greater_than_or_equal.clone(),
    );

    // One Filter per operator and domain.
    let mut filtered = Vec::new();
    for (label, property, operator, declared, value) in [
        ("PagesBelow", "PageCount", &less_than, &map_integer, integer(100)),
        ("PagesAbove", "PageCount", &greater_than, &map_integer, integer(100)),
        ("PagesAtMost", "PageCount", &less_than_or_equal, &map_integer, integer(100)),
        ("PagesAtLeast", "PageCount", &greater_than_or_equal, &map_integer, integer(100)),
        ("TitleBelow", "Title", &less_than, &map_string, string("Roads")),
        ("TitleAbove", "Title", &greater_than, &map_string, string("STRASSE")),
        ("TitleAtMost", "Title", &less_than_or_equal, &map_string, string("Atlas")),
    ] {
        let leaf = author.compare(label, property, operator, declared, value)?;
        let filter = author.filter(label, leaf)?;
        filtered.push(author.query(label, filter)?);
    }
    let [pages_below, pages_above, pages_at_most, pages_at_least, title_below, title_above, title_at_most] =
        <[TestReference; 7]>::try_from(filtered).expect("one query per operator and domain");

    // Attachments.
    let seed_from = author.compare(
        "SeedFrom",
        "Key",
        &greater_than_or_equal,
        &map_string,
        string("QryF4.SeedTarget."),
    )?;
    let seed_until =
        author.compare("SeedUntil", "Key", &less_than, &map_string, string("QryF4.SeedTarget/"))?;
    let seed_range = author.all_of("SeedRange", vec![seed_from, seed_until])?;
    let seed = author.seed("SeedRange", Some(seed_range))?;
    let seed_query = author.query("SeedRange", seed)?;

    let from_bob =
        author.compare("FromBob", "Name", &greater_than_or_equal, &map_string, string("Bob"))?;
    let expand_from_bob =
        author.authoring.expand("Expand.QryF4FromBob", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_from_bob = author.relate(
        expand_from_bob,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![from_bob],
    )?;
    let expansion_query = author.query("FromBobExpansion", expand_from_bob)?;

    // Composition with Not, AllOf, AnyOf, Equals and Contains.
    let long_enough = author.compare(
        "MixedPages",
        "PageCount",
        &greater_than_or_equal,
        &map_integer,
        integer(100),
    )?;
    let atlas_title = author.contains("MixedAtlas", "Title", "atlas")?;
    let not_atlas = author.not("MixedAtlas", vec![atlas_title])?;
    let long_not_atlas = author.all_of("Mixed", vec![long_enough, not_atlas])?;
    let strasse = author.equals("MixedStrasse", "Title", &map_string, string("Straße"))?;
    let mixed = author.any_of("Mixed", vec![long_not_atlas, strasse])?;
    let mixed = author.filter("Mixed", mixed)?;
    let mixed_query = author.query("Mixed", mixed)?;

    // Composition with Distinct, in both orders.
    let at_most =
        author.compare("AtMost", "PageCount", &less_than_or_equal, &map_integer, integer(100))?;
    let filter_first = author.filter("BeforeDistinct", at_most.clone())?;
    let distinct_after = author.authoring.distinct("Distinct.QryF4AfterFilter")?;
    let filter_first = author.authoring.next(filter_first, distinct_after)?;
    let filter_then_distinct = author.query("FilterThenDistinct", filter_first)?;
    let filter_after = author.filter("AfterDistinct", at_most)?;
    let distinct_first = author.authoring.distinct("Distinct.QryF4BeforeFilter")?;
    let distinct_first = author.authoring.next(distinct_first, filter_after)?;
    let distinct_then_filter = author.query("DistinctThenFilter", distinct_first)?;

    // Failures.
    let mut failing = Vec::new();
    for (label, property, operator, declared, value) in [
        ("Boolean", "IsPublished", &greater_than, &map_boolean, true.to_base_value()),
        (
            "Enum",
            "PublicationStatus",
            &less_than,
            &publication_status,
            MapEnumValue(MapString::from("Draft")).to_base_value(),
        ),
        (
            "Bytes",
            "ContentDigest",
            &greater_than_or_equal,
            &content_digest,
            MapBytes(vec![1]).to_base_value(),
        ),
        ("StringPages", "PageCount", &greater_than, &map_string, string("100")),
        ("IntegerTitle", "Title", &less_than_or_equal, &map_integer, integer(1)),
        ("NameOnBooks", "Name", &less_than, &map_string, string("Bob")),
    ] {
        let leaf = author.compare(label, property, operator, declared, value)?;
        let filter = author.filter(label, leaf)?;
        failing.push(author.query(label, filter)?);
    }
    let [boolean_query, enum_query, bytes_query, string_pages_query, integer_title_query, name_on_books_query] =
        <[TestReference; 6]>::try_from(failing).expect("one query per failing leaf");

    let seed_title =
        author.compare("SeedTitle", "Title", &greater_than, &map_string, string("A"))?;
    let title_seed = author.seed("SeedTitle", Some(seed_title))?;
    let title_seed_query = author.query("SeedTitle", title_seed)?;

    let person_title =
        author.compare("PersonTitle", "Title", &less_than, &map_string, string("Z"))?;
    let expand_titles =
        author.authoring.expand("Expand.QryF4PersonTitle", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_titles = author.relate(
        expand_titles,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![person_title],
    )?;
    let person_title_query = author.query("PersonTitle", expand_titles)?;

    let late_boolean = author.compare(
        "LateBoolean",
        "IsPublished",
        &greater_than,
        &map_boolean,
        true.to_base_value(),
    )?;
    let late_boolean = author.filter("LateBoolean", late_boolean)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF4LateBoolean")?;
    let late_distinct = author.authoring.next(late_distinct, late_boolean)?;
    let late_boolean_query = author.query("LateBoolean", late_distinct)?;

    let any_title =
        author.compare("DecidedTitle", "Title", &greater_than_or_equal, &map_string, string(""))?;
    let decided_boolean = author.compare(
        "DecidedBoolean",
        "IsPublished",
        &less_than,
        &map_boolean,
        true.to_base_value(),
    )?;
    let decided = author.any_of("Decided", vec![any_title, decided_boolean])?;
    let decided = author.filter("Decided", decided)?;
    let decided_query = author.query("Decided", decided)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;
        let all = [&short, &exact, &long, &unknown, &huge];

        for (query, input, expected, what) in [
            (&pages_below, books(&all), members(&[&short]), "Filter(PageCount < 100)"),
            (&pages_above, books(&all), members(&[&long, &huge]), "Filter(PageCount > 100)"),
            (
                &pages_at_most,
                books(&all),
                members(&[&short, &exact]),
                "Filter(PageCount <= 100) includes the boundary",
            ),
            (
                &pages_at_least,
                books(&all),
                members(&[&exact, &long, &huge]),
                "Filter(PageCount >= 100) skips an absent optional value",
            ),
            (&title_below, books(&all), members(&[&short]), "Filter(Title < Roads)"),
            (
                &title_above,
                books(&all),
                members(&[&long, &unknown]),
                "Filter(Title > STRASSE) is ordinal and unfolded",
            ),
            (
                &title_at_most,
                books(&all),
                members(&[&short]),
                "Filter(Title <= Atlas) is case-sensitive",
            ),
            (
                &pages_above,
                books(&[&long, &short, &long]),
                members(&[&long, &long]),
                "Filter(PageCount > 100) keeps order and duplicates",
            ),
            (
                &seed_query,
                QueryInputSpec::None,
                members(&[&long]),
                "SeedHolons filtered by a Key range",
            ),
            (
                &expansion_query,
                books(&[&short, &exact, &long]),
                members(&[&bob, &cy, &bob]),
                "Expand filtered by ExpansionPredicate(Name >= Bob)",
            ),
            (
                &mixed_query,
                books(&all),
                members(&[&exact, &unknown, &huge]),
                "Filter(AnyOf(AllOf(ordered, Not(Contains)), Equals))",
            ),
            (
                &filter_then_distinct,
                books(&[&short, &exact, &short]),
                members(&[&short, &exact]),
                "Filter -> Distinct",
            ),
            (
                &distinct_then_filter,
                books(&[&short, &exact, &short]),
                members(&[&short, &exact]),
                "Distinct -> Filter",
            ),
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
            (&boolean_query, books(&[]), HolonErrorKind::UnsupportedOperator, "ordering a Boolean"),
            (&enum_query, books(&[]), HolonErrorKind::UnsupportedOperator, "ordering an enum"),
            (
                &bytes_query,
                books(&[]),
                HolonErrorKind::UnsupportedOperator,
                "ordering an Equals-affording bytes type",
            ),
            (
                &string_pages_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "a string-typed literal on PageCount",
            ),
            (
                &integer_title_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "an integer-typed literal on Title",
            ),
            (
                &name_on_books_query,
                books(&[]),
                HolonErrorKind::DescriptorDeclarationNotFound,
                "a selector Book lacks",
            ),
            (
                &title_seed_query,
                QueryInputSpec::None,
                HolonErrorKind::DescriptorDeclarationNotFound,
                "a SeedPredicate selecting Title on the root type",
            ),
            (
                &person_title_query,
                books(&[]),
                HolonErrorKind::DescriptorDeclarationNotFound,
                "an ExpansionPredicate selecting Title on Person",
            ),
            (
                &late_boolean_query,
                books(&[&short]),
                HolonErrorKind::UnsupportedOperator,
                "ordering a Boolean after a completed Distinct",
            ),
            (
                &decided_query,
                books(&[&short]),
                HolonErrorKind::UnsupportedOperator,
                "ordering a Boolean beside a true branch",
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
