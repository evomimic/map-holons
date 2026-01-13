// use holons_core::{core_shared_objects::WritableRelationship, CollectionState};
// use holons_test::{fixture_holons, DancesTestCase, FixtureHolons, TestReference};
// use pretty_assertions::assert_eq;
// use tracing::{error, info};

// use holons_prelude::prelude::*;
// use rstest::*;

// use crate::{
//     fixture_cases::setup_book_author_steps_with_context,
//     helpers::{
//         init_fixture_context, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY, PERSON_2_KEY,
//         PUBLISHER_KEY,
//     },
// };

// // TODO: enhance test capabilities, ie trying to remove related holons using invalid source and relationship name

// /// For both Transient and Staged references:
// /// adds a new relationship, removes an existing relationship, then adds another relationship again.
// ///
// ///
// #[fixture]
// pub fn simple_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
//     // Init
//     let mut test_case = DancesTestCase::new(
//         "Simple Add / Remove Related Holon Testcase".to_string(),
//         "1) Ensure DB starts empty,\n\
//          2) Stage Book, Person, Publisher Holons, \n\
//          3) Add two Persons to Book's AUTHORED_BY relationship\n\
//          4) (disabled) Try to remove related holons using invalid source holon\n\
//          5) (disabled) Try to remove related holons using invalid relationship name\n\
//          6) Remove 1 related holon\n\
//          7) Test remove all related holons including ignoring a previous one that was already removed\n\
//          8) Commit,\n\
//          9) QueryRelationships.\n".to_string(),
//     );

//     // let _ = holochain_trace::test_run();

//     let fixture_context = init_fixture_context();
//     let mut fixture_holons = FixtureHolons::new();

//     // Ensure DB count //
//     test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

//     // // Use helper function to stage Book, 2 Person, 1 Publisher Holon and AUTHORED_BY relationship
//     // // from the book to the two persons
//    // let fixture_tuple = setup_book_author_steps_with_context(
//    //     &*fixture_context,
//    //     &mut test_case,
//    //     &mut fixture_holons,
//    // )?;

//    // let relationship_name = fixture_tuple.0;
//    // let fixture_bindings = fixture_tuple.1;

//     // info!("fixture: book and author setup complete.");

//     // let book_transient_reference = fixture_holons.get_latest_by_key(&book_key).unwrap().transient();

//     // === TRANSIENT === //  Company -> HOST -> Website  //
//     let host_relationship = "HOST".to_relationship_name();
//     let company_key = MapString("COMPANY_KEY".to_string());
//     let website_key = MapString("WEBSITE_KEY".to_string());
//     // Create transient references
//     let company_transient_reference = new_holon(&*fixture_context, Some(company_key.clone()))?;
//     let website_transient_reference = new_holon(&*fixture_context, Some(website_key.clone()))?;
//     // -- ADD STEP -- //
//     // Set expected
//     let mut company_expected_content =
//         company_transient_reference.essential_content(&*fixture_context)?.clone();
//     company_expected_content
//         .relationships
//         .add_related_holons(
//             &*fixture_context,
//             CollectionState::Transient,
//             host_relationship.clone(),
//             vec![HolonReference::from(website_transient_reference.clone())],
//         )
//         .unwrap();
//     // Mint transient source tokens
//     let company_transient_token = fixture_holons.add_transient(
//         &company_transient_reference,
//         company_key.clone(),
//         &company_expected_content,
//     );
//     let website_transient_token = fixture_holons.add_transient(
//         &website_transient_reference,
//         website_key.clone(),
//         &website_transient_reference.essential_content(&*fixture_context)?,
//     );
//     // Add step
//     test_case.add_add_related_holons_step(
//         company_transient_token.clone(),
//         host_relationship.clone(),
//         vec![website_transient_token.clone()],
//         ResponseStatusCode::OK,
//     )?;

//     // -- REMOVE STEP -- //
//     // Set expected
//     company_expected_content
//         .relationships
//         .remove_related_holons(
//             &*fixture_context,
//             &host_relationship,
//             vec![HolonReference::from(website_transient_reference.clone())],
//         )
//         .unwrap();
//     // Mint transient token with the expected_content
//     let source_company_token = fixture_holons.add_transient(
//         &company_transient_reference,
//         company_key.clone(),
//         &company_expected_content,
//     );
//     // Executor step
//     test_case.add_remove_related_holons_step(
//         source_company_token,
//         host_relationship.clone(),
//         vec![website_transient_token.clone()],
//         ResponseStatusCode::OK,
//     )?;

