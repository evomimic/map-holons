// #![allow(dead_code)]

use rstest::*;

use crate::shared_test::test_data_types::DancesTestCase;
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::HolonId;
use holons_core::{
    core_shared_objects::holon::Holon, dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression, HolonCollection, HolonError, HolonsContextBehavior,
    RelationshipName,
};
use integrity_core_types::{PropertyMap, PropertyName, PropertyValue};

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    //  ADD STEP:  STAGE:  Book Holon  //
    let mut book_holon = Holon::new_transient();
    let book_holon_key = MapString(
        "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
    );
    book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        Some(BaseValue::StringValue(book_holon_key.clone())),
    )?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        Some(BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    ))?.with_property_value(
        PropertyName(MapString("description".to_string())),
        Some(BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string())),
    ))?;
    test_case.add_stage_holon_step(book_holon)?;

    // ADD STEP:  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;

    // ADD STEP: DELETE HOLON - Valid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::OK)?;

    // ADD STEP: DELETE HOLON - Invalid //
    test_case.add_delete_holon_step(book_holon_key.clone(), ResponseStatusCode::NotFound)?;

    Ok(test_case.clone())
}
