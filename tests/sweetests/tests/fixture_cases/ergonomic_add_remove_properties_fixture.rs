// use holons_test::{DancesTestCase, FixtureHolons};
// use pretty_assertions::assert_eq;
// use std::{collections::BTreeMap, sync::Arc};
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

//     // Ergonomics::
//     // ToPropertyName : Enum, String, str, MapString, PropertyName  ... complete ✅
//     // ToBaseValue : String, str, MapString, int, bool, MapEnumValue, BaseValue  ... complete ✅
//     // Add : Enum, String, str, MapString, PropertyName  ... complete ✅
//     // Remove : String, str, MapString, PropertyName  ... complete ✅

//     // === TEST FIXTURE === //
//     //
//     // Flexes ToPropertyName and ToBaseValue trait combinations

//     // -- ADD STEP -- //

//     // BOOK //
//     let book_key = MapString(BOOK_KEY.to_string());
//     let mut book_transient_reference = TransientReference::from_temporary_id(
//         fixture_holons
//             .get_id_by_key(&book_key)
//             .expect(&format!("Id must exist in FixtureHolons, for key: {:?}", book_key)),
//     );

//     let mut properties = PropertyMap::new();
//     properties
//         .insert(Description.to_property_name(), "Changed description".to_string().to_base_value()); // Enum, String
//     properties.insert("NewProperty".to_property_name(), "This is another property".to_base_value()); // str, str
//     properties.insert("Int".to_property_name(), 42.to_base_value()); // str, int
//     properties.insert("Bool".to_property_name(), true.to_base_value()); // str, bool
//                                                                         // Modify source
//     book_transient_reference
//         .with_property_value(&*fixture_context, Description, "Changed description")? // Enum, str
//         .with_property_value(
//             &*fixture_context,
//             "NewProperty".to_string(),
//             "This is another property".to_string(),
//         )?
//         .with_property_value(&*fixture_context, "Int", 42)? // str, int
//         .with_property_value(&*fixture_context, "Bool", true)?; // str, bool
//                                                                 // Mint transient token with the expected_content
//     let book_mod_token = fixture_holons.add_transient_with_key(
//         &book_transient_reference,
//         book_key.clone(),
//         &book_transient_reference.essential_content(&*fixture_context)?,
//     )?;
//     // Add step with the new mint to properly reflect expected
//     test_case.add_with_properties_step(book_mod_token, properties, ResponseStatusCode::OK)?;

//     // PUBLSIHER //
//     let publisher_key = MapString(PUBLISHER_KEY.to_string());
//     let mut publisher_transient_reference = TransientReference::from_temporary_id(
//         fixture_holons
//             .get_id_by_key(&publisher_key)
//             .expect(&format!("Id must exist in FixtureHolons, for key: {:?}", publisher_key)),
//     );

//     let mut properties = PropertyMap::new();
//     properties.insert(
//         PropertyName(MapString("Description".to_string())).to_property_name(), // PropertyName
//         MapString("Changing description".to_string()).to_base_value(),         // MapString
//     );
//     properties.insert(
//         "PublisherProperty".to_string().to_property_name(), // String
//         BaseValue::StringValue(MapString("Adding a property".to_string())).to_base_value(), // BaseValue
//     );
//     // Mod
//     publisher_transient_reference
//         .with_property_value(
//             &*fixture_context,
//             PropertyName(MapString("Description".to_string())), // PropertyName,
//             MapEnumValue(MapString("Changing description".to_string())), //  MapEnumValue
//         )?
//         .with_property_value(
//             &*fixture_context,
//             MapString("PublisherProperty".to_string()), // MapString
//             MapString("Adding a property".to_string()), // MapString
//         )?;
//     // Mint
//     let publisher_mod_token = fixture_holons.add_transient_with_key(
//         &publisher_transient_reference,
//         publisher_key.clone(),
//         &publisher_transient_reference.essential_content(&*fixture_context)?,
//     )?;
//     // Add
//     test_case.add_with_properties_step(publisher_mod_token, properties, ResponseStatusCode::OK)?;

//     // -- REMOVE STEP -- //

//     // TRANSIENT
//     let mut transient_holon_properties_to_remove = BTreeMap::new();
//     transient_holon_properties_to_remove.insert(
//         MapString("NewProperty".to_string()).to_property_name(), // MapString
//         MapString("This is another property".to_string()).to_base_value(), // MapString
//     );
//     transient_holon_properties_to_remove.insert("Int".to_property_name(), 42.to_base_value());
//     transient_holon_properties_to_remove.insert("Bool".to_property_name(), true.to_base_value());
//     // Mod
//     book_transient_reference
//         .remove_property_value(&*fixture_context, "NewProperty".to_string())?
//         .remove_property_value(&*fixture_context, "Int")?
//         .remove_property_value(&*fixture_context, MapString("Bool".to_string()))?;
//     // Mint
//     let book_mod_token = fixture_holons.add_transient_with_key(
//         &book_transient_reference,
//         book_key,
//         &book_transient_reference.essential_content(&*fixture_context)?,
//     )?;
//     // Add
//     test_case.add_remove_properties_step(
//         book_mod_token,
//         transient_holon_properties_to_remove,
//         ResponseStatusCode::OK,
//     )?;

//     // STAGED
//     let mut staged_holon_properties_to_remove = BTreeMap::new();
//     staged_holon_properties_to_remove.insert(
//         "PublisherProperty".to_property_name(),          // str
//         "Adding a property".to_string().to_base_value(), // String
//     );
//     // Mod
//     publisher_transient_reference.remove_property_value(
//         &*fixture_context,
//         PropertyName(MapString("PublisherProperty".to_string())),
//     )?;
//     // Mint
//     let publisher_mod_token = fixture_holons.add_transient_with_key(
//         &publisher_transient_reference,
//         publisher_key,
//         &publisher_transient_reference.essential_content(&*fixture_context)?,
//     )?;
//     test_case.add_remove_properties_step(
//         publisher_mod_token,
//         staged_holon_properties_to_remove,
//         ResponseStatusCode::OK,
//     )?;

//     // Load test_session_state
//     test_case.load_test_session_state(&*fixture_context);

//     Ok(test_case)
// }
