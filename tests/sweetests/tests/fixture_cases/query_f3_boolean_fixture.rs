use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
    PUBLICATION_STATUS_VALUE_TYPE_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};
use type_names::QueryRelationshipTypeName;

use super::query_f1_filter_fixture::{query_authoring, PredicateAuthoring, PredicateTypes};
use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

/// Committed Book/Person data. `Bare Roads` has no Subtitle (an optional
/// property); `Atlas` and `Drafted Roads` have a `draft` Subtitle; only
/// `Drafted Roads` is a Draft.
const BOOK_ATLAS_KEY: &str = "QryF3.Book.Atlas";
const BOOK_ROADS_KEY: &str = "QryF3.Book.Roads";
const BOOK_BARE_KEY: &str = "QryF3.Book.Bare";
const BOOK_DRAFTED_KEY: &str = "QryF3.Book.Drafted";
/// (key, Title, Subtitle, PublicationStatus)
const BOOKS: [(&str, &str, Option<&str>, &str); 4] = [
    (BOOK_ATLAS_KEY, "Atlas", Some("A draft atlas"), "Published"),
    (BOOK_ROADS_KEY, "Roads", Some("Final edition"), "Published"),
    (BOOK_BARE_KEY, "Bare Roads", None, "Published"),
    (BOOK_DRAFTED_KEY, "Drafted Roads", Some("draft"), "Draft"),
];
/// Authors: Atlas -> [Ada, Bob], Roads -> [Ada], Drafted Roads -> [Bob].
const PERSON_ADA_KEY: &str = "QryF3.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF3.Person.Bob";