//     // // -- Again ADD STEP -- //
//     // let again_relationship = "AGAIN".to_relationship_name();
//     // let example_key = MapString("EXAMPLE_KEY".to_string());
//     // let example_transient_reference = new_holon(&*fixture_context, Some(example_key.clone()))?;
//     // let example_transient_token = fixture_holons.add_transient(
//     //     &example_transient_reference,
//     //     example_key.clone(),
//     //     &example_transient_reference.essential_content(&*fixture_context)?,
//     // )?;
//     // // Set expected
//     // company_expected_content
//     //     .relationships
//     //     .add_related_holons(
//     //         &*fixture_context,
//     //         CollectionState::Transient,
//     //         again_relationship.clone(),
//     //         vec![HolonReference::from(example_transient_reference.clone())],
//     //     )
//     //     .unwrap();
//     // // Mint another snapshot
//     // let another_company_token = fixture_holons.add_transient(
//     //     &company_transient_reference,
//     //     company_key.clone(),
//     //     &company_expected_content,
//     // )?;
//     // // Add step
//     // test_case.add_add_related_holons_step(
//     //     company_transient_token.clone(),
//     //     again_relationship.clone(),
//     //     vec![example_transient_token.clone()],
//     //     ResponseStatusCode::OK,
//     // )?;

//     ////////
//     //
//     //
//     //  Old code:  this needs to be adjusted to fit test harness or be dropped
//     // PENDING: approach for:
//     // Testing relationships Staged -> Staged

//     // // Get book Staged token
//     // let book_key = MapString(BOOK_KEY.to_string());
//     // let book_staged_token = fixture_holons.get_latest_by_key(&book_key)?;

//     // // Get its current authors
//     // let authors_reference =
//     //     book_holon_staged_reference.related_holons(&*fixture_context, &relationship_name)?;

//     // // let authors =
//     // //     book_staged_token.token_id().relationship_map.get(&relationship_name).expect("No collection found for relationship_name");

//     // // debug!("authors retrieved for book: {:?}", authors_reference);
//     // let person_1_option =
//     //     authors_reference.read().unwrap().get_by_key(&MapString(PERSON_1_KEY.to_string()))?;
//     // let person_2_option =
//     //     authors_reference.read().unwrap().get_by_key(&MapString(PERSON_2_KEY.to_string()))?;
//     // //

//     // // REMOVE: both authors //

//     // if let Some(person_1) = person_1_option {
//     //     if let Some(person_2) = person_2_option {
//     //         let mut remove_vector = Vec::new();
//     //         remove_vector.push(person_1);
//     //         remove_vector.push(person_2);
//     //         // TestFixture
//     //         book_holon_staged_reference.remove_related_holons(
//     //             &*fixture_context,
//     //             BOOK_TO_PERSON_RELATIONSHIP,
//     //             remove_vector.clone(),
//     //         )?;
//     //         // Executor step
//     //         test_case.add_remove_related_holons_step(
//     //             &mut fixture_holons,
//     //             book_staged_token,
//     //             Some(book_key),
//     //             relationship_name.clone(),
//     //             remove_vector,
//     //             ResponseStatusCode::OK,
//     //         )?;
//     //     } else {
//     //         error!("Could not find {} in related holons for {}", PERSON_2_KEY, relationship_name);
//     //     }
//     // } else {
//     //     error!("Could not find {} in related holons for {}", PERSON_1_KEY, relationship_name);
//     // }
//     // // */
//     // // ADD: publisher //

//     // let publisher =
//     //     get_transient_holon_by_base_key(&*fixture_context, &MapString(PUBLISHER_KEY.to_string()))?;

//     // book_holon_staged_reference.add_related_holons(
//     //     &*fixture_context,
//     //     "PUBLISHED_BY",
//     //     vec![HolonReference::Transient(publisher.clone())],
//     // )?;

//     // test_case.add_add_related_holons_step(
//     //     HolonReference::Staged(book_holon_staged_reference.clone()),
//     //     "PUBLISHED_BY".to_relationship_name(),
//     //     vec![TestReference::TransientHolon(publisher)],
//     //     ResponseStatusCode::OK,
//     // )?;

//     // expected_count += staged_count(&*fixture_context).unwrap();

//     // //  COMMIT  //
//     // test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;

//     // test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

//     //  QUERY RELATIONSHIPS  //
//     let query_expression = QueryExpression::new(host_relationship.clone());
//     test_case.add_query_relationships_step(
//         company_transient_token,
//         query_expression,
//         ResponseStatusCode::OK,
//     )?;

//     // Load test_session_state
//     test_case.load_test_session_state(&*fixture_context);

//     Ok(test_case.clone())
// }
