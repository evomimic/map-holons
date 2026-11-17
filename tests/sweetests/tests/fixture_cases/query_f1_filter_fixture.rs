use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, QueryExpectation, QueryInputSpec,
    QueryRoute, TestCaseInit, TestReference,
};
use type_names::{
    holon_type_descriptor_key, operator_type_descriptor_key, value_type_descriptor_key,
    CoreOperatorTypeName, CoreRelationshipTypeName, CoreValueTypeName, QueryHolonTypeName,
    QueryPropertyTypeName, QueryRelationshipTypeName,
};

use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};
use super::query_qry4a_order_paginate_fixture::{selector, TransientAuthoring};

/// Committed Book/Person data. Titles exercise Unicode case folding
/// (`Straße` matches `STRASSE`); `Atlas` has no Subtitle (an optional
/// property); `Double` matches an `atlas` search in both Title and Subtitle.
/// `Atlas` alone carries `SeedTarget` in its key, for the SeedPredicate case.
const BOOK_STRASSE_KEY: &str = "QryF1.Book.Strasse";
const BOOK_ATLAS_KEY: &str = "QryF1.SeedTarget.Book.Atlas";
const BOOK_ROADS_KEY: &str = "QryF1.Book.Roads";
const BOOK_PLAIN_KEY: &str = "QryF1.Book.Plain";
const BOOK_DOUBLE_KEY: &str = "QryF1.Book.Double";
const BOOKS: [(&str, &str, Option<&str>); 5] = [
    (BOOK_STRASSE_KEY, "Hauptstraße Guide", Some("City walks")),
    (BOOK_ATLAS_KEY, "World Atlas", None),
    (BOOK_ROADS_KEY, "Roads", Some("A STRASSE atlas")),
    (BOOK_PLAIN_KEY, "Plain", Some("nothing to see")),
    (BOOK_DOUBLE_KEY, "Atlas of Atlases", Some("every atlas")),
];
/// Authors: Strasse -> [Ada, Bob], Atlas -> [Bob], Roads -> [Ada], Plain -> [Cy].
const PERSON_ADA_KEY: &str = "QryF1.Person.Ada";
const PERSON_BOB_KEY: &str = "QryF1.Person.Bob";
const PERSON_CY_KEY: &str = "QryF1.Person.Cy";
const PERSON_TO_BOOK_RELATIONSHIP: &str = "AuthorOf";

