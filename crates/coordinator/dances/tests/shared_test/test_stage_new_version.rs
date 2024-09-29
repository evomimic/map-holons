// use dances::dance_response::ResponseBody;
// use dances::dance_response::{DanceResponse, ResponseBody::Index, ResponseStatusCode};
// use dances::holon_dance_adapter::{
//     build_commit_dance_request, build_stage_new_version_dance_request,
// };
// use hdk::prelude::*;
// use holochain::sweettest::*;
// use holochain::sweettest::{SweetCell, SweetConductor};
// use holons::holon::{self, Holon};
// use holons::holon_reference::HolonReference;
// use holons::smart_reference::SmartReference;
// use holons::staged_reference::StagedReference;
// use rstest::*;
// use shared_types_holon::{BaseValue, HolonId, MapString, PropertyName};

// use super::data_types::DanceTestState;

// /// This function builds and dances a `stage_new_version` DanceRequest for the supplied Holon
// /// and confirms a Success response
// ///
// pub async fn execute_stage_new_version(
//     conductor: &SweetConductor,
//     cell: &SweetCell,
//     test_state: &mut DanceTestState,
//     original_holon: Holon,
//     expected_response: ResponseStatusCode,
//     expected_holon: Holon,
// ) -> () {
//     info!("\n\n--- TEST STEP: Stage_New_Version ---- :");

//     // Build a Commit DanceRequest
//     let commit_request = build_commit_dance_request(test_state.staging_area.clone());
//     debug!("COMMIT Dance Request: {:#?}", commit_request);

//     // COMMIT //
//     match commit_request {
//         Ok(valid_request) => {
//             let response: DanceResponse = conductor
//                 .call(&cell.zome("dances"), "dance", valid_request)
//                 .await;

//             debug!("Dance Response: {:#?}", response.clone());
//             test_state.staging_area = response.staging_area.clone();
//             let code = response.status_code;
//             let description = response.description.clone();
//             if code == ResponseStatusCode::OK {
//                 // Check that staging area is empty
//                 assert!(response.staging_area.staged_holons.is_empty());

//                 info!("Success! Commit succeeded");

//                 // get saved holons out of response body and add them to the test_state created holons
//                 match response.body {
//                     ResponseBody::Holon(holon) => {
//                         test_state.created_holons.push(holon);
//                     }
//                     ResponseBody::Holons(holons) => {
//                         for holon in holons {
//                             test_state.created_holons.push(holon);
//                         }
//                     }
//                     _ => panic!("Invalid ResponseBody: {:?}", response.body),
//                 }
//             } else {
//                 panic!("DanceRequest returned {code} for {description}");
//             }
//         }
//         Err(error) => {
//             panic!("{:?} Unable to build a commit request ", error);
//         }
//     }

//     // Book Holon data
//     let book_key = original_holon
//         .get_key()
//         .unwrap()
//         .expect("the original Book Holon should have key!");
//     let created_book_holon: Holon = test_state.created_holons[0].clone();
//     // get id and smart_reference
//     let book_id = created_book_holon.get_local_id().unwrap();
//     let book_holon_smart_reference = SmartReference::new(
//         HolonId::Local(book_id),
//         Some(created_book_holon.property_map.clone()),
//     );

//     // STAGE_NEW_VERSION //
//     // Build a stage_new_version DanceRequest
//     let request =
//         build_stage_new_version_dance_request(test_state.staging_area.clone(), created_book_holon);
//     debug!("STAGE_NEW_VERSION Dance Request: {:#?}", request);
//     match request {
//         Ok(valid_request) => {
//             let response: DanceResponse = conductor
//                 .call(&cell.zome("dances"), "dance", valid_request)
//                 .await;
//             debug!("Dance Response: {:#?}", response.clone());
//             test_state.staging_area = response.staging_area.clone();
//             let code = response.status_code;
//             assert_eq!(code.clone(), expected_response);
//             let description = response.description.clone();

//             if let ResponseStatusCode::OK = code {
//                 if let Index(index) = response.body {
//                     let index_value = index.to_string();
//                     debug!("{index_value} returned in body");
//                     // An index was returned in the body, retrieve the Holon at that index within
//                     // the StagingArea and confirm it matches the expected Holon.

//                     let holons = response.staging_area.staged_holons;

//                     debug!("holons:{:#?}", holons);
//                     assert_eq!(
//                         expected_holon.essential_content(),
//                         holons[index].essential_content(),
//                     );
//                     info!("Success! DB fetched holon matched expected");
//                 } else {
//                     panic!("Expected `index` to staged_holon in the response body, but didn't get one!");
//                 }
//             } else {
//                 panic!("DanceRequest returned {code} for {description}");
//             }
//         }
//         Err(error) => {
//             panic!("{:?} Unable to build a new_version request ", error);
//         }
//     }

//     assert_eq!(test_state.staging_area.staged_holons.len(), 1); // there should only be the new Book Holon in staging_area, since the others were commited in the previous request
//     let mut new_cloned_book: Holon = test_state.staging_area.staged_holons.pop().unwrap();
//     let cloned_book_key =
//         BaseValue::StringValue(MapString("A new version of: Emerging World".to_string()));

//     //  CHANGE PROPERTIES  //
//     new_cloned_book
//         .with_property_value(
//             PropertyName(MapString("title".to_string())),
//             cloned_book_key.clone(),
//         )
//         .unwrap();
//     new_cloned_book
//         .with_property_value(
//             PropertyName(MapString("key".to_string())),
//             cloned_book_key.clone(),
//         )
//         .unwrap();
//     new_cloned_book
//         .with_property_value(
//             PropertyName(MapString("description".to_string())),
//             BaseValue::StringValue(MapString(
//                 "example property change for a new version from staged Holon".to_string(),
//             )),
//         )
//         .unwrap();
//     test_state.staging_area.staged_holons.push(new_cloned_book);

//     //  New COMMIT  //
//     // Build a new Commit DanceRequest
//     let new_commit_request = build_commit_dance_request(test_state.staging_area.clone());
//     debug!("COMMIT Dance Request: {:#?}", new_commit_request);

//     match new_commit_request {
//         Ok(valid_request) => {
//             let response: DanceResponse = conductor
//                 .call(&cell.zome("dances"), "dance", valid_request)
//                 .await;

//             debug!("Dance Response: {:#?}", response.clone());
//             test_state.staging_area = response.staging_area.clone();
//             let code = response.status_code;
//             let description = response.description.clone();
//             if code == ResponseStatusCode::OK {
//                 // Check that staging area is empty
//                 assert!(response.staging_area.staged_holons.is_empty());

//                 info!("Success! Commit succeeded");

//                 // get saved holons out of response body and add them to the test_state created holons
//                 match response.body {
//                     ResponseBody::Holon(holon) => {
//                         test_state.created_holons.push(holon);
//                     }
//                     ResponseBody::Holons(holons) => {
//                         for holon in holons {
//                             test_state.created_holons.push(holon);
//                         }
//                     }
//                     _ => panic!("Invalid ResponseBody: {:?}", response.body),
//                 }
//             } else {
//                 panic!("DanceRequest returned {code} for {description}");
//             }
//         }
//         Err(error) => {
//             panic!("{:?} Unable to build a commit request ", error);
//         }
//     }
// }
