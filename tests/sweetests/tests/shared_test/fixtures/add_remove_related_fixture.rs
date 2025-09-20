// #![allow(dead_code)]

use pretty_assertions::assert_eq;
use tracing::{error, info};

use rstest::*;

use crate::shared_test::{
    setup_book_author_steps_with_context,
    test_add_related_holon::execute_add_related_holons,
    test_context::init_fixture_context,
    test_data_types::{DancesTestCase, BOOK_KEY},
};

use base_types::{MapBoolean, MapInteger, MapString};
use core_types::{BaseTypeKind, HolonError, HolonId};
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::Holon,
    dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression,
    reference_layer::{HolonReference, ReadableHolon, WritableHolon},
    HolonCollection, HolonCollectionApi, HolonsContextBehavior,
};
use integrity_core_types::{PropertyMap, PropertyName, PropertyValue, RelationshipName};

#[fixture]
pub fn simple_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    // Init
    let mut test_case = DancesTestCase::new(
        "Simple Add / Remove Related Holon Testcase".to_string(),
        "1) Ensure DB starts empty,\n\
         2) Stage Book, Person, Publisher Holons, \n\
         3) Add two Persons to Book's AUTHORED_BY relationship\n\
         4) (disabled) Try to remove related holons using invalid source holon\n\
         5) (disabled) Try to remove related holons using invalid relationship name\n\
         6) Remove 1 related holon\n\
         7) Test remove all related holons including ignoring a previous one that was already removed\n\
         8) Commit,\n\
         9) QueryRelationships.\n".to_string(),
    );

    // let _ = holochain_trace::test_run();

    let fixture_context = init_fixture_context();

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Use helper function to stage Book, 2 Person, 1 Publisher Holon and AUTHORED_BY relationship
    // from the book to the two persons
    let relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key

    info!("fixture: book and author setup complete.");

    // 4) (disabled) Try to remove related holons using invalid source holon
    // 5) (disabled) Try to remove related holons using invalid relationship name
    // 6) Remove 1 related holon

    // Retrieve the book from the context
    let book_holon_staged_reference =
        get_staged_holon_by_base_key(&*fixture_context, &MapString(BOOK_KEY.to_string()))?;

    // Get its current authors

    let authors_reference =
        book_holon_staged_reference.related_holons(&*fixture_context, &relationship_name)?;

    info!("authors retrieved for book: {:?}", authors_reference);

    let author_name_to_remove = MapString("George Smith".to_string());

    let maybe_author_to_remove = authors_reference.as_ref().get_by_key(&author_name_to_remove)?;

    info!("result of searching for George Smith authors: {:?}", maybe_author_to_remove);

    if let Some(author_to_remove) = maybe_author_to_remove {
        let mut remove_vector = Vec::new();
        remove_vector.push(author_to_remove);
        test_case.remove_related_holons_step(
            HolonReference::Staged(book_holon_staged_reference),
            relationship_name.clone(),
            remove_vector,
            ResponseStatusCode::OK,
        )?;
    } else {
        error!(
            "Could not find {} in related holons for {}",
            author_name_to_remove, relationship_name
        );
    }

    // test remove all related holons including ignoring a previous one that was already removed
    // test_case.remove_related_holons_step(
    //     staged_book_holon_reference, // source holon
    //     relationship_name.clone(),
    //     authors.to_vec(),
    //     ResponseStatusCode::OK,
    //     book_holon_with_no_related.clone(), //expected none
    // )?;
    //
    // test_case.add_related_holons_step(
    //     StagedReference::from_index(book_index), // source holon
    //     authored_by_relationship_name.clone(),
    //     related_holons.to_vec(),
    //     ResponseStatusCode::OK,
    //     book_holon.clone(),
    // )?;

    expected_count += staged_count(&*fixture_context);

    //  COMMIT  //
    test_case.add_commit_step()?;

    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  QUERY RELATIONSHIPS  //
    let query_expression = QueryExpression::new(relationship_name.clone());
    test_case.add_query_relationships_step(
        MapString(BOOK_KEY.to_string()),
        query_expression,
        ResponseStatusCode::OK,
    )?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case.clone())
}
