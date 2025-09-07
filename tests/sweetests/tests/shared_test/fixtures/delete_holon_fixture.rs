// #![allow(dead_code)]

use rstest::*;

use crate::shared_test::{
    test_context::init_fixture_context,
    test_data_types::{DancesTestCase, BOOK_KEY},
};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{HolonError, HolonId};
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression,
    reference_layer::{TransientReference, WriteableHolonReferenceLayer},
    stage_new_holon_api, HolonCollection, HolonsContextBehavior,
};
use integrity_core_types::{PropertyMap, PropertyName, PropertyValue, RelationshipName};

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    let fixture_context = init_fixture_context();

    // Get transient manager behavior
    let transient_manager_behavior_service =
        fixture_context.get_space_manager().get_transient_behavior_service();
    let transient_manager_behavior = transient_manager_behavior_service.borrow();

    //  ADD STEP:  STAGE:  Book Holon  //
    let book_holon_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference =
        transient_manager_behavior.create_empty(book_holon_key.clone())?;

    book_transient_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("title".to_string())),
            BaseValue::StringValue(book_holon_key.clone()),
        )?
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("description".to_string())),
            BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            )))?;

    test_case.add_stage_holon_step(book_transient_reference.clone())?;

    stage_new_holon_api(&*fixture_context, book_transient_reference)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::OK)?;

    // ADD STEP: DELETE HOLON - Invalid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::NotFound)?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
