use holons_test::{DancesTestCase, TestCaseInit};

use holons_prelude::prelude::*;
use rstest::*;
use std::collections::BTreeMap;

use holons_test::harness::helpers::BOOK_KEY;
use type_names::ToPropertyName;

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

    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings, } =
        TestCaseInit::new(
        "Simple Add / Remove Holon Properties Testcase".to_string(),
        "Tests the adding and removing of Holon properties for both Staged and Transient references".to_string(),
    );

    setup_book_author_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    // == //

    // -- ADD STEP -- //

    // EXAMPLE (Transient) //
    let example_key = MapString("EXAMPLE_KEY".to_string());
    let example_transient_reference =
        fixture_context.mutation().new_holon(Some(example_key.clone()))?;
    // Mint
    let mut example_properties = PropertyMap::new();
    let example_transient_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        example_transient_reference,
        example_properties.clone(),
        Some(example_key.clone()),
        ResponseStatusCode::OK,
    )?;
    // Add properties
    example_properties
        .insert("Description".to_property_name(), "This is an example description".to_base_value());
    example_properties
        .insert("ExampleProperty".to_property_name(), "Adding a property".to_base_value());
    example_properties.insert("Integer".to_property_name(), (-1).to_base_value());
    example_properties.insert("Boolean".to_property_name(), false.to_base_value());

    let modified_example_token = test_case.add_with_properties_step(
        &mut fixture_holons,
        example_transient_token,
        example_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // BOOK (Staged) //
    let _book_key = MapString(BOOK_KEY.to_string());
    let book_source_token = fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixture return_items to contain a staged-intent token associated with 'Book' label").clone();
    // Add
    let mut book_properties = PropertyMap::new();
    book_properties.insert("Description".to_property_name(), "Changed description".to_base_value());
    book_properties.insert("title".to_property_name(), BOOK_KEY.to_base_value());
    book_properties
        .insert("NewProperty".to_property_name(), "This is another property".to_base_value());
    book_properties.insert("Int".to_property_name(), 42.to_base_value());
    book_properties.insert("Bool".to_property_name(), true.to_base_value());

    let modified_book_token = test_case.add_with_properties_step(
        &mut fixture_holons,
        book_source_token,
        book_properties.clone(),
        ResponseStatusCode::OK,
    )?;

    // -- REMOVE STEP -- //

    // TRANSIENT //
    let mut transient_holon_properties_to_remove = BTreeMap::new();
    // Note: Technically for a remove_property_value call, a value is not required. However, the
    // RequestBody for the Dance takes a ParameterValues(PropertyMap) and thus a full map is
    // populated for the step. We should consider changing this. The value here is arbitrary and
    // could be incorrect, which would go unflagged and therefore ultimately be misleading for
    // readers of the test code in such a scenario.
    transient_holon_properties_to_remove
        .insert("Boolean".to_property_name(), false.to_base_value());
    transient_holon_properties_to_remove.insert("Integer".to_property_name(), (-1).to_base_value());
    // Remove
    example_properties.remove(&"Integer".to_property_name());
    example_properties.remove(&"Boolean".to_property_name());

    let _modified_book_token_after_remove = test_case.add_remove_properties_step(
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
        &mut fixture_holons,
        modified_book_token,
        staged_holon_properties_to_remove,
        ResponseStatusCode::OK,
    )?;

    // TODO:
    // -- ADD (Again) STEP -- // Confirming add succeeds after removal of things

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
