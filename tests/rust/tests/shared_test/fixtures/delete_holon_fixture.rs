#![allow(dead_code)]

// use crate::get_holon_by_key_from_test_state;
use core::panic;
use std::cell::RefCell;
use tracing::{error, info, warn};
//use holochain::core::author_key_is_valid;

use crate::shared_test::test_data_types::DancesTestCase;
use holons_core::core_shared_objects::{Holon, HolonCollection, HolonError, RelationshipName};
use holons_core::dances::dance_response::ResponseStatusCode;
use holons_core::query_layer::QueryExpression;
use holons_core::HolonsContextBehavior;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

/// Fixture for creating a DeleteHolon Testcase
#[fixture]
pub fn delete_holon_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "DeleteHolon Testcase".to_string(),
        "Tests delete_holon dance, matches expected response, in the OK case confirms get_holon_by_id returns NotFound error response for the given holon_to_delete ID.".to_string(),
    );

    //  ADD STEP:  STAGE:  Book Holon  //
    let mut book_holon = Holon::new();
    let book_holon_key = MapString(
        "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
    );
    book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        Some(BaseValue::StringValue(book_holon_key.clone()),
    ))?;
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
