// #![allow(dead_code)]

// use crate::get_holon_by_key_from_test_state;
use core::panic;
use std::cell::RefCell;
use tracing::{error, info, warn};
//use holochain::core::author_key_is_valid;

use crate::shared_test::test_data_types::DancesTestCase;
use holons_core::core_shared_objects::{Holon, HolonCollection, HolonError, RelationshipName};
use holons_core::dances::dance_response::ResponseStatusCode;
use holons_core::query_layer::QueryExpression;
use holons_core::{HolonsContextBehavior, StagedReference};
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure DB starts empty, stage Book and Person Holons, add properties, commit, ensure db count is 2".to_string(),
    );

    let mut expected_holons = Vec::new();
    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Create book Holon with properties //
    let mut book_holon = Holon::new();
    book_holon.with_property_value(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;
    book_holon.with_property_value(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(
            "Emerging World: The Evolution of Consciousness and the Future of Humanity".to_string(),
        )),
    )?;

    //  STAGE:  Book Holon  //
    test_case.add_stage_holon_step(book_holon.clone())?;
    expected_holons.push(book_holon.clone());
    expected_count += 1;

    //  PROPERTIES:  Book  //
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("description".to_string())),
        BaseValue::StringValue(MapString("Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_string())),
    );
    test_case.add_with_properties_step(
        StagedReference::from_index(0),
        properties,
        ResponseStatusCode::OK,
    )?;

    //  STAGE:  Person Holon  //
    let person_holon = Holon::new();
    test_case.add_stage_holon_step(person_holon.clone())?;
    expected_holons.push(person_holon.clone());
    expected_count += 1;

    //  PROPERTIES:  Person  //
    let mut properties = PropertyMap::new();
    properties.insert(
        PropertyName(MapString("key".to_string())),
        BaseValue::StringValue(MapString("RogerBriggs".to_string())),
    );
    properties.insert(
        PropertyName(MapString("first name".to_string())),
        BaseValue::StringValue(MapString("Roger".to_string())),
    );
    properties.insert(
        PropertyName(MapString("last name".to_string())),
        BaseValue::StringValue(MapString("Briggs".to_string())),
    );
    test_case.add_with_properties_step(
        StagedReference::from_index(1),
        properties,
        ResponseStatusCode::OK,
    )?;

    //  COMMIT  //
    test_case.add_commit_step()?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    Ok(test_case.clone())
}
