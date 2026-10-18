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
/// literal operands) is a transient holon, and the executor asserts that
/// evaluation leaves that whole graph unchanged.
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
/// - failures: an Equals predicate (`NotImplemented`), the same after a
///   completed `Distinct`, nested `AnyOf` (`NotImplemented`), a selector the
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
    test_case.add_match_saved_content_step()?;

    // --- Author transient definitions and execute them in a fresh transaction ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for QRY-F1 transient query execution".to_string()),
    )?;
    let lookup = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key: &str| {
        lookup_by_key(test_case, &fixture_context, fixture_holons, key)
    };
    let query_key = |name: QueryHolonTypeName| holon_type_descriptor_key(&name.as_holon_name()).0;
    let operator_key =
        |name: CoreOperatorTypeName| operator_type_descriptor_key(&name.as_operator_name().0).0;
    let saved = |test_case: &mut DancesTestCase, fixture_holons: &mut FixtureHolons, key| {
        lookup(test_case, fixture_holons, key)
    };
    let strasse = saved(&mut test_case, &mut fixture_holons, BOOK_STRASSE_KEY)?;
    let atlas = saved(&mut test_case, &mut fixture_holons, BOOK_ATLAS_KEY)?;
    let roads = saved(&mut test_case, &mut fixture_holons, BOOK_ROADS_KEY)?;
    let plain = saved(&mut test_case, &mut fixture_holons, BOOK_PLAIN_KEY)?;
    let double = saved(&mut test_case, &mut fixture_holons, BOOK_DOUBLE_KEY)?;
    let ada = saved(&mut test_case, &mut fixture_holons, PERSON_ADA_KEY)?;
    let bob = saved(&mut test_case, &mut fixture_holons, PERSON_BOB_KEY)?;

    let types = PredicateTypes {
        filter: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Filter),
        )?,
        comparison: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::ComparisonPredicate),
        )?,
        any_of: lookup(&mut test_case, &mut fixture_holons, &query_key(QueryHolonTypeName::AnyOf))?,
        literal: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::LiteralOperand),
        )?,
        seed: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::SeedHolons),
        )?,
        contains: lookup(
            &mut test_case,
            &mut fixture_holons,
            &operator_key(CoreOperatorTypeName::ContainsOperator),
        )?,
        equals: lookup(
            &mut test_case,
            &mut fixture_holons,
            &operator_key(CoreOperatorTypeName::EqualsOperator),
        )?,
        map_string: lookup(
            &mut test_case,
            &mut fixture_holons,
            &value_type_descriptor_key(
                &CoreValueTypeName::MapStringValueType.as_value_name(),
                &CoreValueTypeName::StringValueType,
            )
            .0,
        )?,
        map_integer: lookup(
            &mut test_case,
            &mut fixture_holons,
            &value_type_descriptor_key(
                &CoreValueTypeName::MapIntegerValueType.as_value_name(),
                &CoreValueTypeName::IntegerValueType,
            )
            .0,
        )?,
    };
    let mut authoring = TransientAuthoring {
        query_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Query),
        )?,
        expand_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Expand),
        )?,
        skip_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Skip),
        )?,
        limit_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Limit),
        )?,
        order_by_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::OrderBy),
        )?,
        spec_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::OrderBySpec),
        )?,
        distinct_type: lookup(
            &mut test_case,
            &mut fixture_holons,
            &query_key(QueryHolonTypeName::Distinct),
        )?,
        test_case: &mut test_case,
        fixture_context: &fixture_context,
        fixture_holons: &mut fixture_holons,
    };
    let mut author = PredicateAuthoring { authoring: &mut authoring, types: &types };

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

    // Failures.
    let equals_operand = author.literal(
        "Equals",
        &types.map_string,
        MapString("Roads".to_string()).to_base_value(),
    )?;
    let equals_title = author.comparison("Equals", "Title", &types.equals, vec![equals_operand])?;
    let equals_filter = author.filter("Equals", equals_title.clone())?;
    let equals_query = author.query("Equals", equals_filter)?;
    let late_equals = author.filter("LateEquals", equals_title)?;
    let late_distinct = author.authoring.distinct("Distinct.QryF1LateEquals")?;
    let late_distinct = author.authoring.next(late_distinct, late_equals)?;
    let late_equals_query = author.query("LateEquals", late_distinct)?;

    let nested_leaf = author.contains("NestedLeaf", "Title", "x")?;
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

    for route in [QueryRoute::Direct, QueryRoute::QueryDance] {
        let books = |members: &[&TestReference]| {
            QueryInputSpec::collection("Book", members.iter().map(|book| (*book).clone()).collect())
        };
        let members = |members: &[&TestReference]| {
            QueryExpectation::Members(members.iter().map(|member| (*member).clone()).collect())
        };
        let error = QueryExpectation::Error;

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
                &equals_query,
                books(&[&roads]),
                HolonErrorKind::NotImplemented,
                "an Equals predicate",
            ),
            (
                &late_equals_query,
                books(&[&roads]),
                HolonErrorKind::NotImplemented,
                "an Equals predicate after a completed Distinct",
            ),
            (&nested_query, books(&[&roads]), HolonErrorKind::NotImplemented, "a nested AnyOf"),
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

