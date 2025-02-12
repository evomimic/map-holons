#![allow(dead_code)]

// use crate::get_holon_by_key_from_test_state;
use crate::tracing::{error, info, warn};
use core::panic;
use std::cell::RefCell;
//use holochain::core::author_key_is_valid;

use crate::shared_test::test_data_types::DancesTestCase;
use dances::dance_response::ResponseStatusCode;
use holons::reference_layer::HolonReference::Staged;
use holons::reference_layer::{HolonReference, StagedReference};
use holons_core::core_shared_objects::{Holon, HolonCollection, HolonError, RelationshipName};

use crate::shared_test::setup_book_author_steps_with_context;
use crate::shared_test::test_add_related_holon::execute_add_related_holons;
use holochain::prelude::dependencies::kitsune_p2p_types::dependencies::lair_keystore_api::config::get_server_pub_key_from_connection_url;
use holons_client::init_client_context;
use holons_core::*;
use holons_core::{stage_new_holon_api, HolonReadable, HolonWritable, HolonsContextBehavior};
use holons_guest::query_layer::QueryExpression;
use pretty_assertions::assert_eq;
use rstest::*;
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{
    HolonId, MapBoolean, MapInteger, MapString, PropertyMap, PropertyName, PropertyValue,
};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

#[fixture]
pub fn simple_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_client_context().as_ref().clone();
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

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

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Use helper function to stage Book, 2 Person, 1 Publisher Holon and AUTHORED_BY relationship
    // from the book to the two persons
    let relationship_name = setup_book_author_steps_with_context(fixture_context, &mut test_case)?;

    // 4) (disabled) Try to remove related holons using invalid source holon
    // 5) (disabled) Try to remove related holons using invalid relationship name
    // 6) Remove 1 related holon

    let book_key = MapString("Emerging World".to_string());

    // Retrieve the book from the context
    let staged_book_holon_ref = get_staged_holon_by_key(fixture_context, book_key.clone())?;

    // Get its current authors

    let authors_ref =
        staged_book_holon_ref.get_related_holons(fixture_context, &relationship_name)?;

    let author_name_to_remove = MapString("George Smith".to_string());

    let maybe_author_to_remove =
        staged_book_holon_ref.get_related_holons(fixture_context, &relationship_name)?;

    if let Some(author_to_remove) = maybe_author_to_remove {
        let mut remove_vector = Vec::new();
        remove_vector.push(author_to_remove);
        staged_book_holon_ref.remove_related_holons(
            fixture_context,
            &relationship_name,
            remove_vector,
        )?;
    } else {
        error!(
            "Could not find {} in related holons for {}",
            author_name_to_remove, relationship_name
        );
    }

    // // test remove all related holons including ignoring a previous one that was already removed
    // test_case.remove_related_holons_step(
    //     staged_book_holon_ref, // source holon
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

    expected_count += staging_service.borrow().staged_count();

    //  COMMIT  //
    test_case.add_commit_step()?;

    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  QUERY RELATIONSHIPS  //
    let query_expression = QueryExpression::new(relationship_name.clone());
    test_case.add_query_relationships_step(
        book_key.clone(),
        query_expression,
        ResponseStatusCode::OK,
    )?;

    Ok(test_case.clone())
}
