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

/// Committed Book/Person data. `Falling` has no Subtitle (an optional
/// property). `Plain`'s Title ends in pattern metacharacters. Only `Shouted`
/// carries `PrefixTarget` in its key, for the SeedPredicate prefix. Seed
/// predicates scan every saved holon in the shared conductor, so the marker
/// avoids the `SeedTarget` keys the F1 and F4 seeds select.
const BOOK_RISING_KEY: &str = "QryF6.Book.Rising";
const BOOK_FALLING_KEY: &str = "QryF6.Book.Falling";
const BOOK_SHOUTED_KEY: &str = "QryF6.PrefixTarget.Book.Shouted";
const BOOK_PLAIN_KEY: &str = "QryF6.Book.Plain";
/// (key, Title, Subtitle)
const BOOKS: [(&str, &str, Option<&str>); 4] = [
    (BOOK_RISING_KEY, "Straße der Graphen", Some("A Field Guide")),
    (BOOK_FALLING_KEY, "Graphen der Straße", None),
    (BOOK_SHOUTED_KEY, "STRASSE", Some("Notes on Graphs")),
    (BOOK_PLAIN_KEY, "Atlas.*", Some("Roads")),
];
/// Authors: Rising -> [Ada, Bob], Falling -> [Cy], Plain -> [Bob].
const PERSON_ADA_KEY: &str = "QryF6.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF6.Person.Bob";
const PERSON_CY_KEY: &str = "QryF6.Person.Cy";

