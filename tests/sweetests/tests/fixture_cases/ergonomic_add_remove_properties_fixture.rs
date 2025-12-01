use holons_test::{DancesTestCase, FixtureHolons};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tracing::{error, info};

use holons_prelude::prelude::*;
use rstest::*;

use crate::helpers::{init_fixture_context, BOOK_KEY, PERSON_1_KEY, PERSON_2_KEY, PUBLISHER_KEY};
use type_names::{CorePropertyTypeName::Description, ToPropertyName};

use super::setup_book_author_steps_with_context;

#[fixture]
pub fn ergonomic_add_remove_properties_fixture() -> Result<DancesTestCase, HolonError> {
    // == Init == //
    let mut test_case = DancesTestCase::new(
        "Ergonomic Add / Remove Holon Properties Testcase".to_string(),
        "Tests the adding and removing of Holon properties using all combinations of ergonomic values".to_string(),
    );
    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();
    setup_book_author_steps_with_context(&*fixture_context, &mut test_case, &mut fixture_holons)?;
    // == //

    // Modifies an existing property and adds a new property, passing Enum, MapString, String, and string literal
    // each through both parameters of PropertyName and BaseValue

    // TEST FIXTURE //
    //
    // Flexes ToPropertyName and ToBaseValue trait combinations

    let book_key = MapString(BOOK_KEY.to_string());
    let book_source_token = fixture_holons.get_latest_by_key(&book_key)?;
    let mut book_transient_reference = book_source_token.transient().clone();

    let mut properties = PropertyMap::new();
    properties
        .insert(Description.to_property_name(), "Changed description".to_string().to_base_value()); // enum, String
    properties.insert("NewProperty".to_property_name(), "This is another property".to_base_value()); // str, str
    test_case.add_with_properties_step(book_source_token, properties, ResponseStatusCode::OK)?;

    book_transient_reference
        .with_property_value(&*fixture_context, "Description", "Changed description")?
        .with_property_value(
            &*fixture_context,
            "NewProperty".to_string(),
            "This is another property".to_string(),
        )?;

    // Mint transient token to reflect expected_content
    let book_mod_token = fixture_holons.add_transient_with_key(
        &book_transient_reference,
        book_key,
        &book_transient_reference.essential_content(&*fixture_context)?,
    )?;

    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_source_token = fixture_holons.get_latest_by_key(&publisher_key)?;
    let mut publisher_transient_reference = publisher_source_token.transient().clone();

    let mut properties = PropertyMap::new();
    properties
        .insert(Description.to_property_name(), "Changed description".to_string().to_base_value()); // enum, String
    properties.insert("NewProperty".to_property_name(), "This is another property".to_base_value()); // str, str
    test_case.add_with_properties_step(
        publisher_source_token,
        properties,
        ResponseStatusCode::OK,
    )?;

    publisher_transient_reference
        .with_property_value(&*fixture_context, "Description", "Changed description")?
        .with_property_value(
            &*fixture_context,
            "NewProperty".to_string(),
            "This is another property".to_string(),
        )?;

    // Mint transient token to reflect expected_content
    let publisher_mod_token = fixture_holons.add_transient_with_key(
        &publisher_transient_reference,
        publisher_key,
        &publisher_transient_reference.essential_content(&*fixture_context)?,
    )?;

    // REMOVE STEP //

    // let mut transient_holon_properties_to_remove = BTreeMap::new();
    // transient_holon_properties_to_remove.insert(
    //     MapString("New Property".to_string().to_property_name()),
    //     MapString("This is another property".to_string()).to_base_value(),
    // ); // MapString, Mapstring
    // test_case.add_remove_properties_step(
    //     HolonReference::Transient(book_transient_reference),
    //     transient_holon_properties_to_remove,
    //     ResponseStatusCode::OK,
    // )?;

    // let mut staged_holon_properties_to_remove = BTreeMap::new();
    // staged_holon_properties_to_remove.insert(
    //     "Publisher Property".to_property_name(),
    //     "Adding a property".to_string().to_base_value(),
    // );
    // test_case.add_remove_properties_step(
    //     HolonReference::Staged(publisher_staged_reference),
    //     staged_holon_properties_to_remove,
    //     ResponseStatusCode::OK,
    // )?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
