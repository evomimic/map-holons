use crate::shared_test::{
    setup_book_author_steps_with_context,
    test_context::init_fixture_context,
    test_data_types::{
        DancesTestCase, TestReference, BOOK_KEY, EDITOR_FOR, PERSON_2_KEY, PUBLISHER_KEY,
    },
};
use rstest::*;
use tracing::{debug, info};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;
use core_types::{PropertyMap, PropertyName, RelationshipName};
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::Holon,
    dances::ResponseStatusCode,
    reference_layer::{
        HolonReference, ReadableHolon, StagedReference, TransientReference, WritableHolon,
    },
};
use type_names::*;

/// Fixture for creating Simple StageNewFromClone Testcase
#[fixture]
pub fn simple_stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    // The fixture has its own TransientHolonManager which is used as a scratch pad during the test setup phase.
    // This allows them to be assigned TransientReferences and also retrieved by either index or key
    let fixture_context = init_fixture_context();

    let mut test_case = DancesTestCase::new(
        "Simple StageNewFromClone Testcase".to_string(),
        "Phase 1 clones a staged holon, Phase 2 clones a saved holon, changes some of its\n\
        properties, adds a relationship, commits it and then compares essential content of existing \n\
        holon and cloned holon".to_string(),
    );

    info!("In simple_stage_new_from_clone_fixture: {:?}", test_case);

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty except for HolonSpace  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Use helper function to set up a book holon, 2 persons, a publisher, and a relationship from
    // the book to both persons. Note that this uses the fixture's Nursery as a place to hold the test data.

    let _author_relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    // The following assumes the fixture's nursery contains the same number of holons as
    // test executor's nursery will have staged immediately prior to commit.
    expected_count += staged_count(&*fixture_context);

    // Get references to the Holons stashed in the fixture's transient_holon_manager.
    let book_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference = get_transient_holon_by_base_key(&*fixture_context, &book_key)?;

    // Save info about publisher and person 2 to use in Phase 2
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let person_2_staged_reference = get_staged_holon_by_base_key(&*fixture_context, &person_2_key)?;

    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_transient_reference =
        get_transient_holon_by_base_key(&*fixture_context, &publisher_key)?;

    // // The publisher holon will be the holon cloned in Phase II. Clone it here to use as a basis
    // // for mirroring the Phase II test step actions.
    let publisher_staged_reference =
        get_staged_holon_by_base_key(&*fixture_context, &publisher_key)?;

    // ******************     PHASE 1: CLONE A STAGED HOLON     **********************************
    // When stage_new_from_clone is executed (during the test execution phase), it will add an exact
    // copy of the book holon to the Nursery. For this phase of the test case we will commit
    // the duplicate. This step should fail on commit once we have duplicate-key prevention logic
    // But for now, this commit will succeed.
    //

    // Add a test step to the test case that will stage an exact duplicate of the book holon.

    test_case.add_stage_new_from_clone_step(
        TestReference::TransientHolon(book_transient_reference.clone()),
        book_key,
        ResponseStatusCode::OK,
    )?;
    stage_new_holon_api(&*fixture_context, book_transient_reference.clone())?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;
    expected_count += 1;

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // ******************     PHASE 2: CLONE A SAVED HOLON     ************************************
    // TEST STEPS:
    // Step 1: stage_new_from_clone for the publisher holon committed in phase 1
    // Step 2: with_properties step to modify the staged clone's properties
    // Step 3: add
    // Step 4: Commit
    // Step 5: Ensure DB count (s/b 1 more than previous commit)
    // Step 6: Match DB content to ensure original holon is unchanged and clone has expected
    // properties and related holons
    //
    // NOTE: In each step we need to mirror the actions being added via test steps in the
    // fixture's nursery in order to build the expected holon.
    //
    // Step 1: stage_new_from_clone for the publisher holon committed in Phase I.
    // This will create an exact copy of the publisher holon.

    test_case.add_stage_new_from_clone_step(
        TestReference::SavedHolon(publisher_key.clone()),
        publisher_key.clone(),
        ResponseStatusCode::OK,
    )?;
    // Mirror the test step in the fixture's Nursery
    let expected_fixture_holon =
        stage_new_holon_api(&*fixture_context, publisher_transient_reference.clone())?; // Staged

    // Step 2: with_properties step to modify the staged clone's properties
    let mut changed_properties = PropertyMap::new();

    changed_properties.insert(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(publisher_key),
    );
    changed_properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("this is testing a clone from a saved Holon, changing it, modifying relationships, then committing".to_string())));

    test_case.add_with_properties_step(
        HolonReference::Staged(publisher_staged_reference.clone()),
        changed_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    for (property_name, value) in changed_properties.iter() {
        expected_fixture_holon.with_property_value(
            &*fixture_context,
            property_name.clone(),
            value.clone(),
        )?;
    }

    // Step 3: add_related_holons step to stage an additional relationship for the clone
    let publisher_relationship_name = EDITOR_FOR.to_relationship_name();

    let mut fixture_holons_to_add: Vec<HolonReference> = Vec::new();
    let mut holons_to_add: Vec<TestReference> = Vec::new();

    fixture_holons_to_add.push(HolonReference::Staged(person_2_staged_reference));

    let person_test_reference = TestReference::SavedHolon(person_2_key.clone());
    holons_to_add.push(person_test_reference);

    // Update the fixture's expected holon
    expected_fixture_holon.add_related_holons(
        &*fixture_context,
        &publisher_relationship_name,
        fixture_holons_to_add.clone(),
    )?;

    let expected_holon =
        HolonReference::Transient(expected_fixture_holon.clone_holon(&*fixture_context)?);

    test_case.add_related_holons_step(
        HolonReference::Staged(publisher_staged_reference), // source holon
        publisher_relationship_name.clone(),
        holons_to_add,
        ResponseStatusCode::OK,
        expected_holon, // expected holon
    )?;

    //  COMMIT  // the cloned & modified Book Holon
    test_case.add_commit_step()?;
    expected_count += 1;

    //  ENSURE DATABASE COUNT -- 7 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT //
    test_case.add_match_saved_content_step()?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
