// #![allow(dead_code)]

use rstest::*;

use crate::shared_test::{
    test_context::{init_test_context, TestContextConfigOption::TestFixture},
    test_data_types::{DancesTestCase, BOOK_KEY},
};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{HolonError, HolonId};
use holons_core::{
    core_shared_objects::{Holon, TransientHolon},
    dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression,
    stage_new_holon_api, HolonCollection, HolonsContextBehavior,
};
use integrity_core_types::{PropertyMap, PropertyName, PropertyValue, RelationshipName};

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    let fixture_context = init_test_context(TestFixture);

    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    //  ADD STEP:  STAGE:  Book Holon  //
    let mut book_holon = TransientHolon::new();
    let book_holon_key = MapString(BOOK_KEY.to_string());

    book_holon
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(book_holon_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("title".to_string())),
            Some(BaseValue::StringValue(book_holon_key.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("description".to_string())),
            Some(BaseValue::StringValue(MapString(
                "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string(),
            ))),
        )?;
    test_case.add_stage_holon_step(book_holon.clone())?;
    let book_ref = stage_new_holon_api(&*fixture_context, book_holon)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::OK)?;

    // ADD STEP: DELETE HOLON - Invalid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::NotFound)?;

    Ok(test_case.clone())
}