/// QRY-F6 (issue #865): literal StartsWith and EndsWith predicates through
/// `Filter`, `SeedPredicate` and `ExpansionPredicate`, alone and composed, on
/// both the direct and QueryDance routes. Every definition is transient.
/// Matching semantics in detail (Unicode cases, both policies, metadata,
/// extension operators) are owned by unit tests; this fixture proves the
/// loaded Core operators execute through every entry form and route.
///
/// Cases, over [Rising, Falling, Shouted, Plain] unless noted:
/// - `Title startsWith "STRASSE"`: [Rising, Shouted]; `Title endsWith
///   "STRASSE"`: [Falling, Shouted] — case folded, `ß` matching `SS`;
/// - `Title startsWith "der"`: [] — interior text matches no boundary;
/// - `Title endsWith ".*"`: [Plain] — metacharacters are literal;
/// - `Subtitle endsWith ""`: [Rising, Shouted, Plain] — the empty literal
///   matches every present value, and the absent optional Subtitle never;
/// - `Filter(Title startsWith "graph")` over [Falling, Rising, Falling]:
///   [Falling, Falling];
/// - `SeedHolons` with `SeedPredicate(Key startsWith "QryF6.PrefixTarget.")`:
///   [Shouted];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(Name startsWith "b")` over
///   [Rising, Falling, Plain]: [Bob, Bob];
/// - `Filter(AnyOf(AllOf(Title endsWith "strasse", Not(Title startsWith
///   "graph")), Subtitle endsWith "ROADS"))`: [Shouted, Plain];
/// - failures, on empty input unless noted: a boundary operator on an integer
///   and a Boolean (`UnsupportedOperator`); a literal declared outside the
///   required string type (`IncompatibleLiteralType`); a selector Book lacks
///   (`DescriptorDeclarationNotFound`); the integer case beside a true branch
///   (over [Rising]).
pub fn query_f6_string_matching_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f6_string_matching",
        "QRY-F6: literal StartsWith and EndsWith predicates filter through Filter, SeedPredicate \
         and ExpansionPredicate, alone and composed, through the direct and QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F6 Book/Person data".to_string()),
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
    for (key, title, subtitle) in BOOKS {
        let mut properties = instance_properties("Title", title);
        if let Some(subtitle) = subtitle {
            properties.insert(
                "Subtitle".to_property_name(),
                MapString(subtitle.to_string()).to_base_value(),
            );
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
            BOOK_RISING_KEY => vec![ada.clone(), bob.clone()],
            BOOK_FALLING_KEY => vec![cy.clone()],
            BOOK_PLAIN_KEY => vec![bob.clone()],
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
        Some("Commit the QRY-F6 Book/Person data".to_string()),
    )?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F6 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let rising = saved(&mut test_case, &mut fixture_holons, BOOK_RISING_KEY)?;
    let falling = saved(&mut test_case, &mut fixture_holons, BOOK_FALLING_KEY)?;
    let shouted = saved(&mut test_case, &mut fixture_holons, BOOK_SHOUTED_KEY)?;
    let plain = saved(&mut test_case, &mut fixture_holons, BOOK_PLAIN_KEY)?;
    let bob = saved(&mut test_case, &mut fixture_holons, PERSON_BOB_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF6" };
    let string = |value: &str| MapString(value.to_string()).to_base_value();
    let (map_string, map_integer) = (types.map_string.clone(), types.map_integer.clone());
    let (starts_with, ends_with) = (types.starts_with.clone(), types.ends_with.clone());

    // One Filter per boundary case.
    let mut filtered = Vec::new();
    for (label, property, operator, literal) in [
        ("TitleStarts", "Title", &starts_with, "STRASSE"),
        ("TitleEnds", "Title", &ends_with, "STRASSE"),
        ("TitleInterior", "Title", &starts_with, "der"),
        ("TitleMetacharacters", "Title", &ends_with, ".*"),
        ("SubtitleEmpty", "Subtitle", &ends_with, ""),
        ("TitleGraph", "Title", &starts_with, "graph"),
    ] {
        let leaf = author.compare(label, property, operator, &map_string, string(literal))?;
        let filter = author.filter(label, leaf)?;
        filtered.push(author.query(label, filter)?);
    }
    let [title_starts, title_ends, title_interior, title_metacharacters, subtitle_empty, title_graph] =
        <[TestReference; 6]>::try_from(filtered).expect("one query per boundary case");

    // Attachments.
    let seed_prefix = author.compare(
        "SeedPrefix",
        "Key",
        &starts_with,
        &map_string,
        string("QryF6.PrefixTarget."),
    )?;
    let seed = author.seed("SeedPrefix", Some(seed_prefix))?;
    let seed_query = author.query("SeedPrefix", seed)?;

    let named_b = author.compare("NamedB", "Name", &starts_with, &map_string, string("b"))?;
    let expand_named_b =
        author.authoring.expand("Expand.QryF6NamedB", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_named_b = author.relate(
        expand_named_b,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![named_b],
    )?;
    let expansion_query = author.query("NamedBExpansion", expand_named_b)?;

    // Composition with Not, AllOf and AnyOf.
    let ends_strasse =
        author.compare("MixedEnds", "Title", &ends_with, &map_string, string("strasse"))?;
    let starts_graph =
        author.compare("MixedStarts", "Title", &starts_with, &map_string, string("graph"))?;
    let not_graph = author.not("MixedStarts", vec![starts_graph])?;
    let strasse_not_graph = author.all_of("Mixed", vec![ends_strasse, not_graph])?;
    let roads =
        author.compare("MixedRoads", "Subtitle", &ends_with, &map_string, string("ROADS"))?;
    let mixed = author.any_of("Mixed", vec![strasse_not_graph, roads])?;
    let mixed = author.filter("Mixed", mixed)?;
    let mixed_query = author.query("Mixed", mixed)?;

    // Failures.
    let mut failing = Vec::new();
    for (label, property, operator, declared, value) in [
        ("IntegerPages", "PageCount", &starts_with, &map_string, string("1")),
        ("Boolean", "IsPublished", &ends_with, &map_string, string("true")),
        ("IntegerLiteral", "Title", &starts_with, &map_integer, MapInteger(1).to_base_value()),
        ("NameOnBooks", "Name", &ends_with, &map_string, string("b")),
    ] {
        let leaf = author.compare(label, property, operator, declared, value)?;
        let filter = author.filter(label, leaf)?;
        failing.push(author.query(label, filter)?);
    }
    let [integer_pages_query, boolean_query, integer_literal_query, name_on_books_query] =
        <[TestReference; 4]>::try_from(failing).expect("one query per failing leaf");

    let any_title =
        author.compare("DecidedTitle", "Title", &starts_with, &map_string, string(""))?;
    let decided_pages =
        author.compare("DecidedPages", "PageCount", &ends_with, &map_string, string("0"))?;
    let decided = author.any_of("Decided", vec![any_title, decided_pages])?;
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
        let all = [&rising, &falling, &shouted, &plain];

        for (query, input, expected, what) in [
            (
                &title_starts,
                books(&all),
                members(&[&rising, &shouted]),
                "Filter(Title startsWith STRASSE) folds case",
            ),
            (
                &title_ends,
                books(&all),
                members(&[&falling, &shouted]),
                "Filter(Title endsWith STRASSE) folds case",
            ),
            (
                &title_interior,
                books(&all),
                members(&[]),
                "Filter(Title startsWith der) ignores interior text",
            ),
            (
                &title_metacharacters,
                books(&all),
                members(&[&plain]),
                "Filter(Title endsWith .*) is literal",
            ),
            (
                &subtitle_empty,
                books(&all),
                members(&[&rising, &shouted, &plain]),
                "Filter(Subtitle endsWith \"\") skips an absent optional value",
            ),
            (
                &title_graph,
                books(&[&falling, &rising, &falling]),
                members(&[&falling, &falling]),
                "Filter(Title startsWith graph) keeps order and duplicates",
            ),
            (
                &seed_query,
                QueryInputSpec::None,
                members(&[&shouted]),
                "SeedHolons filtered by a Key prefix",
            ),
            (
                &expansion_query,
                books(&[&rising, &falling, &plain]),
                members(&[&bob, &bob]),
                "Expand filtered by ExpansionPredicate(Name startsWith b)",
            ),
            (
                &mixed_query,
                books(&all),
                members(&[&shouted, &plain]),
                "Filter(AnyOf(AllOf(endsWith, Not(startsWith)), endsWith))",
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
            (
                &integer_pages_query,
                books(&[]),
                HolonErrorKind::UnsupportedOperator,
                "StartsWith on an integer",
            ),
            (
                &boolean_query,
                books(&[]),
                HolonErrorKind::UnsupportedOperator,
                "EndsWith on a Boolean",
            ),
            (
                &integer_literal_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "an integer-typed prefix",
            ),
            (
                &name_on_books_query,
                books(&[]),
                HolonErrorKind::DescriptorDeclarationNotFound,
                "a selector Book lacks",
            ),
            (
                &decided_query,
                books(&[&rising]),
                HolonErrorKind::UnsupportedOperator,
                "EndsWith on an integer beside a true branch",
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
