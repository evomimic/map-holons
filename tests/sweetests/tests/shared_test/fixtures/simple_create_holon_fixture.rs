// #![allow(dead_code)]

// use crate::get_holon_by_base_key_from_test_state;
use core::panic;
use std::cell::RefCell;
use tracing::{error, info, warn};
//use holochain::core::author_key_is_valid;

use crate::shared_test::setup_book_author_steps_with_context;
use crate::shared_test::test_context::init_test_context;
use crate::shared_test::test_context::TestContextConfigOption::TestFixture;
use crate::shared_test::test_data_types::DancesTestCase;
use holons_core::core_shared_objects::{Holon, HolonCollection, HolonError, RelationshipName};
use holons_core::dances::dance_response::ResponseStatusCode;
use holons_core::query_layer::QueryExpression;
use holons_core::{HolonsContextBehavior, StagedReference};
use pretty_assertions::assert_eq;
use rstest::*;
use base_types::{MapBoolean, MapInteger, MapString};
use core_types::HolonId;
use integrity_core_types::{PropertyMap, PropertyName, PropertyValue};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

/// This function creates a set of simple (undescribed) holons
///
#[fixture]
pub fn simple_create_holon_fixture() -> Result<DancesTestCase, HolonError> {
    // Test Holons are staged (but never committed) in the fixture_context's Nursery
    // This allows them to be assigned StagedReferences and also retrieved by either index or key
    let fixture_context = init_test_context(TestFixture);
    let staging_service = fixture_context.get_space_manager().get_staging_behavior_access();

    let mut test_case = DancesTestCase::new(
        "Simple Create/Get Holon Testcase".to_string(),
        "Ensure the holons and relationships setup by book and author setup helper commit successfully".to_string(),
    );

    // Set initial expected_database_count to 1 (to account for the HolonSpace Holon)
    let mut expected_count: i64 = 1;

    // Ensure DB count //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    // Use helper function to set up a book holon, 2 persons, a publisher, and a relationship from
    // the book to both persons. Note that this uses the fixture's Nursery as a place to hold the test data.
    // let desired_test_relationship = RelationshipName(MapString("AUTHORED_BY".to_string()));

    let _author_relationship_name =
        setup_book_author_steps_with_context(&*fixture_context, &mut test_case)?;

    //  COMMIT  // all Holons in staging_area
    test_case.add_commit_step()?;
    expected_count += staging_service.borrow().staged_count();

    //  ENSURE DATABASE COUNT //
    test_case.add_ensure_database_count_step(MapInteger(expected_count))?;

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    Ok(test_case.clone())
}
