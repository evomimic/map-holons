use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, FixtureHolons, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use rstest::*;
// use tracing::debug;

use super::setup_undescribed_book_people_publisher_steps_with_context;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_KEY, BOOK_PERSON_INVERSE_METRICS, CORE_SCHEMA_METRICS,
};

// TODO: add/remove relationships

/// Expected DB count once core + Book/Person schemas are loaded: fixture-saved
/// holons (incl. the LocalHolonSpace baseline) plus the loader-committed schema holons.
fn schema_backed_db_count(fixture_holons: &FixtureHolons) -> MapInteger {
    MapInteger(
        fixture_holons.count_saved().0
            + CORE_SCHEMA_METRICS.committed
            + BOOK_PERSON_INVERSE_METRICS.committed,
    )
}

/// Fixture for creating Simple NEWVERSION Testcase
///
/// Schema-backed setup (issue #442): `add_stage_new_version_step` auto-stages a
/// `Predecessor` relationship, and strict commit Pass 2 only persists
/// relationships the source holon's effective schema surface declares. The Book
/// is therefore described by the loaded `Book.HolonType` (whose Extends chain
/// reaches `MetaHolonType`, where `Predecessor` is declared); the Persons and
/// Publisher stage no relationships and stay undescribed.
#[fixture]
pub fn stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings } =
        TestCaseInit::new("Simple StageNewVersion Testcase", "Tests stage_new_version dance");
    let mut version_count = MapInteger(1);

    // Load the schemas that declare Book.HolonType and (via MetaHolonType) Predecessor.
    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for Book/People/Publisher setup".to_string()),
    )?;

    // Use helper function to set up a book holon, 2 persons, and a publisher.
    setup_undescribed_book_people_publisher_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    let book_staged_token = fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Book' label").clone();

    // Describe the Book by the loaded Book.HolonType so its Predecessor edges resolve.
    let book_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let book_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        book_type_stub,
        MapString(BOOK_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;
    let book_staged_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_staged_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![book_type_token],
        None,
        Some("Describe Book by Book.HolonType".to_string()),
    )?;

    //  ENSURE DATABASE COUNT -- Initial //
    test_case.add_ensure_database_count_step(
        schema_backed_db_count(&fixture_holons),
        Some("Ensuring DB holds only schema holons before first commit".to_string()),
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- after setup_book_authors".to_string()),
    )?;

    //  ENSURE DATABASE COUNT -- After Commit //
    test_case.add_ensure_database_count_step(schema_backed_db_count(&fixture_holons), None)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Begin a fresh transaction before resuming mutating work from the saved book.
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before staging first new version".to_string()),
    )?;

    // Get book source
    let book_key = MapString(BOOK_KEY.to_string());

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    version_count.0 += 1;

    let staged_clone = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        None,
        version_count.clone(),
        None,
        Some("Stage New Version -- first clone from book into fresh transaction".to_string()),
    )?;

    // Add properties
    let mut expected_clone_properties = PropertyMap::new();
    expected_clone_properties.insert("Key".to_property_name(), book_key.clone().to_base_value());
    expected_clone_properties.insert(
        "Description".to_property_name(),
        "This is a different description".to_base_value(),
    );
    expected_clone_properties.insert("Title".to_property_name(), "Changed".to_base_value());

    test_case.add_with_properties_step(
        &mut fixture_holons,
        staged_clone,
        expected_clone_properties.clone(),
        None,
        Some("With Properties -- first version cloned from book.".to_string()),
    )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- after staging new first version".to_string()),
    )?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(schema_backed_db_count(&fixture_holons), None)?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Begin fresh transaction so versions 2/3 stage into a clean nursery
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction after second commit".to_string()),
    )?;

    // VERSION 2 //
    // Stage a second version from the same original holon in order to verify that:
    // a. get_staged_holon_by_base_key returns an error (>1 staged holon with that key)
    // b. get_staged_holons_by_base_key correctly returns BOTH staged holons

    version_count.0 += 1;

    let _version_2_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        None,
        version_count.clone(),
        None,
        Some(
            "Stage New Version --- second version; first in this transaction, no duplicate"
                .to_string(),
        ),
    )?;

    // Third version in same transaction — now 2 staged holons share the base key
    let staged_in_this_tx = MapInteger(2);

    let _version_3_token = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        book_staged_token,
        None,
        staged_in_this_tx,
        Some(HolonErrorKind::DuplicateError),
        Some("Stage New Version --- third version, expecting DuplicateError from get_staged_holon_by_base_key".to_string()),
    )?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
