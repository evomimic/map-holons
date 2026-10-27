use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, CONTENT_DIGEST_VALUE_TYPE_KEY,
    PERSON_DESCRIPTOR_KEY, PUBLICATION_STATUS_VALUE_TYPE_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};
use type_names::{value_type_descriptor_key, CoreValueTypeName, QueryRelationshipTypeName};

use super::query_f1_filter_fixture::{query_authoring, PredicateAuthoring, PredicateTypes};
use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

/// Committed Book/Person data. `Straße` and `STRASSE` differ only under case
/// folding; `Atlas Notes` has no PageCount (an optional property) and is both
/// a Draft and an `atlas` Contains match; only `Straße` carries a ContentDigest.
const BOOK_STRASSE_KEY: &str = "QryF2.Book.Strasse";
const BOOK_UPPER_KEY: &str = "QryF2.Book.Upper";
const BOOK_ATLAS_KEY: &str = "QryF2.Book.Atlas";
const BOOK_NOTES_KEY: &str = "QryF2.Book.Notes";
/// (key, Title, PageCount, IsPublished, PublicationStatus, ContentDigest)
const BOOKS: [(&str, &str, Option<i64>, bool, &str, Option<&[u8]>); 4] = [
    (BOOK_STRASSE_KEY, "Straße", Some(120), true, "Published", Some(&[1, 2, 3])),
    (BOOK_UPPER_KEY, "STRASSE", Some(120), false, "Draft", None),
    (BOOK_ATLAS_KEY, "Atlas", Some(300), true, "Published", None),
    (BOOK_NOTES_KEY, "Atlas Notes", None, false, "Draft", None),
];
/// Authors: Straße -> [Ada, Bob], STRASSE -> [Ada], Atlas -> [Bob].
const PERSON_ADA_KEY: &str = "QryF2.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF2.Person.Bob";

