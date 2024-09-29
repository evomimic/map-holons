// use std::collections::BTreeMap;

// use dances::{dance_response::ResponseStatusCode, holon_dance_adapter::QueryExpression};
// use hdi::prelude::warn;
// use holons::{
//     holon::Holon,
//     holon_collection::HolonCollection,
//     holon_error::HolonError,
//     holon_reference::HolonReference,
//     relationship::{self, RelationshipName},
//     smart_reference::SmartReference,
//     staged_reference::StagedReference,
// };
// use rstest::*;
// use shared_types_holon::{BaseValue, HolonId, MapInteger, MapString, PropertyMap, PropertyName};

// use crate::data_types::DancesTestCase;

// use super::book_authors_setup_fixture::setup_book_author_steps;

// /// Fixture for creating Simple NEWVERSION Testcase
// #[fixture]
// pub fn simple_stage_new_version_fixture() -> Result<DancesTestCase, HolonError> {
//     let mut test_case = DancesTestCase::new(
//         "Simple StageNewFromClone Testcase".to_string(),
//         "Tests stage_new_from_clone dance, creates and commits a holon, clones it, changes some properties, adds and removes some relationships, commits it and then compares essential content of existing holon and cloned holon".to_string(),
//     );

//     //  ENSURE DATABASE COUNT -- Empty  //
//     test_case.add_ensure_database_count_step(MapInteger(0))?;

//     let mut holons_to_add: Vec<HolonReference> = Vec::new();

//     // Use helper function to set up a book holon, 2 persons, and an AUTHORED_BY relationship from
//     // the book to both persons.
//     let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));

//     let test_data = setup_book_author_steps(
//         &mut test_case,
//         &mut holons_to_add,
//         &desired_test_relationship,
//     )?;

//     let book_holon = test_data[0]
//         .expected_holon
//         .clone()
//         .expect("Expected setup method to return Some book holon at index 0, got none.");
//     let book_index = test_data[0].staged_index;
//     let book_key = test_data[0].key.clone();
//     let book_holon_reference = HolonReference::Staged(StagedReference::new(book_index.clone()));

//     let person_1_index = test_data[1].staged_index;
//     let person_1_key = test_data[1].key.clone();
//     let person_1_holon_reference =
//         HolonReference::Staged(StagedReference::new(person_1_index.clone()));

//     let person_2_index = test_data[2].staged_index;
//     let person_2_key = test_data[2].key.clone();
//     let person_2_holon_reference =
//         HolonReference::Staged(StagedReference::new(person_2_index.clone()));

//     //  NEW_VERSION -- SmartReference -- Book Holon Clone  //
//     // set expected
//     let cloned_book_index = 4;
//     let cloned_book_key =
//         BaseValue::StringValue(MapString("A new version of: Emerging World".to_string()));

//     // test_case.add_stage_new_version_step(book_holon, ResponseStatusCode::OK)?;

//     //  CHANGE PROPERTIES  //
//     let changed_properties = BTreeMap::new();
//     changed_properties.insert(
//         PropertyName(MapString("title".to_string())),
//         cloned_book_key.clone(),
//     )?;
//     changed_properties.insert(
//         PropertyName(MapString("key".to_string())),
//         cloned_book_key.clone(),
//     )?;
//     changed_properties.insert(
//         PropertyName(MapString("description".to_string())),
//         BaseValue::StringValue(MapString(
//             "example property change for a new version from staged Holon".to_string(),
//         )),
//     )?;

//     Ok(test_case.clone())
// }
