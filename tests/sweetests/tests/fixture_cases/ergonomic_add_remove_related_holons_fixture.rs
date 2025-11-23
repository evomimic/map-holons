// use holons_test::DancesTestCase;
// use tracing::{error, info};

// use holons_prelude::prelude::*;
// use rstest::*;
// use type_names::{CoreRelationshipTypeName, ToRelationshipName};

// use crate::helpers::{
//     init_fixture_context, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, EDITOR_FOR, PERSON_1_KEY,
//     PERSON_2_KEY, PUBLISHER_KEY,
// };

// #[fixture]
// pub fn ergonomic_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
//     // Init
//     let mut test_case = DancesTestCase::new(
//         "Ergonomic Add / Remove Related Holons Testcase".to_string(),
//         "Tests the adding and removing of related Holons, using all combinations of ergonomic relationship names, for both Transient & Staged Holons".to_string(),
//     );

//     // let _ = holochain_trace::test_run();

//     let fixture_context = init_fixture_context();

//     // Use helper function to stage Book, 2 Person, 1 Publisher Holon and AUTHORED_BY relationship
//     // from the book to the two persons
//     let book_to_person_relationship_name =
//         setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

//     let mut book_staged_reference =
//         get_staged_holon_by_base_key(&*fixture_context, &MapString(BOOK_KEY.to_string()))?;

//     let mut person_1_staged_reference =
//         get_staged_holon_by_base_key(&*fixture_context, &MapString(PERSON_1_KEY.to_string()))?;

//     let mut person_2_staged_reference =
//         get_staged_holon_by_base_key(&*fixture_context, &MapString(PERSON_2_KEY.to_string()))?;

//     let mut publisher_transient_reference =
//         get_transient_holon_by_base_key(&*fixture_context, &MapString(PUBLISHER_KEY.to_string()))?;

//     let mut publisher_staged_reference =
//         get_staged_holon_by_base_key(&*fixture_context, &MapString(PUBLISHER_KEY.to_string()))?;

//     // ADD & REMOVE STEPS //

//     // Book
//     book_staged_reference
//         .add_related_holons(
//             &*fixture_context,
//             "PUBLISHED_BY".to_string(),
//             vec![HolonReference::Staged(publisher_staged_reference.clone())],
//         )?
//         .remove_related_holons(
//             &*fixture_context,
//             BOOK_TO_PERSON_RELATIONSHIP.to_string(),
//             vec![HolonReference::Staged(person_2_staged_reference.clone())],
//         )?
//         .remove_related_holons(
//             &*fixture_context,
//             MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string()),
//             vec![HolonReference::Staged(person_2_staged_reference.clone())],
//         )?;

//     test_case.add_add_related_holons_step(
//         HolonReference::Staged(book_staged_reference.clone()),
//         "PUBLISHED_BY".to_relationship_name(),
//         vec![TestReference::TransientHolon(publisher_transient_reference.clone())],
//         ResponseStatusCode::OK,
//         HolonReference::Staged(book_staged_reference.clone()),
//     )?;

//     test_case.add_remove_related_holons_step(
//         HolonReference::Staged(book_staged_reference.clone()),
//         book_to_person_relationship_name,
//         vec![HolonReference::Staged(person_1_staged_reference.clone())],
//         ResponseStatusCode::OK,
//     )?;

//     // Person 1
//     person_1_staged_reference.add_related_holons(
//         &*fixture_context,
//         EDITOR_FOR,
//         vec![HolonReference::Staged(book_staged_reference.clone())],
//     )?;

//     test_case.add_add_related_holons_step(
//         HolonReference::Staged(person_1_staged_reference.clone()),
//         "EDITOR_FOR".to_relationship_name(),
//         vec![TestReference::StagedHolon(book_staged_reference.clone())],
//         ResponseStatusCode::OK,
//         HolonReference::Staged(person_1_staged_reference),
//     )?;

//     // Person 2
//     person_2_staged_reference.add_related_holons(
//         &*fixture_context,
//         MapString(EDITOR_FOR.to_string()),
//         vec![HolonReference::Staged(book_staged_reference.clone())],
//     )?;

//     test_case.add_add_related_holons_step(
//         HolonReference::Staged(person_2_staged_reference.clone()),
//         MapString("EDITOR_FOR".to_string()).to_relationship_name(),
//         vec![TestReference::StagedHolon(book_staged_reference.clone())],
//         ResponseStatusCode::OK,
//         HolonReference::Staged(person_2_staged_reference),
//     )?;

//     // Publisher
//     publisher_staged_reference.add_related_holons(
//         &*fixture_context,
//         CoreRelationshipTypeName::SourceOf, // Arbitrary relationship
//         vec![HolonReference::Staged(book_staged_reference.clone())],
//     )?;

//     test_case.add_add_related_holons_step(
//         HolonReference::Staged(publisher_staged_reference.clone()),
//         "EDITOR_FOR".to_relationship_name(),
//         vec![TestReference::StagedHolon(book_staged_reference.clone())],
//         ResponseStatusCode::OK,
//         HolonReference::Staged(publisher_staged_reference),
//     )?;

//     publisher_transient_reference.add_related_holons(
//         &*fixture_context,
//         "Relationship".to_string().to_relationship_name(),
//         vec![HolonReference::Staged(book_staged_reference.clone())],
//     )?;

//     test_case.add_add_related_holons_step(
//         HolonReference::Transient(publisher_transient_reference.clone()),
//         RelationshipName(MapString("Relationship".to_string())).to_relationship_name(),
//         vec![TestReference::StagedHolon(book_staged_reference.clone())],
//         ResponseStatusCode::OK,
//         HolonReference::Transient(publisher_transient_reference.clone()),
//     )?;

//     // Load test_session_state
//     test_case.load_test_session_state(&*fixture_context);

//     Ok(test_case.clone())
// }
