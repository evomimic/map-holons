//! Shared persisted subjects and saved definitions for the query boundary contract.
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{BOOK_DESCRIPTOR_KEY, PERSON_DESCRIPTOR_KEY};
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};

use super::query_qry2_seed_expand_fixture::{instance_properties, lookup_by_key, stage_described};

pub const BOOK_KEYS: [&str; 5] = [
    "QueryBoundary.Book.Atlas",
    "QueryBoundary.Book.Roads",
    "QueryBoundary.Book.Bare",
    "QueryBoundary.Book.Drafted",
    "QueryBoundary.Book.NoAuthors",
];
pub const PERSON_KEYS: [&str; 2] = ["QueryBoundary.Person.Ada", "QueryBoundary.Person.Bob"];
pub const SEED_QUERY: &str = "QueryBoundary.SavedSeed";
pub const DECLARED_QUERY: &str = "QueryBoundary.SavedDeclared";
pub const INVERSE_QUERY: &str = "QueryBoundary.SavedInverse";

/// Real Commit and inverse-link setup is shared by all query storage boundaries.
/// Transient query construction is tested through the production authoring API.
pub fn query_boundary_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } =
        TestCaseInit::new("query_boundary_data", "Shared persisted query subjects and definitions");
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let book_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, BOOK_DESCRIPTOR_KEY)?;
    let person_type = lookup_by_key(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        PERSON_DESCRIPTOR_KEY,
    )?;
    let mut people = Vec::new();
    for (key, name) in PERSON_KEYS.into_iter().zip(["Ada Lovelace", "Bob"]) {
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
    for (index, (title, pages, status, subtitle)) in [
        ("Atlas", Some(300), "Published", Some("final")),
        ("Roads", Some(100), "Draft", Some("final")),
        ("Bare Roads", None, "Published", None),
        ("Drafted Roads", Some(200), "Draft", Some("draft")),
        ("No Authors", Some(500), "Draft", None),
    ]
    .into_iter()
    .enumerate()
    {
        let mut properties = instance_properties("Title", title);
        properties.insert(
            "PublicationStatus".to_property_name(),
            MapEnumValue(MapString::from(status)).to_base_value(),
        );
        if let Some(pages) = pages {
            properties.insert("PageCount".to_property_name(), MapInteger(pages).to_base_value());
        }
        if let Some(subtitle) = subtitle {
            properties
                .insert("Subtitle".to_property_name(), MapString::from(subtitle).to_base_value());
        }
        let book = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            BOOK_KEYS[index],
            properties,
            &book_type,
            "Book",
        )?;
        let authors = match index {
            0 => people.clone(),
            1 | 2 => vec![people[0].clone()],
            3 => vec![people[1].clone()],
            _ => vec![],
        };
        if !authors.is_empty() {
            test_case.add_add_related_holons_step(
                &mut fixture_holons,
                book,
                "AuthoredBy".to_relationship_name(),
                authors,
                None,
                None,
            )?;
        }
    }
    let query_type =
        lookup_by_key(&mut test_case, &fixture_context, &mut fixture_holons, "Query.HolonType")?;
    for (key, kind, relationship) in [
        (SEED_QUERY, "SeedHolons", None),
        (DECLARED_QUERY, "Expand", Some("AuthoredBy")),
        (INVERSE_QUERY, "Expand", Some("AuthorOf")),
    ] {
        let expression_type = lookup_by_key(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            &format!("{kind}.HolonType"),
        )?;
        let mut properties = PropertyMap::new();
        if let Some(relationship) = relationship {
            properties.insert(
                "ExpansionRelationshipName".to_property_name(),
                MapString::from(relationship).to_base_value(),
            );
        }
        let root = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            &format!("{key}.Root"),
            properties,
            &expression_type,
            kind,
        )?;
        let query = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            key,
            instance_properties("QueryName", key),
            &query_type,
            "Query",
        )?;
        test_case.add_add_related_holons_step(
            &mut fixture_holons,
            query,
            "RootExpression".to_relationship_name(),
            vec![root],
            None,
            None,
        )?;
    }
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