/// QRY-F1 (issue #797): Contains filtering through `Filter`, `SeedPredicate`
/// and `ExpansionPredicate`, with flat `AnyOf`, on both the direct and
/// QueryDance routes. Every definition (query, expressions, predicates,
/// literal operands) is a transient holon.
///
/// Cases:
/// - `Filter(Title contains "STRASSE")` over [Strasse, Atlas, Roads, Strasse]:
///   [Strasse, Strasse] — folding, order, and duplicate occurrences;
/// - `Filter(AnyOf(Title contains "atlas", Subtitle contains "atlas"))` over
///   [Strasse, Atlas, Roads, Plain, Double]: [Atlas, Roads, Double] — Atlas has
///   no Subtitle, and Double matches both leaves but is kept once;
/// - `Filter(Subtitle contains "")` over [Atlas, Plain]: [Plain] — the empty
///   literal needs a present value;
/// - `SeedHolons` with `SeedPredicate(Key contains "seedtarget")`: [Atlas];
/// - `Expand(AuthoredBy)` with `ExpansionPredicate(Name contains "ADA")` over
///   [Strasse, Atlas, Roads]: [Ada, Ada];
/// - `Filter -> Distinct` and `Distinct -> Filter`;
/// - `Expand(AuthorOf) -> Filter(AnyOf) -> OrderBy(Title) -> Limit(1)` from
///   [Ada, Bob]: [Roads];
/// - failures: GreaterThan on the Boolean IsPublished
///   (`UnsupportedOperator`), the same after a completed `Distinct`, the same
///   in a nested `AnyOf`, a selector the
///   declared type lacks on empty input, a SeedPredicate selecting Title on
///   the root type, an integer-typed literal (`IncompatibleLiteralType`), and a
///   misplaced SeedPredicate (`InvalidParameter`).
pub fn query_f1_filter_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "query_f1_filter",
        "QRY-F1: Contains comparison predicates filter through Filter, SeedPredicate and \
         ExpansionPredicate, with flat AnyOf, through the direct and QueryDance routes",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    // --- Commit the Book/Person data ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for the QRY-F1 Book/Person data".to_string()),
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
            BOOK_STRASSE_KEY => vec![ada.clone(), bob.clone()],
            BOOK_ATLAS_KEY => vec![bob.clone()],
            BOOK_ROADS_KEY => vec![ada.clone()],
            BOOK_PLAIN_KEY => vec![cy.clone()],
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
        Some("Commit the QRY-F1 Book/Person data".to_string()),
    )?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F1 transient query execution".to_string()),
    )?;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let strasse = saved(&mut test_case, &mut fixture_holons, BOOK_STRASSE_KEY)?;
    let atlas = saved(&mut test_case, &mut fixture_holons, BOOK_ATLAS_KEY)?;
    let roads = saved(&mut test_case, &mut fixture_holons, BOOK_ROADS_KEY)?;
    let plain = saved(&mut test_case, &mut fixture_holons, BOOK_PLAIN_KEY)?;
    let double = saved(&mut test_case, &mut fixture_holons, BOOK_DOUBLE_KEY)?;
    let ada = saved(&mut test_case, &mut fixture_holons, PERSON_ADA_KEY)?;
    let bob = saved(&mut test_case, &mut fixture_holons, PERSON_BOB_KEY)?;

    let types = PredicateTypes::look_up(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let tight_string = saved(&mut test_case, &mut fixture_holons, "ShortText.MapStringValueType")?;
    let inherited_tight_string =
        saved(&mut test_case, &mut fixture_holons, "ShortTextChild.ShortText")?;
    let mut authoring = query_authoring(&mut test_case, &fixture_context, &mut fixture_holons)?;
    let mut author =
        PredicateAuthoring { authoring: &mut authoring, types: &types, prefix: "QryF1" };

    // Filter leaves and flat AnyOf.
    let strasse_title = author.contains("Strasse", "Title", "STRASSE")?;
    let title_filter = author.filter("Strasse", strasse_title)?;
    let title_filter = author.query("Strasse", title_filter)?;

    let atlas_title = author.contains("AtlasTitle", "Title", "atlas")?;
    let atlas_subtitle = author.contains("AtlasSubtitle", "Subtitle", "atlas")?;
    let atlas_any = author.any_of("Atlas", vec![atlas_title, atlas_subtitle])?;
    let atlas_filter = author.filter("Atlas", atlas_any)?;
    let atlas_filter = author.query("Atlas", atlas_filter)?;

    let any_subtitle = author.contains("AnySubtitle", "Subtitle", "")?;
    let subtitle_filter = author.filter("AnySubtitle", any_subtitle)?;
    let subtitle_filter = author.query("AnySubtitle", subtitle_filter)?;

    // Attachments.
    let seed_key = author.contains("SeedKey", "Key", "seedtarget")?;
    let seed = author.seed("SeedKey", Some(seed_key))?;
    let seed_query = author.query("SeedKey", seed)?;

    let ada_name = author.contains("AdaName", "Name", "ADA")?;
    let expand_to_ada = author.authoring.expand("Expand.QryF1Ada", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand_to_ada = author.relate(
        expand_to_ada,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![ada_name],
    )?;
    let expansion_query = author.query("AdaExpansion", expand_to_ada)?;

    // Composition with Distinct, in both orders.
    let atlas_again = author.contains("AtlasAgain", "Title", "atlas")?;
    let filter_first = author.filter("BeforeDistinct", atlas_again.clone())?;
    let distinct_after = author.authoring.distinct("Distinct.QryF1AfterFilter")?;
    let filter_first = author.authoring.next(filter_first, distinct_after)?;
    let filter_then_distinct = author.query("FilterThenDistinct", filter_first)?;
    let filter_after = author.filter("AfterDistinct", atlas_again)?;
    let distinct_first = author.authoring.distinct("Distinct.QryF1BeforeFilter")?;
    let distinct_first = author.authoring.next(distinct_first, filter_after)?;
    let distinct_then_filter = author.query("DistinctThenFilter", distinct_first)?;

    // Expand(AuthorOf) -> Filter(AnyOf) -> OrderBy(Title) -> Limit(1). The sort
    // makes the inverse expansion's storage order irrelevant.
    let mixed_limit = author.authoring.limit("Limit.QryF1Mixed", Some(1))?;
    let mixed_spec = author.authoring.spec("Spec.QryF1Mixed", selector("Title"), None, None)?;
    let mixed_order = author.authoring.order_by("OrderBy.QryF1Mixed", vec![mixed_spec])?;
    let mixed_order = author.authoring.next(mixed_order, mixed_limit)?;
    let mixed_atlas = author.contains("MixedAtlas", "Title", "atlas")?;
    let mixed_roads = author.contains("MixedRoads", "Title", "roads")?;
    let mixed_any = author.any_of("Mixed", vec![mixed_atlas, mixed_roads])?;
    let mixed_filter = author.filter("Mixed", mixed_any)?;
    let mixed_filter = author.authoring.next(mixed_filter, mixed_order)?;
    let mixed_expand = author.authoring.expand("Expand.QryF1Mixed", PERSON_TO_BOOK_RELATIONSHIP)?;
    let mixed_expand = author.authoring.next(mixed_expand, mixed_filter)?;
    let mixed_query = author.query("Mixed", mixed_expand)?;

    // Failures. Booleans afford Equals but no ordering, so an ordered
    // comparison on IsPublished is an unsupported operator capability.
    let map_boolean = types.map_boolean.clone();
    let greater_than = types.greater_than.clone();
    let unordered = author.compare(
        "Unordered",
        "IsPublished",
        &greater_than,
        &map_boolean,
        true.to_base_value(),
    )?;
    let unordered_filter = author.filter("Unordered", unordered.clone())?;
    let unordered_query = author.query("Unordered", unordered_filter)?;
    let late_unordered = author.filter("LateUnordered", unordered)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF1LateUnordered")?;
    let late_distinct = author.authoring.next(late_distinct, late_unordered)?;
    let late_unordered_query = author.query("LateUnordered", late_distinct)?;

    // Nesting is supported since QRY-F3; an unsupported operator still fails at depth.
    let nested_leaf = author.compare(
        "NestedLeaf",
        "IsPublished",
        &greater_than,
        &map_boolean,
        true.to_base_value(),
    )?;
    let inner = author.any_of("Inner", vec![nested_leaf])?;
    let outer_leaf = author.contains("OuterLeaf", "Title", "y")?;
    let outer = author.any_of("Outer", vec![outer_leaf, inner])?;
    let nested_filter = author.filter("Nested", outer)?;
    let nested_query = author.query("Nested", nested_filter)?;

    let person_name_on_books = author.contains("NameOnBooks", "Name", "x")?;
    let undeclared_filter = author.filter("NameOnBooks", person_name_on_books)?;
    let undeclared_query = author.query("NameOnBooks", undeclared_filter)?;

    let seed_title = author.contains("SeedTitle", "Title", "atlas")?;
    let title_seed = author.seed("SeedTitle", Some(seed_title))?;
    let title_seed_query = author.query("SeedTitle", title_seed)?;

    let integer_operand =
        author.literal("Integer", &types.map_integer, MapInteger(7).to_base_value())?;
    let integer_title =
        author.comparison("Integer", "Title", &types.contains, vec![integer_operand])?;
    let integer_filter = author.filter("Integer", integer_title)?;
    let integer_query = author.query("Integer", integer_filter)?;

    let misplaced_leaf = author.contains("Misplaced", "Title", "atlas")?;
    let misplaced = author.filter("Misplaced", misplaced_leaf.clone())?;
    let misplaced =
        author.relate(misplaced, QueryRelationshipTypeName::SeedPredicate, vec![misplaced_leaf])?;
    let misplaced_query = author.query("Misplaced", misplaced)?;

    // Scalar literal contracts are checked before candidates are read, including empty inputs.
    let tight_boundary = author.compare(
        "TightBoundary",
        "Title",
        &types.contains,
        &tight_string,
        "atlas".to_base_value(),
    )?;
    let tight_boundary = author.filter("TightBoundary", tight_boundary)?;
    let tight_boundary = author.query("TightBoundary", tight_boundary)?;
    let invalid_filter = author.compare(
        "TightFilter",
        "Title",
        &types.contains,
        &tight_string,
        "abcdef".to_base_value(),
    )?;
    let invalid_filter = author.filter("TightFilter", invalid_filter)?;
    let invalid_filter = author.query("TightFilter", invalid_filter)?;
    let invalid_seed = author.compare(
        "TightSeed",
        "Key",
        &types.contains,
        &tight_string,
        "abcdef".to_base_value(),
    )?;
    let invalid_seed = author.seed("TightSeed", Some(invalid_seed))?;
    let invalid_seed = author.query("TightSeed", invalid_seed)?;
    let invalid_expand = author.compare(
        "TightExpand",
        "Name",
        &types.contains,
        &tight_string,
        "abcdef".to_base_value(),
    )?;
    let expand = author.authoring.expand("Expand.TightLiteral", BOOK_TO_PERSON_RELATIONSHIP)?;
    let expand = author.relate(
        expand,
        QueryRelationshipTypeName::ExpansionPredicate,
        vec![invalid_expand],
    )?;
    let invalid_expand = author.query("TightExpand", expand)?;
    let invalid_any_leaf = author.compare(
        "TightAnyOf",
        "Title",
        &types.contains,
        &tight_string,
        "abcdef".to_base_value(),
    )?;
    let valid_any_leaf = author.contains("ValidAnyOf", "Title", "atlas")?;
    let invalid_any = author.any_of("TightAnyOf", vec![valid_any_leaf, invalid_any_leaf])?;
    let invalid_any = author.filter("TightAnyOf", invalid_any)?;
    let invalid_any = author.query("TightAnyOf", invalid_any)?;

    // Equals requires ShortText itself; D inherits its bound from this required type R.
    let inherited_invalid = author.compare(
        "InheritedTight",
        "ShortText",
        &types.equals,
        &inherited_tight_string,
        "abcdef".to_base_value(),
    )?;
    let inherited_invalid = author.filter("InheritedTight", inherited_invalid)?;
    let inherited_invalid = author.query("InheritedTight", inherited_invalid)?;

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;
        test_case.add_execute_query_step(
            tight_boundary.clone(),
            books(&[&atlas, &plain]),
            route,
            members(&[&atlas]),
            Some("A tight declared literal accepts its grapheme boundary".into()),
        )?;
        for (query, input) in [
            (invalid_filter.clone(), books(&[&atlas])),
            (invalid_filter.clone(), books(&[])),
            (invalid_seed.clone(), QueryInputSpec::None),
            (invalid_expand.clone(), books(&[&strasse])),
            (invalid_expand.clone(), books(&[])),
            (invalid_any.clone(), books(&[])),
            (inherited_invalid.clone(), books(&[])),
        ] {
            test_case.add_execute_query_step(
                query,
                input,
                route,
                error(HolonErrorKind::ValueConstraintViolation),
                Some(format!(
                    "Configured literal constraints fail before candidate reads via {route:?}"
                )),
            )?;
        }

        test_case.add_execute_query_step(
            title_filter.clone(),
            books(&[&strasse, &atlas, &roads, &strasse]),
            route,
            members(&[&strasse, &strasse]),
            Some(format!("Filter(Title contains STRASSE) folds Straße via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            atlas_filter.clone(),
            books(&[&strasse, &atlas, &roads, &plain, &double]),
            route,
            members(&[&atlas, &roads, &double]),
            Some(format!(
                "Filter(AnyOf(Title, Subtitle)) keeps each occurrence once via {route:?}"
            )),
        )?;
        test_case.add_execute_query_step(
            subtitle_filter.clone(),
            books(&[&atlas, &plain]),
            route,
            members(&[&plain]),
            Some(format!("An empty substring needs a present optional value via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            seed_query.clone(),
            QueryInputSpec::None,
            route,
            members(&[&atlas]),
            Some(format!("SeedHolons filtered by SeedPredicate(Key) via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            expansion_query.clone(),
            books(&[&strasse, &atlas, &roads]),
            route,
            members(&[&ada, &ada]),
            Some(format!("Expand filtered by ExpansionPredicate(Name) via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            filter_then_distinct.clone(),
            books(&[&atlas, &double, &roads, &atlas]),
            route,
            members(&[&atlas, &double]),
            Some(format!("Filter -> Distinct via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            distinct_then_filter.clone(),
            books(&[&atlas, &double, &roads, &atlas]),
            route,
            members(&[&atlas, &double]),
            Some(format!("Distinct -> Filter via {route:?}")),
        )?;
        test_case.add_execute_query_step(
            mixed_query.clone(),
            QueryInputSpec::collection("Person", vec![ada.clone(), bob.clone()]),
            route,
            members(&[&roads]),
            Some(format!("Expand -> Filter -> OrderBy -> Limit via {route:?}")),
        )?;

        for (query, input, kind, what) in [
            (
                &unordered_query,
                books(&[&roads]),
                HolonErrorKind::UnsupportedOperator,
                "an ordered predicate on a Boolean",
            ),
            (
                &late_unordered_query,
                books(&[&roads]),
                HolonErrorKind::UnsupportedOperator,
                "an ordered predicate on a Boolean after a completed Distinct",
            ),
            (
                &nested_query,
                books(&[&roads]),
                HolonErrorKind::UnsupportedOperator,
                "an ordered predicate on a Boolean in a nested AnyOf",
            ),
            (
                &undeclared_query,
                books(&[]),
                HolonErrorKind::DescriptorDeclarationNotFound,
                "a selector Book lacks, on empty input",
            ),
            (
                &title_seed_query,
                QueryInputSpec::None,
                HolonErrorKind::DescriptorDeclarationNotFound,
                "a SeedPredicate selecting Title on the root type",
            ),
            (
                &integer_query,
                books(&[]),
                HolonErrorKind::IncompatibleLiteralType,
                "an integer-typed Contains literal",
            ),
            (
                &misplaced_query,
                books(&[&atlas]),
                HolonErrorKind::InvalidParameter,
                "a SeedPredicate attached to a Filter",
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

/// Looks up the Query descriptors [`TransientAuthoring`] describes holons with.
pub(super) fn query_authoring<'a>(
    test_case: &'a mut DancesTestCase,
    fixture_context: &'a std::sync::Arc<TransactionContext>,
    fixture_holons: &'a mut FixtureHolons,
) -> Result<TransientAuthoring<'a>, HolonError> {
    let mut lookup = |name: QueryHolonTypeName| {
        lookup_by_key(
            test_case,
            fixture_context,
            fixture_holons,
            &holon_type_descriptor_key(&name.as_holon_name()).0,
        )
    };
    let query_type = lookup(QueryHolonTypeName::Query)?;
    let expand_type = lookup(QueryHolonTypeName::Expand)?;
    let skip_type = lookup(QueryHolonTypeName::Skip)?;
    let limit_type = lookup(QueryHolonTypeName::Limit)?;
    let order_by_type = lookup(QueryHolonTypeName::OrderBy)?;
    let spec_type = lookup(QueryHolonTypeName::OrderBySpec)?;
    let distinct_type = lookup(QueryHolonTypeName::Distinct)?;
    Ok(TransientAuthoring {
        test_case,
        fixture_context,
        fixture_holons,
        query_type,
        expand_type,
        skip_type,
        limit_type,
        order_by_type,
        spec_type,
        distinct_type,
    })
}

/// Canonical descriptors the predicate builders describe holons with.
pub(super) struct PredicateTypes {
    pub(super) filter: TestReference,
    pub(super) comparison: TestReference,
    pub(super) any_of: TestReference,
    pub(super) all_of: TestReference,
    pub(super) not: TestReference,
    pub(super) literal: TestReference,
    pub(super) seed: TestReference,
    pub(super) contains: TestReference,
    pub(super) equals: TestReference,
    pub(super) less_than: TestReference,
    pub(super) greater_than: TestReference,
    pub(super) less_than_or_equal: TestReference,
    pub(super) greater_than_or_equal: TestReference,
    pub(super) map_string: TestReference,
    pub(super) map_integer: TestReference,
    pub(super) map_boolean: TestReference,
}

impl PredicateTypes {
    pub(super) fn look_up(
        test_case: &mut DancesTestCase,
        fixture_context: &std::sync::Arc<TransactionContext>,
        fixture_holons: &mut FixtureHolons,
    ) -> Result<Self, HolonError> {
        let mut lookup = |key: &str| lookup_by_key(test_case, fixture_context, fixture_holons, key);
        let query = |name: QueryHolonTypeName| holon_type_descriptor_key(&name.as_holon_name()).0;
        let operator =
            |name: CoreOperatorTypeName| operator_type_descriptor_key(&name.as_operator_name().0).0;
        let value = |name: CoreValueTypeName, family: CoreValueTypeName| {
            value_type_descriptor_key(&name.as_value_name(), &family).0
        };
        Ok(Self {
            filter: lookup(&query(QueryHolonTypeName::Filter))?,
            comparison: lookup(&query(QueryHolonTypeName::ComparisonPredicate))?,
            any_of: lookup(&query(QueryHolonTypeName::AnyOf))?,
            all_of: lookup(&query(QueryHolonTypeName::AllOf))?,
            not: lookup(&query(QueryHolonTypeName::Not))?,
            literal: lookup(&query(QueryHolonTypeName::LiteralOperand))?,
            seed: lookup(&query(QueryHolonTypeName::SeedHolons))?,
            contains: lookup(&operator(CoreOperatorTypeName::ContainsOperator))?,
            equals: lookup(&operator(CoreOperatorTypeName::EqualsOperator))?,
            less_than: lookup(&operator(CoreOperatorTypeName::LessThanOperator))?,
            greater_than: lookup(&operator(CoreOperatorTypeName::GreaterThanOperator))?,
            less_than_or_equal: lookup(&operator(CoreOperatorTypeName::LessThanOrEqualOperator))?,
            greater_than_or_equal: lookup(&operator(
                CoreOperatorTypeName::GreaterThanOrEqualOperator,
            ))?,
            map_string: lookup(&value(
                CoreValueTypeName::MapStringValueType,
                CoreValueTypeName::StringValueType,
            ))?,
            map_integer: lookup(&value(
                CoreValueTypeName::MapIntegerValueType,
                CoreValueTypeName::IntegerValueType,
            ))?,
            map_boolean: lookup(&value(
                CoreValueTypeName::MapBooleanValueType,
                CoreValueTypeName::BooleanValueType,
            ))?,
        })
    }
}

/// Authors transient predicate-bearing definitions on top of [`TransientAuthoring`].
/// Every definition key carries `prefix`, so fixtures sharing a transaction
/// history never collide.
pub(super) struct PredicateAuthoring<'a, 'b> {
    pub(super) authoring: &'a mut TransientAuthoring<'b>,
    pub(super) types: &'a PredicateTypes,
    pub(super) prefix: &'static str,
}

impl PredicateAuthoring<'_, '_> {
    pub(super) fn relate(
        &mut self,
        source: TestReference,
        relationship: QueryRelationshipTypeName,
        targets: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let name = relationship.as_relationship_name().to_string();
        self.authoring.relate_all(source, &name, targets, format!("Relate --{name}--> targets"))
    }

    pub(super) fn query(
        &mut self,
        label: &str,
        root: TestReference,
    ) -> Result<TestReference, HolonError> {
        self.authoring.query(&format!("Query.{}{label}", self.prefix), root)
    }

    pub(super) fn literal(
        &mut self,
        label: &str,
        value_type: &TestReference,
        value: BaseValue,
    ) -> Result<TestReference, HolonError> {
        let mut properties = PropertyMap::new();
        properties.insert(QueryPropertyTypeName::OperandValue.as_property_name(), value);
        let literal = self.authoring.described(
            &format!("LiteralOperand.{}{label}", self.prefix),
            properties,
            self.types.literal.clone(),
        )?;
        self.authoring.relate(
            literal,
            &CoreRelationshipTypeName::ValueType.as_relationship_name().to_string(),
            value_type.clone(),
            format!("LiteralOperand.{}{label} --ValueType--> its declared type", self.prefix),
        )
    }

    pub(super) fn comparison(
        &mut self,
        label: &str,
        property: &str,
        operator: &TestReference,
        operands: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let properties = instance_properties(
            &QueryPropertyTypeName::PropertyName.as_property_name().to_string(),
            property,
        );
        let predicate = self.authoring.described(
            &format!("ComparisonPredicate.{}{label}", self.prefix),
            properties,
            self.types.comparison.clone(),
        )?;
        let predicate =
            self.relate(predicate, QueryRelationshipTypeName::Operator, vec![operator.clone()])?;
        self.relate(predicate, QueryRelationshipTypeName::Operands, operands)
    }

    /// A Contains leaf whose substring is declared as `MapStringValueType`.
    pub(super) fn contains(
        &mut self,
        label: &str,
        property: &str,
        substring: &str,
    ) -> Result<TestReference, HolonError> {
        let map_string = self.types.map_string.clone();
        let operand =
            self.literal(label, &map_string, MapString(substring.to_string()).to_base_value())?;
        let contains = self.types.contains.clone();
        self.comparison(label, property, &contains, vec![operand])
    }

    /// An Equals leaf whose literal is declared as `value_type`, which must be
    /// the selected property's ValueType or a subtype of it.
    pub(super) fn equals(
        &mut self,
        label: &str,
        property: &str,
        value_type: &TestReference,
        value: BaseValue,
    ) -> Result<TestReference, HolonError> {
        let operand = self.literal(label, value_type, value)?;
        let equals = self.types.equals.clone();
        self.comparison(label, property, &equals, vec![operand])
    }

    /// A single-literal comparison through `operator`, its literal declared as
    /// `value_type`.
    pub(super) fn compare(
        &mut self,
        label: &str,
        property: &str,
        operator: &TestReference,
        value_type: &TestReference,
        value: BaseValue,
    ) -> Result<TestReference, HolonError> {
        let operand = self.literal(label, value_type, value)?;
        self.comparison(label, property, operator, vec![operand])
    }

    pub(super) fn any_of(
        &mut self,
        label: &str,
        predicates: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let any_of = self.authoring.described(
            &format!("AnyOf.{}{label}", self.prefix),
            PropertyMap::new(),
            self.types.any_of.clone(),
        )?;
        self.relate(any_of, QueryRelationshipTypeName::Predicates, predicates)
    }

    pub(super) fn all_of(
        &mut self,
        label: &str,
        predicates: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let all_of = self.authoring.described(
            &format!("AllOf.{}{label}", self.prefix),
            PropertyMap::new(),
            self.types.all_of.clone(),
        )?;
        self.relate(all_of, QueryRelationshipTypeName::Predicates, predicates)
    }

    /// A `Not` over `predicates`; any count can be authored, so malformed
    /// negations are expressible.
    pub(super) fn not(
        &mut self,
        label: &str,
        predicates: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let not = self.authoring.described(
            &format!("Not.{}{label}", self.prefix),
            PropertyMap::new(),
            self.types.not.clone(),
        )?;
        self.relate(not, QueryRelationshipTypeName::Predicate, predicates)
    }

    pub(super) fn filter(
        &mut self,
        label: &str,
        predicate: TestReference,
    ) -> Result<TestReference, HolonError> {
        let filter = self.authoring.described(
            &format!("Filter.{}{label}", self.prefix),
            PropertyMap::new(),
            self.types.filter.clone(),
        )?;
        self.relate(filter, QueryRelationshipTypeName::Predicate, vec![predicate])
    }

    pub(super) fn seed(
        &mut self,
        label: &str,
        predicate: Option<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let seed = self.authoring.described(
            &format!("SeedHolons.{}{label}", self.prefix),
            PropertyMap::new(),
            self.types.seed.clone(),
        )?;
        self.relate(seed, QueryRelationshipTypeName::SeedPredicate, predicate.into_iter().collect())
    }
}