/// QRY-F3 (issue #842): recursive `Not`, `AllOf` and nested `AnyOf` through
/// `Filter`, `SeedPredicate` and `ExpansionPredicate`, on both the direct and
/// QueryDance routes. Every definition is transient, and the executor asserts
/// that evaluation leaves the whole graph unchanged, cyclic graphs included.
///
/// Cases, over [Atlas, Roads, Bare Roads, Drafted Roads] unless noted:
/// - `Not(Subtitle contains "draft")`: [Roads, Bare Roads] — an absent
///   optional Subtitle makes the leaf false and its negation true;
/// - `AllOf(Published, Title contains "atlas")` over [Atlas, Roads, Atlas,
///   Drafted Roads]: [Atlas, Atlas];
/// - `Not(Not(Title contains "roads"))` and the singleton
///   `AllOf(Title contains "roads")` both equal the leaf;
/// - `AnyOf(AllOf(L, Subtitle contains "final"), AllOf(L, Draft))` with one
///   shared `L = Title contains "roads"`: [Roads, Drafted Roads];
/// - `AllOf(Published, AnyOf(Title contains "atlas", Not(Subtitle contains
///   "draft")))`, mixing Equals and Contains: [Atlas, Roads, Bare Roads];
/// - `SeedHolons` with `SeedPredicate(AllOf(Key = <Atlas key>, Not(Key
///   contains "Drafted")))`: [Atlas];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(AnyOf(Name contains
///   "lovelace", Not(Name = "Bob")))` over [Atlas, Roads, Drafted Roads]:
///   [Ada, Ada];
/// - the mixed predicate before and after `Distinct`;
/// - failures, on empty input unless noted: a `Not` without a child, a `Not`
///   with two, an empty `AllOf`, a self-referencing `Not`, a two-node cycle,
///   an unsupported LessThan leaf beside a true branch (over [Roads]), an
///   undeclared selector beside a false branch, and the cycle after a
///   completed `Distinct` (over [Roads]).
///
/// Required-absence and stored-representation errors under composites are
/// covered by unit tests only: Commit rejects such Books.
pub fn query_f3_boolean_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f3_boolean",
        "QRY-F3: Not, AllOf and nested AnyOf filter through Filter, SeedPredicate and \
         ExpansionPredicate through the direct and QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F3 Book/Person data".to_string()),
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
    for (key, name) in [(PERSON_ADA_KEY, "Ada Lovelace"), (PERSON_BOB_KEY, "Bob")] {
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
    let [ada, bob] = [people[0].clone(), people[1].clone()];
    for (key, title, subtitle, status) in BOOKS {
        let mut properties = instance_properties("Title", title);
        if let Some(subtitle) = subtitle {
            properties
                .insert("Subtitle".to_property_name(), MapString::from(subtitle).to_base_value());
        }
        properties.insert(
            "PublicationStatus".to_property_name(),
            MapEnumValue(MapString::from(status)).to_base_value(),
        );
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
            BOOK_ATLAS_KEY => vec![ada.clone(), bob.clone()],
            BOOK_ROADS_KEY => vec![ada.clone()],
            BOOK_DRAFTED_KEY => vec![bob.clone()],
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
        Some("Commit the QRY-F3 Book/Person data".to_string()),
    )?;
    test_case.add_match_saved_content_step()?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F3 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let atlas = saved(&mut test_case, &mut fixture_holons, BOOK_ATLAS_KEY)?;
    let roads = saved(&mut test_case, &mut fixture_holons, BOOK_ROADS_KEY)?;
    let bare = saved(&mut test_case, &mut fixture_holons, BOOK_BARE_KEY)?;
    let drafted = saved(&mut test_case, &mut fixture_holons, BOOK_DRAFTED_KEY)?;
    let ada = saved(&mut test_case, &mut fixture_holons, PERSON_ADA_KEY)?;
    let publication_status =
        saved(&mut test_case, &mut fixture_holons, PUBLICATION_STATUS_VALUE_TYPE_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF3" };
    let string = |value: &str| MapString(value.to_string()).to_base_value();
    let status = |variant: &str| MapEnumValue(MapString::from(variant)).to_base_value();

    // Truth-table shapes.
    let draft_subtitle = author.contains("NotDraft", "Subtitle", "draft")?;
    let not_draft = author.not("NotDraft", vec![draft_subtitle])?;
    let not_draft = author.filter("NotDraft", not_draft)?;
    let not_draft_query = author.query("NotDraft", not_draft)?;

    let published = author.equals(
        "PublishedAtlasStatus",
        "PublicationStatus",
        &publication_status,
        status("Published"),
    )?;
    let atlas_title = author.contains("PublishedAtlasTitle", "Title", "atlas")?;
    let published_atlas = author.all_of("PublishedAtlas", vec![published, atlas_title])?;
    let published_atlas = author.filter("PublishedAtlas", published_atlas)?;
    let published_atlas_query = author.query("PublishedAtlas", published_atlas)?;

    let roads_title = author.contains("DoubleNot", "Title", "roads")?;
    let inner_not = author.not("DoubleNotInner", vec![roads_title])?;
    let double_not = author.not("DoubleNot", vec![inner_not])?;
    let double_not = author.filter("DoubleNot", double_not)?;
    let double_not_query = author.query("DoubleNot", double_not)?;

    let roads_title = author.contains("Singleton", "Title", "roads")?;
    let singleton = author.all_of("Singleton", vec![roads_title])?;
    let singleton = author.filter("Singleton", singleton)?;
    let singleton_query = author.query("Singleton", singleton)?;

    // One leaf definition shared by two branches is not a cycle.
    let shared = author.contains("Shared", "Title", "roads")?;
    let final_subtitle = author.contains("SharedFinal", "Subtitle", "final")?;
    let final_branch = author.all_of("SharedFinal", vec![shared.clone(), final_subtitle])?;
    let draft_status =
        author.equals("SharedDraft", "PublicationStatus", &publication_status, status("Draft"))?;
    let draft_branch = author.all_of("SharedDraft", vec![shared, draft_status])?;
    let shared = author.any_of("Shared", vec![final_branch, draft_branch])?;
    let shared = author.filter("Shared", shared)?;
    let shared_query = author.query("Shared", shared)?;

    // Equals and Contains leaves at three levels; reused below with Distinct.
    let mut mixed = |label: &str| -> Result<TestReference, HolonError> {
        let published =
            author.equals(label, "PublicationStatus", &publication_status, status("Published"))?;
        let atlas_title = author.contains(&format!("{label}Atlas"), "Title", "atlas")?;
        let draft_subtitle = author.contains(&format!("{label}Draft"), "Subtitle", "draft")?;
        let not_draft = author.not(label, vec![draft_subtitle])?;
        let either = author.any_of(label, vec![atlas_title, not_draft])?;
        let all_of = author.all_of(label, vec![published, either])?;
        author.filter(label, all_of)
    };
    let mixed_filter = mixed("Mixed")?;
    let filter_first = mixed("BeforeDistinct")?;
    let filter_after = mixed("AfterDistinct")?;
    let mixed_query = author.query("Mixed", mixed_filter)?;
    let distinct_after = author.authoring.distinct("Distinct.QryF3AfterFilter")?;
    let filter_first = author.authoring.next(filter_first, distinct_after)?;
    let filter_then_distinct = author.query("FilterThenDistinct", filter_first)?;
    let distinct_first = author.authoring.distinct("Distinct.QryF3BeforeFilter")?;
    let distinct_first = author.authoring.next(distinct_first, filter_after)?;
    let distinct_then_filter = author.query("DistinctThenFilter", distinct_first)?;

    // Attachments.
    let seed_key = author.equals("SeedKey", "Key", &types.map_string, string(BOOK_ATLAS_KEY))?;
    let drafted_key = author.contains("SeedNotDrafted", "Key", "Drafted")?;
    let not_drafted = author.not("SeedNotDrafted", vec![drafted_key])?;
    let seed_predicate = author.all_of("Seed", vec![seed_key, not_drafted])?;
    let seed = author.seed("Seed", Some(seed_predicate))?;
    let seed_query = author.query("Seed", seed)?;

    let lovelace = author.contains("ExpandLovelace", "Name", "lovelace")?;
    let bob_name = author.equals("ExpandBob", "Name", &types.map_string, string("Bob"))?;
    let not_bob = author.not("ExpandBob", vec![bob_name])?;
    let expansion_predicate = author.any_of("Expand", vec![lovelace, not_bob])?;
    let expand = author.authoring.expand("Expand.QryF3Authors", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand = author.relate(
        expand,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![expansion_predicate],
    )?;
    let expansion_query = author.query("Expansion", expand)?;

    // Failures.
    let childless = author.not("Childless", vec![])?;
    let childless = author.filter("Childless", childless)?;
    let childless_query = author.query("Childless", childless)?;

    let first = author.contains("TwoChildrenA", "Title", "a")?;
    let second = author.contains("TwoChildrenB", "Title", "b")?;
    let two_children = author.not("TwoChildren", vec![first, second])?;
    let two_children = author.filter("TwoChildren", two_children)?;
    let two_children_query = author.query("TwoChildren", two_children)?;

    let empty = author.all_of("Empty", vec![])?;
    let empty = author.filter("Empty", empty)?;
    let empty_query = author.query("Empty", empty)?;

    let self_cycle = author.not("SelfCycle", vec![])?;
    let self_cycle = author.relate(
        self_cycle.clone(),
        QueryRelationshipTypeName::Predicate,
        vec![self_cycle],
    )?;
    let self_cycle_filter = author.filter("SelfCycle", self_cycle.clone())?;
    let self_cycle_query = author.query("SelfCycle", self_cycle_filter)?;
    let late_cycle = author.filter("LateCycle", self_cycle)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF3LateCycle")?;
    let late_distinct = author.authoring.next(late_distinct, late_cycle)?;
    let late_cycle_query = author.query("LateCycle", late_distinct)?;

    // AllOf -> [leaf, Not -> AllOf].
    let cycle_leaf = author.contains("TwoNodeCycle", "Title", "x")?;
    let cycle_all_of = author.all_of("TwoNodeCycle", vec![cycle_leaf])?;
    let cycle_not = author.not("TwoNodeCycle", vec![cycle_all_of.clone()])?;
    let cycle_all_of =
        author.relate(cycle_all_of, QueryRelationshipTypeName::Predicates, vec![cycle_not])?;
    let two_node_cycle = author.filter("TwoNodeCycle", cycle_all_of)?;
    let two_node_cycle_query = author.query("TwoNodeCycle", two_node_cycle)?;

    let any_title = author.contains("UnsupportedTrue", "Title", "")?;
    let less_than_operand =
        author.literal("UnsupportedLessThan", &types.map_string, string("Roads"))?;
    let less_than = author.comparison(
        "UnsupportedLessThan",
        "Title",
        &types.less_than,
        vec![less_than_operand],
    )?;
    let unsupported = author.any_of("Unsupported", vec![any_title, less_than])?;
    let unsupported = author.filter("Unsupported", unsupported)?;
    let unsupported_query = author.query("Unsupported", unsupported)?;

    let no_title = author.contains("Undeclared", "Title", "no such title")?;
    let undeclared = author.contains("UndeclaredMissing", "Missing", "x")?;
    let undeclared = author.not("Undeclared", vec![undeclared])?;
    let undeclared = author.all_of("Undeclared", vec![no_title, undeclared])?;
    let undeclared = author.filter("Undeclared", undeclared)?;
    let undeclared_query = author.query("Undeclared", undeclared)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;
        let all = [&atlas, &roads, &bare, &drafted];

        for (query, input, expected, what) in [
            (
                &not_draft_query,
                books(&all),
                members(&[&roads, &bare]),
                "Not(Contains) is true for an absent optional value",
            ),
            (
                &published_atlas_query,
                books(&[&atlas, &roads, &atlas, &drafted]),
                members(&[&atlas, &atlas]),
                "AllOf(Equals, Contains) keeps order and duplicates",
            ),
            (
                &double_not_query,
                books(&all),
                members(&[&roads, &bare, &drafted]),
                "Not(Not(leaf)) equals the leaf",
            ),
            (
                &singleton_query,
                books(&all),
                members(&[&roads, &bare, &drafted]),
                "a singleton AllOf equals its child",
            ),
            (
                &shared_query,
                books(&all),
                members(&[&roads, &drafted]),
                "a child shared by two branches is not a cycle",
            ),
            (
                &mixed_query,
                books(&all),
                members(&[&atlas, &roads, &bare]),
                "AllOf(Equals, AnyOf(Contains, Not(Contains)))",
            ),
            (
                &seed_query,
                QueryInputSpec::None,
                members(&[&atlas]),
                "SeedHolons filtered by SeedPredicate(AllOf(Key =, Not(Key contains)))",
            ),
            (
                &expansion_query,
                books(&[&atlas, &roads, &drafted]),
                members(&[&ada, &ada]),
                "Expand filtered by ExpansionPredicate(AnyOf(Contains, Not(Equals)))",
            ),
            (
                &filter_then_distinct,
                books(&[&atlas, &drafted, &atlas, &roads]),
                members(&[&atlas, &roads]),
                "nested Filter -> Distinct",
            ),
            (
                &distinct_then_filter,
                books(&[&atlas, &drafted, &atlas, &roads]),
                members(&[&atlas, &roads]),
                "Distinct -> nested Filter",
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
                &childless_query,
                books(&[]),
                HolonErrorKind::MissingRequiredRelationship,
                "a Not without a child",
            ),
            (
                &two_children_query,
                books(&[]),
                HolonErrorKind::MultipleRelatedHolons,
                "a Not with two children",
            ),
            (&empty_query, books(&[]), HolonErrorKind::InvalidParameter, "an empty AllOf"),
            (
                &self_cycle_query,
                books(&[]),
                HolonErrorKind::InvalidParameter,
                "a Not negating itself",
            ),
            (
                &two_node_cycle_query,
                books(&[]),
                HolonErrorKind::InvalidParameter,
                "an AllOf reached again through its Not child",
            ),
            (
                &unsupported_query,
                books(&[&roads]),
                HolonErrorKind::NotImplemented,
                "a LessThan leaf beside a true branch",
            ),
            (
                &undeclared_query,
                books(&[]),
                HolonErrorKind::DescriptorDeclarationNotFound,
                "an undeclared selector beside a false branch",
            ),
            (
                &late_cycle_query,
                books(&[&roads]),
                HolonErrorKind::InvalidParameter,
                "a cyclic predicate after a completed Distinct",
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