/// QRY-F2 (issue #830): Equals comparison predicates through `Filter`,
/// `SeedPredicate` and `ExpansionPredicate`, alone and in flat `AnyOf` groups
/// with Contains, on both the direct and QueryDance routes. Every definition
/// is transient, and the executor asserts evaluation leaves it unchanged.
///
/// Cases:
/// - `Filter(Title = "Straße")` over [Straße, STRASSE, Atlas, Straße]:
///   [Straße, Straße] — case-sensitive, independent of Contains folding;
/// - `Filter(PageCount = 120)`, `Filter(IsPublished = true)`,
///   `Filter(PublicationStatus = Draft)` and `Filter(ContentDigest = [1,2,3])`,
///   one per scalar domain; an absent optional value never matches;
/// - `Filter(AnyOf(PublicationStatus = Draft, Title contains "atlas"))`: Notes
///   matches both leaves and is kept once;
/// - `Filter(AnyOf(PageCount = 120, PageCount = 300))`, an equality-only group;
/// - `SeedHolons` with `SeedPredicate(Key = <Atlas key>)`: [Atlas];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(Name = "Ada Lovelace")` over
///   [Straße, Atlas, STRASSE]: [Ada, Ada];
/// - `Filter -> Distinct` and `Distinct -> Filter`;
/// - failures, on empty input unless noted: Equals on canonical
///   `MapBytesValueType` (`UnsupportedOperator`), an undeclared enum literal
///   (`EnumVariantNotInSchema`), literals declared outside the subject type
///   (`IncompatibleLiteralType`), and the undeclared literal after a completed
///   `Distinct` over [Straße].
pub fn query_f2_equality_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f2_equality",
        "QRY-F2: Equals comparison predicates filter through Filter, SeedPredicate and \
         ExpansionPredicate, alone and mixed with Contains in flat AnyOf, through the direct \
         and QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F2 Book/Person data".to_string()),
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
    for (key, title, page_count, is_published, status, digest) in BOOKS {
        let mut properties = instance_properties("Title", title);
        if let Some(page_count) = page_count {
            properties
                .insert("PageCount".to_property_name(), MapInteger(page_count).to_base_value());
        }
        properties.insert("IsPublished".to_property_name(), is_published.to_base_value());
        properties.insert(
            "PublicationStatus".to_property_name(),
            MapEnumValue(MapString::from(status)).to_base_value(),
        );
        if let Some(digest) = digest {
            properties.insert(
                "ContentDigest".to_property_name(),
                MapBytes(digest.to_vec()).to_base_value(),
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
            BOOK_STRASSE_KEY => vec![ada.clone(), bob.clone()],
            BOOK_UPPER_KEY => vec![ada.clone()],
            BOOK_ATLAS_KEY => vec![bob.clone()],
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
        Some("Commit the QRY-F2 Book/Person data".to_string()),
    )?;
    test_case.add_match_saved_content_step()?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F2 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let strasse = saved(&mut test_case, &mut fixture_holons, BOOK_STRASSE_KEY)?;
    let upper = saved(&mut test_case, &mut fixture_holons, BOOK_UPPER_KEY)?;
    let atlas = saved(&mut test_case, &mut fixture_holons, BOOK_ATLAS_KEY)?;
    let notes = saved(&mut test_case, &mut fixture_holons, BOOK_NOTES_KEY)?;
    let ada = saved(&mut test_case, &mut fixture_holons, PERSON_ADA_KEY)?;

    // Each Equals literal declares the selected property's own ValueType.
    let value_type = |name: CoreValueTypeName, family: CoreValueTypeName| {
        value_type_descriptor_key(&name.as_value_name(), &family).0
    };
    let map_boolean = saved(
        &mut test_case,
        &mut fixture_holons,
        &value_type(CoreValueTypeName::MapBooleanValueType, CoreValueTypeName::BooleanValueType),
    )?;
    let map_bytes = saved(
        &mut test_case,
        &mut fixture_holons,
        &value_type(CoreValueTypeName::MapBytesValueType, CoreValueTypeName::BytesValueType),
    )?;
    let publication_status =
        saved(&mut test_case, &mut fixture_holons, PUBLICATION_STATUS_VALUE_TYPE_KEY)?;
    let content_digest = saved(&mut test_case, &mut fixture_holons, CONTENT_DIGEST_VALUE_TYPE_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF2" };
    let string = |value: &str| MapString(value.to_string()).to_base_value();
    let integer = |value: i64| MapInteger(value).to_base_value();
    let status = |variant: &str| MapEnumValue(MapString::from(variant)).to_base_value();
    let digest = |value: &[u8]| MapBytes(value.to_vec()).to_base_value();

    // One Filter per scalar domain.
    let mut filtered = Vec::new();
    for (label, property, declared, value) in [
        ("Title", "Title", &types.map_string, string("Straße")),
        ("PageCount", "PageCount", &types.map_integer, integer(120)),
        ("IsPublished", "IsPublished", &map_boolean, true.to_base_value()),
        ("Status", "PublicationStatus", &publication_status, status("Draft")),
        ("Digest", "ContentDigest", &content_digest, digest(&[1, 2, 3])),
    ] {
        let leaf = author.equals(label, property, declared, value)?;
        let filter = author.filter(label, leaf)?;
        filtered.push(author.query(label, filter)?);
    }
    let [title_query, page_count_query, published_query, draft_query, digest_query] =
        <[TestReference; 5]>::try_from(filtered).expect("one query per scalar domain");

    // Flat AnyOf, mixed and equality-only.
    let draft =
        author.equals("MixedDraft", "PublicationStatus", &publication_status, status("Draft"))?;
    let atlas_title = author.contains("MixedAtlas", "Title", "atlas")?;
    let mixed = author.any_of("Mixed", vec![draft, atlas_title])?;
    let mixed = author.filter("Mixed", mixed)?;
    let mixed_query = author.query("Mixed", mixed)?;

    let pages_120 = author.equals("Pages120", "PageCount", &types.map_integer, integer(120))?;
    let pages_300 = author.equals("Pages300", "PageCount", &types.map_integer, integer(300))?;
    let pages = author.any_of("Pages", vec![pages_120, pages_300])?;
    let pages = author.filter("Pages", pages)?;
    let pages_query = author.query("Pages", pages)?;

    // Attachments.
    let seed_key = author.equals("SeedKey", "Key", &types.map_string, string(BOOK_ATLAS_KEY))?;
    let seed = author.seed("SeedKey", Some(seed_key))?;
    let seed_query = author.query("SeedKey", seed)?;

    let ada_name = author.equals("AdaName", "Name", &types.map_string, string("Ada Lovelace"))?;
    let expand_to_ada = author.authoring.expand("Expand.QryF2Ada", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_to_ada = author.relate(
        expand_to_ada,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![ada_name],
    )?;
    let expansion_query = author.query("AdaExpansion", expand_to_ada)?;

    // Composition with Distinct, in both orders.
    let published = author.equals(
        "Published",
        "PublicationStatus",
        &publication_status,
        status("Published"),
    )?;
    let filter_first = author.filter("BeforeDistinct", published.clone())?;
    let distinct_after = author.authoring.distinct("Distinct.QryF2AfterFilter")?;
    let filter_first = author.authoring.next(filter_first, distinct_after)?;
    let filter_then_distinct = author.query("FilterThenDistinct", filter_first)?;
    let filter_after = author.filter("AfterDistinct", published)?;
    let distinct_first = author.authoring.distinct("Distinct.QryF2BeforeFilter")?;
    let distinct_first = author.authoring.next(distinct_first, filter_after)?;
    let distinct_then_filter = author.query("DistinctThenFilter", distinct_first)?;

    // Failures.
    let cover = author.equals("Cover", "CoverDigest", &map_bytes, digest(&[0, 127, 255]))?;
    let cover = author.filter("Cover", cover)?;
    let cover_query = author.query("Cover", cover)?;

    let archived =
        author.equals("Archived", "PublicationStatus", &publication_status, status("Archived"))?;
    let archived_filter = author.filter("Archived", archived.clone())?;
    let archived_query = author.query("Archived", archived_filter)?;
    let late_archived = author.filter("LateArchived", archived)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF2LateArchived")?;
    let late_distinct = author.authoring.next(late_distinct, late_archived)?;
    let late_archived_query = author.query("LateArchived", late_distinct)?;

    let integer_title = author.equals("IntegerTitle", "Title", &types.map_integer, integer(5))?;
    let integer_title = author.filter("IntegerTitle", integer_title)?;
    let integer_title_query = author.query("IntegerTitle", integer_title)?;

    let string_pages =
        author.equals("StringPages", "PageCount", &types.map_string, string("120"))?;
    let string_pages = author.filter("StringPages", string_pages)?;
    let string_pages_query = author.query("StringPages", string_pages)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;
        let all = [&strasse, &upper, &atlas, &notes];

        for (query, input, expected, what) in [
            (
                &title_query,
                books(&[&strasse, &upper, &atlas, &strasse]),
                members(&[&strasse, &strasse]),
                "Filter(Title = Straße) is case-sensitive and keeps duplicates",
            ),
            (
                &page_count_query,
                books(&all),
                members(&[&strasse, &upper]),
                "Filter(PageCount = 120) skips an absent optional value",
            ),
            (&published_query, books(&all), members(&[&strasse, &atlas]), "Filter(IsPublished)"),
            (
                &draft_query,
                books(&all),
                members(&[&upper, &notes]),
                "Filter(PublicationStatus = Draft)",
            ),
            (
                &digest_query,
                books(&all),
                members(&[&strasse]),
                "Filter(ContentDigest) on an Equals-affording bytes type",
            ),
            (
                &mixed_query,
                books(&all),
                members(&[&upper, &atlas, &notes]),
                "Filter(AnyOf(Equals, Contains)) keeps each occurrence once",
            ),
            (
                &pages_query,
                books(&all),
                members(&[&strasse, &upper, &atlas]),
                "Filter(AnyOf(Equals, Equals))",
            ),
            (
                &seed_query,
                QueryInputSpec::None,
                members(&[&atlas]),
                "SeedHolons filtered by SeedPredicate(Key =)",
            ),
            (
                &expansion_query,
                books(&[&strasse, &atlas, &upper]),
                members(&[&ada, &ada]),
                "Expand filtered by ExpansionPredicate(Name =)",
            ),
            (
                &filter_then_distinct,
                books(&[&strasse, &atlas, &strasse, &upper]),
                members(&[&strasse, &atlas]),
                "Filter -> Distinct",
            ),
            (
                &distinct_then_filter,
                books(&[&strasse, &atlas, &strasse, &upper]),
                members(&[&strasse, &atlas]),
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
            (
                &cover_query,
                books(&[]),
                HolonErrorKind::UnsupportedOperator,
                "Equals on canonical MapBytesValueType",
            ),
            (
                &archived_query,
                books(&[]),
                HolonErrorKind::EnumVariantNotInSchema,
                "an undeclared enum literal",
            ),
            (
                &late_archived_query,
                books(&[&strasse]),
                HolonErrorKind::EnumVariantNotInSchema,
                "an undeclared enum literal after a completed Distinct",
            ),
            (
                &integer_title_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "an integer-typed literal on Title",
            ),
            (
                &string_pages_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "a string-typed literal on PageCount",
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