/// Canonical descriptors the predicate builders describe holons with.
struct PredicateTypes {
    filter: TestReference,
    comparison: TestReference,
    any_of: TestReference,
    literal: TestReference,
    seed: TestReference,
    contains: TestReference,
    equals: TestReference,
    map_string: TestReference,
    map_integer: TestReference,
}

/// Authors transient predicate-bearing definitions on top of [`TransientAuthoring`].
struct PredicateAuthoring<'a, 'b> {
    authoring: &'a mut TransientAuthoring<'b>,
    types: &'a PredicateTypes,
}

impl PredicateAuthoring<'_, '_> {
    fn relate(
        &mut self,
        source: TestReference,
        relationship: QueryRelationshipTypeName,
        targets: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let name = relationship.as_relationship_name().to_string();
        self.authoring.relate_all(source, &name, targets, format!("Relate --{name}--> targets"))
    }

    fn query(&mut self, label: &str, root: TestReference) -> Result<TestReference, HolonError> {
        self.authoring.query(&format!("Query.QryF1{label}"), root)
    }

    fn literal(
        &mut self,
        label: &str,
        value_type: &TestReference,
        value: BaseValue,
    ) -> Result<TestReference, HolonError> {
        let mut properties = PropertyMap::new();
        properties.insert(QueryPropertyTypeName::OperandValue.as_property_name(), value);
        let literal = self.authoring.described(
            &format!("LiteralOperand.QryF1{label}"),
            properties,
            self.types.literal.clone(),
        )?;
        self.authoring.relate(
            literal,
            &CoreRelationshipTypeName::ValueType.as_relationship_name().to_string(),
            value_type.clone(),
            format!("LiteralOperand.QryF1{label} --ValueType--> its declared type"),
        )
    }

    fn comparison(
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
            &format!("ComparisonPredicate.QryF1{label}"),
            properties,
            self.types.comparison.clone(),
        )?;
        let predicate =
            self.relate(predicate, QueryRelationshipTypeName::Operator, vec![operator.clone()])?;
        self.relate(predicate, QueryRelationshipTypeName::Operands, operands)
    }

    /// A Contains leaf whose substring is declared as `MapStringValueType`.
    fn contains(
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

    fn any_of(
        &mut self,
        label: &str,
        predicates: Vec<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let any_of = self.authoring.described(
            &format!("AnyOf.QryF1{label}"),
            PropertyMap::new(),
            self.types.any_of.clone(),
        )?;
        self.relate(any_of, QueryRelationshipTypeName::Predicates, predicates)
    }

    fn filter(
        &mut self,
        label: &str,
        predicate: TestReference,
    ) -> Result<TestReference, HolonError> {
        let filter = self.authoring.described(
            &format!("Filter.QryF1{label}"),
            PropertyMap::new(),
            self.types.filter.clone(),
        )?;
        self.relate(filter, QueryRelationshipTypeName::Predicate, vec![predicate])
    }

    fn seed(
        &mut self,
        label: &str,
        predicate: Option<TestReference>,
    ) -> Result<TestReference, HolonError> {
        let seed = self.authoring.described(
            &format!("SeedHolons.QryF1{label}"),
            PropertyMap::new(),
            self.types.seed.clone(),
        )?;
        self.relate(seed, QueryRelationshipTypeName::SeedPredicate, predicate.into_iter().collect())
    }
}
