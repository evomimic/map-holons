// use holons_test::{DancesTestCase, FixtureHolons};
// use pretty_assertions::assert_eq;
// use std::collections::BTreeMap;
// use tracing::{error, info};

// use holons_prelude::prelude::*;
// use rstest::*;

// use crate::helpers::{init_fixture_context, BOOK_KEY, PERSON_1_KEY, PERSON_2_KEY, PUBLISHER_KEY};
// use type_names::{CorePropertyTypeName::Description, ToPropertyName};

// use super::setup_book_author_steps_with_context;

// #[fixture]
// pub fn ergonomic_add_remove_properties_fixture() -> Result<DancesTestCase, HolonError> {
//     // == Init == //
//     let mut test_case = DancesTestCase::new(
//         "Ergonomic Add / Remove Holon Properties Testcase".to_string(),
//         "Tests the adding and removing of Holon properties using all combinations of ergonomic values".to_string(),
//     );
//     let fixture_context = init_fixture_context();
//     let mut fixture_holons = FixtureHolons::new();
//     setup_book_author_steps_with_context(&*fixture_context, &mut test_case, &mut fixture_holons)?;
//     // == //

//     // Modifies an existing property and adds a new property, passing Enum, MapString, String, and string literal
//     // each through both parameters of PropertyName and BaseValue

//     // TEST FIXTURE //

//     let book_key = MapString(BOOK_KEY.to_string());
//     let mut book_source_token =
//         fixture_holons.get_latest_by_key(&book_key)?;

//     // Mint transient token for mutation
//     let mut book_mod_token = fixture_holons.add_transient_with_key(
//         &book_source_token.transient(),
//         book_key,
//         ,
//     )?;

//     let mut properties = PropertyMap::new();
//     properties.insert("TITLE".to_property_name(), "Dune (Transient Clone)".to_base_value());
//     properties.insert("EDITION".to_property_name(), 1.to_base_value());
//     test_case.add_with_properties_step(clone_from_transient_staged, properties, ResponseStatusCode::OK)?;
//     book_mod_token
//         .with_property_value(
//             &*fixture_context,
//             "New Property".to_string(),
//             "This is another property".to_string(),
//         )?
//         .with_property_value(&*fixture_context, "Description", "Changed Description")?;

//     let mut publisher_staged_reference =
//         get_staged_holon_by_base_key(&*fixture_context, &MapString(PUBLISHER_KEY.to_string()))?;
//     publisher_staged_reference
//         .with_property_value(
//             &*fixture_context,
//             MapString("Publisher Property".to_string()),
//             BaseValue::StringValue(MapString("Adding a property".to_string())),
//         )?
//         .with_property_value(
//             &*fixture_context,
//             Description,
//             MapString("New Publisher Description".to_string()),
//         )?;

//     // EXECUTOR STEP - to ensure expected //
//     //
//     // Flexes ToPropertyName and ToBaseValue trait combinations

//     let mut expected_book_property_map = BTreeMap::new();
//     expected_book_property_map.insert(
//         Description.to_property_name(),
//         MapString("Changed Description".to_string()).to_base_value(),
//     );
//     expected_book_property_map.insert(
//         "New Property".to_property_name(),
//         "This is another property".to_string().to_base_value(),
//     );

//     test_case.add_with_properties_step(
//         HolonReference::Transient(book_transient_reference.clone()),
//         expected_book_property_map.clone(),
//         ResponseStatusCode::OK,
//     )?;

//     let mut expected_publisher_property_map = BTreeMap::new();
//     expected_publisher_property_map.insert(
//         MapString("Description".to_string()).to_property_name(),
//         "New Publisher Description".to_base_value(),
//     );
//     expected_publisher_property_map.insert(
//         PropertyName(MapString("Publisher Property".to_string())).to_property_name(),
//         BaseValue::StringValue(MapString("Adding a property".to_string())).to_base_value(),
//     );

//     test_case.add_with_properties_step(
//         HolonReference::Staged(publisher_staged_reference.clone()),
//         expected_publisher_property_map.clone(),
//         ResponseStatusCode::OK,
//     )?;

//     // REMOVE STEP //

//     let mut transient_holon_properties_to_remove = BTreeMap::new();
//     transient_holon_properties_to_remove.insert(
//         "New Property".to_property_name(),
//         "This is another property".to_string().to_base_value(),
//     );
//     test_case.add_remove_properties_step(
//         HolonReference::Transient(book_transient_reference),
//         transient_holon_properties_to_remove,
//         ResponseStatusCode::OK,
//     )?;

//     let mut staged_holon_properties_to_remove = BTreeMap::new();
//     staged_holon_properties_to_remove.insert(
//         "Publisher Property".to_property_name(),
//         "Adding a property".to_string().to_base_value(),
//     );
//     test_case.add_remove_properties_step(
//         HolonReference::Staged(publisher_staged_reference),
//         staged_holon_properties_to_remove,
//         ResponseStatusCode::OK,
//     )?;

//     // Load test_session_state
//     test_case.load_test_session_state(&*fixture_context);

//     Ok(test_case)
// }
