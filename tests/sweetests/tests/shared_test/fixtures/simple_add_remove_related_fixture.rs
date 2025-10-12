#[allow(unused_must_use)]
use pretty_assertions::assert_eq;
use tracing::{error, info};

use crate::shared_test::{
    setup_book_author_steps_with_context,
    test_add_related_holon::execute_add_related_holons,
    test_context::init_fixture_context,
    test_data_types::{
        DancesTestCase, TestReference, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY,
        PERSON_2_KEY, PUBLISHER_KEY,
    },
};
use base_types::{MapBoolean, MapInteger, MapString};
use core_types::{BaseTypeKind, HolonError, HolonId};
use core_types::{PropertyMap, PropertyName, PropertyValue, RelationshipName};
use holons_core::reference_layer::holon_operations_api::*;
use holons_core::{
    core_shared_objects::Holon,
    dances::dance_response::ResponseStatusCode,
    query_layer::QueryExpression,
    reference_layer::{HolonReference, ReadableHolon, WritableHolon},
    HolonCollection, HolonCollectionApi, HolonsContextBehavior,
};
use holons_prelude::prelude::*;
use rstest::*;
use type_names::ToRelationshipName;

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

    //TODO:
    // 4) (disabled) Try to remove related holons using invalid source holon
    // 5) (disabled) Try to remove related holons using invalid relationship name

    // Retrieve the book from the context
    let mut book_holon_staged_reference =
        get_staged_holon_by_base_key(&*fixture_context, &MapString(BOOK_KEY.to_string()))?;

    // Get its current authors
    let authors_reference =
        book_holon_staged_reference.related_holons(&*fixture_context, &relationship_name)?;

    // debug!("authors retrieved for book: {:?}", authors_reference);
    let person_1_option =
        authors_reference.read().unwrap().get_by_key(&MapString(PERSON_1_KEY.to_string()))?;
    let person_2_option =
        authors_reference.read().unwrap().get_by_key(&MapString(PERSON_2_KEY.to_string()))?;

    // REMOVE: both authors //

    if let Some(person_1) = person_1_option {
        if let Some(person_2) = person_2_option {
            let mut remove_vector = Vec::new();
            remove_vector.push(person_1);
            remove_vector.push(person_2);
            // TestFixture
            book_holon_staged_reference.remove_related_holons(
                &*fixture_context,
                BOOK_TO_PERSON_RELATIONSHIP,
                remove_vector.clone(),
            )?;
            // Executor step
            test_case.add_remove_related_holons_step(
                HolonReference::Staged(book_holon_staged_reference.clone()),
                relationship_name.clone(),
                remove_vector,
                ResponseStatusCode::OK,
            )?;
        } else {
            error!("Could not find {} in related holons for {}", PERSON_2_KEY, relationship_name);
        }
    } else {
        error!("Could not find {} in related holons for {}", PERSON_1_KEY, relationship_name);
    }

    // ADD: publisher //

    let publisher =
        get_transient_holon_by_base_key(&*fixture_context, &MapString(PUBLISHER_KEY.to_string()))?;

    book_holon_staged_reference.add_related_holons(
        &*fixture_context,
        "PUBLISHED_BY",
        vec![HolonReference::Transient(publisher.clone())],
    )?;

    test_case.add_add_related_holons_step(
        HolonReference::Staged(book_holon_staged_reference.clone()),
        "PUBLISHED_BY".to_relationship_name(),
        vec![TestReference::TransientHolon(publisher)],
        ResponseStatusCode::OK,
        HolonReference::Staged(book_holon_staged_reference),
    )?;

    expected_count += staged_count(&*fixture_context).unwrap();

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
