use std::collections::BTreeMap;

use dances::dance_response::ResponseStatusCode;
use holons::reference_layer::HolonReference;

use crate::shared_test::{setup_book_author_steps_with_context, BOOK_KEY};
use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep, DancesTestCase};
use holons_core::core_shared_objects::{HolonError, RelationshipName};

use holons_client::init_client_context;
use rstest::*;
use shared_types_holon::{BaseValue, HolonId, MapInteger, MapString, PropertyMap, PropertyName};

/// Fixture for creating Simple NEWVERSION Testcase
#[fixture]
pub fn simple_stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple StageNewVersion Testcase".to_string(),
        "Tests stage_new_version dance, creates and commits a holon, clones it, changes some properties, adds and removes some relationships, commits it and then compares essential content of existing holon and cloned holon".to_string(),
    );

    // Initialize a client context the fixture can use
    // NOTE: This context will NOT be shared by test executors. The fixture's client context
    // will go away once
    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_client_context().as_ref();
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    let mut holons_to_add: Vec<HolonReference> = Vec::new();

    // Use helper function to set up a book holon, 2 persons, a publisher, and an AUTHORED_BY relationship from
    // the book to both persons.
    let _relationship_name =
        setup_book_author_steps_with_context(&fixture_context, &mut test_case)?;

    expected_count += staging_service.borrow().staged_count();

    // Get and set the various Holons data.
    let book_key = BOOK_KEY.to_string();
    let book_holon_ref = staging_service.get_staged_holon_by_key(fixture_context, &book_key)?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
    let cloned_book_index = 0;
    let cloned_book_key =
        BaseValue::StringValue(MapString("A new version of: Emerging World".to_string()));

    test_case.add_stage_new_version_step(book_key, ResponseStatusCode::OK)?;
    // Don't increment expected count because new version replaces previous version.

    //  CHANGE PROPERTIES  //
    let mut changed_properties = BTreeMap::new();
    changed_properties
        .insert(PropertyName(MapString("title".to_string())), cloned_book_key.clone());
    changed_properties.insert(PropertyName(MapString("key".to_string())), cloned_book_key.clone());
    changed_properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString(
            "example property change for a new version from staged Holon".to_string(),
        )),
    );

    test_case.add_with_properties_step(
        cloned_book_index,
        changed_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // // STAGE:  A 2nd publisher Holon
    // // Another staged test Holon for adding a relationship to // temp workaround until support for passing TestReference to other test steps
    // let mut publisher_2_holon = Holon::new();
    // let publisher_2_index: usize = 1; // assume publisher is at this position in new staged_holons vector
    // let publisher_2_key = MapString("AnotherPublishingCompany".to_string());
    // let publisher_2_holon_reference = HolonReference::Staged(StagedReference {
    //     holon_index: publisher_2_index.clone(),
    // });
    // publisher_2_holon
    //     .with_property_value(
    //         PropertyName(MapString("name".to_string())),
    //         BaseValue::StringValue(MapString("Another Publishing Company".to_string())),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("key".to_string())),
    //         BaseValue::StringValue(publisher_2_key.clone()),
    //     )?
    //     .with_property_value(
    //         PropertyName(MapString("description".to_string())),
    //         BaseValue::StringValue(MapString(
    //             "Again, We publish Holons for testing purposes".to_string(),
    //         )),
    //     )?;
    // test_case.add_stage_holon_step(publisher_2_holon.clone())?;

    // // //  ADD RELATIONSHIP: Cloned Book -> Publisher  //
    // let published_by_relationship_name = RelationshipName(MapString("PUBLISHED_BY".to_string()));
    // // set expected
    // let mut expected_book_holon = book_holon.clone();
    // expected_book_holon.property_map = changed_properties;
    // let mut expected_publisher_holon_collection = HolonCollection::new_staged();
    // expected_publisher_holon_collection
    //     .add_reference_with_key(Some(&publisher_2_key), &publisher_2_holon_reference)?;
    // expected_book_holon.relationship_map.0.insert(
    //     published_by_relationship_name.clone(),
    //     expected_publisher_holon_collection,
    // );

    // test_case.add_related_holons_step(
    //     cloned_book_index, // source holon
    //     published_by_relationship_name.clone(),
    //     vec![publisher_2_holon_reference],
    //     ResponseStatusCode::OK,
    //     expected_book_holon.clone(), // expected holon
    // )?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
