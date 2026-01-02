use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_test::{fixture_holons, DancesTestCase, FixtureHolons, TestReference};
use pretty_assertions::assert_eq;
use std::{collections::BTreeMap, sync::Arc};
use tracing::{error, info};

use holons_prelude::prelude::*;
use rstest::*;

use crate::helpers::{init_fixture_context, BOOK_KEY, PERSON_1_KEY, PERSON_2_KEY, PUBLISHER_KEY};
use type_names::{CorePropertyTypeName::Description, ToPropertyName};

use super::setup_book_author_steps_with_context;

// TODO: enhance test capabilities, ie trying to remove a property that doesnt exist, etc trying to add invalid property
// Add again after removing

/// For both Transient and Staged references:
/// modifies an existing property, adds new properties, removes properties, then changes a property again and adds properties again.
///
///
#[fixture]
pub fn simple_add_remove_properties_fixture() -> Result<DancesTestCase, HolonError> {
    // == Init == //
    let mut test_case = DancesTestCase::new(
        "Simple Add / Remove Holon Properties Testcase".to_string(),
        "Tests the adding and removing of Holon properties for both Staged and Transient references".to_string(),
    );
    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();
    setup_book_author_steps_with_context(&*fixture_context, &mut test_case, &mut fixture_holons)?;
    // == //

    // -- ADD STEP -- //

    // EXAMPLE (Transient) //
    let example_key = MapString("EXAMPLE_KEY".to_string());
    let example_transient_reference = new_holon(&*fixture_context, Some(example_key.clone()))?;
    // Mint transient source token
    let example_transient_token = fixture_holons.add_transient_with_key(
        &example_transient_reference,
        example_key.clone(),
        example_transient_reference.clone(),
    );
    // Add properties
    let mut example_properties = PropertyMap::new();
    example_properties
        .insert("Description".to_property_name(), "This is an example description".to_base_value());
    example_properties
        .insert("ExampleProperty".to_property_name(), "Adding a property".to_base_value());
    example_properties.insert("Integer".to_property_name(), (-1).to_base_value());
    example_properties.insert("Boolean".to_property_name(), false.to_base_value());

    let modified_example_token = test_case.add_with_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        example_transient_token,
        example_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // BOOK (Staged) //
    let book_key = MapString(BOOK_KEY.to_string());
    let book_source_token = fixture_holons
        .get_latest_by_key(&book_key)
        .expect(&format!("Id must exist in FixtureHolons, for key: {:?}", book_key));
    // Add
    let mut book_properties = PropertyMap::new();
    book_properties.insert("Description".to_property_name(), "Changed description".to_base_value());
    book_properties.insert("title".to_property_name(), BOOK_KEY.to_base_value());
    book_properties
        .insert("NewProperty".to_property_name(), "This is another property".to_base_value());
    book_properties.insert("Int".to_property_name(), 42.to_base_value());
    book_properties.insert("Bool".to_property_name(), true.to_base_value());

    let modified_book_token = test_case.add_with_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        book_source_token,
        book_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // -- REMOVE STEP -- //

    // TRANSIENT //
    let mut transient_holon_properties_to_remove = BTreeMap::new();
    // Note: Technically for a remove_property_value call, a value is not required, however the RequestBody for the Dance
    // takes a ParameterValues(PropertyMap) and thus a full map is populated for the step.
    // We could consider changing this - ie the Body, as what's interesting is the value here is arbitrary and could be incorrect,
    // which would go unflagged and therefore ultimately be misleading for readers of the test code in such a scenario.
    transient_holon_properties_to_remove
        .insert("Boolean".to_property_name(), false.to_base_value());
    transient_holon_properties_to_remove.insert("Integer".to_property_name(), (-1).to_base_value());
    // Remove
    example_properties.remove(&"Integer".to_property_name());
    example_properties.remove(&"Boolean".to_property_name());

    let _modified_book_token_after_remove = test_case.add_remove_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        modified_example_token,
        transient_holon_properties_to_remove,
        ResponseStatusCode::OK,
    )?;

    // STAGED //
    let mut staged_holon_properties_to_remove = BTreeMap::new();
    staged_holon_properties_to_remove
        .insert("NewProperty".to_property_name(), "This is another property".to_base_value());
    staged_holon_properties_to_remove.insert("Int".to_property_name(), 42.to_base_value());
    staged_holon_properties_to_remove.insert("Bool".to_property_name(), true.to_base_value());
    // Remove
    book_properties.remove(&"NewProperty".to_property_name());
    book_properties.remove(&"Int".to_property_name());
    book_properties.remove(&"Bool".to_property_name());

    let _modified_example_token_after_remove = test_case.add_remove_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        modified_book_token,
        staged_holon_properties_to_remove,
        ResponseStatusCode::OK,
    )?;

    // TODO:
    // -- ADD (Again) STEP -- // Confirming add succeeds after removal of things

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
