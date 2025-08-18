use rstest::*;

use crate::shared_test::{
    setup_book_author_steps_with_context,
    test_context::{init_test_context, TestContextConfigOption::TestFixture},
    test_data_types::{
        DancesTestCase, TestReference, BOOK_KEY, EDITOR_FOR, PERSON_2_KEY, PUBLISHER_KEY,
    },
};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;
use holons_core::{
    core_shared_objects::Holon,
    dances::ResponseStatusCode,
    reference_layer::{
        HolonReference, ReadableHolon, ReadableHolonReferenceLayer, StagedReference,
        WriteableHolon, WriteableHolonReferenceLayer,
    },
};
use integrity_core_types::{PropertyMap, PropertyName, RelationshipName};
use type_names::*;

/// Fixture for creating Simple StageNewFromClone Testcase
#[fixture]
pub fn simple_stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    // The fixture has its own Nursery which is used as a scratch pad during the test setup phase.
    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_test_context(TestFixture);
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

    let mut test_case = DancesTestCase::new(
        "Simple StageNewFromClone Testcase".to_string(),
        "Phase 1 clones a staged holon, Phase 2 clones a saved holon, changes some of its\n\
        properties, adds a relationship, commits it and then compares essential content of existing \n\
        holon and cloned holon".to_string(),
    );

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    //  ENSURE DATABASE COUNT -- Empty except for HolonSpace  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Use helper function to set up a book holon, 2 persons, a publisher, and a relationship from
    // the book to both persons. Note that this uses the fixture's Nursery as a place to hold the test data.
    // let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));

    let _author_relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    // The following assumes the fixture's nursery contains the same number of holons as
    // test executor's nursery will have staged immediately prior to commit.
    expected_count += staging_service.borrow().staged_count();

    // Get references to the Holons stashed in the fixture's Nursery.
    let book_key = MapString(BOOK_KEY.to_string());
    let book_ref = staging_service.borrow().get_staged_holon_by_base_key(&book_key)?;

    // Save info about publisher and person 2 to use in Phase 2
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let person_2_ref = staging_service.borrow().get_staged_holon_by_base_key(&person_2_key)?;

    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_ref = staging_service.borrow().get_staged_holon_by_base_key(&publisher_key)?;

    // The publisher holon will be the holon cloned in Phase II. Clone it here to use as a basis
    // for mirroring the Phase II test step actions.
    let expected_holon = publisher_ref.clone_holon(&*fixture_context)?;
    let expected_executor_holon_ref =
        staging_service.borrow().get_staged_holon_by_base_key(&publisher_key)?;

    // ******************     PHASE 1: CLONE A STAGED HOLON     **********************************
    // When stage_new_from_clone is executed (during the test execution phase), it will add an exact
    // copy of the book holon to the Nursery. For this phase of the test case we will commit
    // the duplicate. This step should fail on commit once we have duplicate-key prevention logic
    // But for now, this commit will succeed.
    //

    // // Do a local clone of the book holon
    // let mut expected_holon = book_holon_ref.clone_holon(&*fixture_context)?;
    //
    // // add the expected holon to the fixture's nursery to keep the staged references in sync between
    // // the fixture and the test executor
    // let expected_holon_reference = staging_service.borrow().stage_new_holon(expected_holon)?;

    // Add a test step to the test case that will stage an exact duplicate of the book holon.

    test_case.add_stage_new_from_clone_step(
        TestReference::StagedHolon(book_ref.clone()),
        book_key,
        ResponseStatusCode::OK,
    )?;

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
    // NOTE: Since Phase I concludes with a `commit` step, the test executor's Nursery will be reset.
    // But the fixture's nursery never commits, so it will continue to grow. This means we will need
    // to track index position separately in Phase 2 of the fixture and synthetically create
    // the StagedReference passed to its test step constructors

    // Step 1: stage_new_from_clone for the publisher holon committed in Phase I.
    // This will create an exact copy of the publisher holon.

    test_case.add_stage_new_from_clone_step(
        TestReference::SavedHolon(publisher_key.clone()),
        publisher_key.clone(),
        ResponseStatusCode::OK,
    )?;

    // Mirror the test step in the fixture's Nursery
    let expected_fixture_holon_ref = staging_service.borrow().stage_new_holon(expected_holon)?;

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
        expected_executor_holon_ref.clone(),
        changed_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    for (property_name, value) in changed_properties.iter() {
        expected_fixture_holon_ref.with_property_value(
            &*fixture_context,
            property_name.clone(),
            value.clone(),
        )?;
    }

    // Step 3: add_related_holons step to stage an additional relationship for the clone
    let publisher_relationship_name = EDITOR_FOR.to_relationship_name();

    let mut fixture_holons_to_add: Vec<HolonReference> = Vec::new();
    let mut holons_to_add: Vec<TestReference> = Vec::new();

    fixture_holons_to_add.push(HolonReference::Staged(person_2_ref));

    let person_test_ref = TestReference::SavedHolon(person_2_key.clone());
    holons_to_add.push(person_test_ref);

    // Update the fixture's expected holon
    expected_fixture_holon_ref.add_related_holons(
        &*fixture_context,
        &publisher_relationship_name,
        fixture_holons_to_add.clone(),
    )?;

    test_case.add_related_holons_step(
        expected_executor_holon_ref.clone(), // source holon
        publisher_relationship_name.clone(),
        holons_to_add,
        ResponseStatusCode::OK,
        Holon::Transient(expected_fixture_holon_ref.clone_holon(&*fixture_context).unwrap()), // expected holon
    )?;

    //  COMMIT  // the cloned & modified Book Holon
    test_case.add_commit_step()?;
    expected_count += 1;

    //  ENSURE DATABASE COUNT -- 7 Holons  //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT //
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
