use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_DESCRIPTOR_KEY,
};
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, FixtureHolons, TestCaseInit, TestReference,
};
use std::sync::Arc;

const BOOK_1_KEY: &str = "Book.SmartLinkCache.1";
const BOOK_2_KEY: &str = "Book.SmartLinkCache.2";
const PERSON_1_KEY: &str = "Person.SmartLinkCache.1";
const PERSON_2_KEY: &str = "Person.SmartLinkCache.2";

/// Covers one relationship commit with two repeated SmartLink write buckets:
///
/// - `Book.SmartLinkCache.1 --AuthoredBy--> [Person.SmartLinkCache.1,
///   Person.SmartLinkCache.2]` shares its declared source bucket; and
/// - `Book.SmartLinkCache.1` and `Book.SmartLinkCache.2` both materialize an
///   `AuthorOf` inverse on `Person.SmartLinkCache.1`, sharing that inverse
///   source bucket.
///
/// The fixture uses only Dance Test Language adders. It proves the resulting
/// bidirectional traversal, while instrumentation unit coverage pins the one
/// initial storage expansion per populated bucket.
pub fn smartlink_commit_cache_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "smartlink_commit_cache",
        "Commit repeated declared and inverse Book/Person SmartLink relationships",
    );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for repeated Book/Person relationships".to_string()),
    )?;

    let book_type = lookup_descriptor(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        BOOK_DESCRIPTOR_KEY,
    )?;
    let person_type = lookup_descriptor(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        PERSON_DESCRIPTOR_KEY,
    )?;

    let book_1 = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        BOOK_1_KEY,
        "Title",
        book_type.clone(),
    )?;
    let book_2 = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        BOOK_2_KEY,
        "Title",
        book_type,
    )?;
    let person_1 = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        PERSON_1_KEY,
        "Name",
        person_type.clone(),
    )?;
    let person_2 = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        PERSON_2_KEY,
        "Name",
        person_type,
    )?;

    let book_1 = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_1,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_1.clone(), person_2.clone()],
        None,
        Some("Relate Book.SmartLinkCache.1 --AuthoredBy--> both People".to_string()),
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_2,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_1],
        None,
        Some("Relate Book.SmartLinkCache.2 --AuthoredBy--> Person.SmartLinkCache.1".to_string()),
    )?;

    // Keep the final Book.1 head in the fixture ledger before committing all four instances.
    let _ = book_1;
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit repeated declared and inverse SmartLink relationships".to_string()),
    )?;

    test_case.add_match_saved_content_step()?;
    test_case.add_verify_book_person_smartlink_commit_cache_links_step(None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

fn lookup_descriptor(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    key: &str,
) -> Result<TestReference, HolonError> {
    let key = MapString(key.to_string());
    let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
    test_case.add_lookup_saved_holon_by_key_step(fixture_holons, stub, key, None, None)
}

fn add_described_instance(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    key: &str,
    property_name: &str,
    descriptor: TestReference,
) -> Result<TestReference, HolonError> {
    let key = MapString(key.to_string());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    let mut properties = PropertyMap::new();
    properties.insert(property_name.to_property_name(), key.clone().to_base_value());
    let instance = test_case.add_new_holon_step(
        fixture_holons,
        source,
        properties,
        Some(key.clone()),
        None,
        Some(format!("Create {key}")),
    )?;
    let instance = test_case.add_stage_holon_step(fixture_holons, instance, None, None)?;
    test_case.add_add_related_holons_step(
        fixture_holons,
        instance,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![descriptor],
        None,
        Some(format!("Describe {key}")),
    )
}
