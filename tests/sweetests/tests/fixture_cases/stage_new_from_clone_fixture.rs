use std::collections::BTreeMap;

use base_types::{MapInteger, MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_core::{
    dances::ResponseStatusCode, new_holon, reference_layer::TransientReference,
    HolonsContextBehavior, ReadableHolon, WritableHolon,
};
use holons_test::{
    dance_test_language::DancesTestCase, fixture_holons::FixtureHolons, TestReference,
};
use type_names::ToPropertyName;

use crate::{
    fixture_cases::setup_book_and_authors_fixture::*,
    helpers::{init_fixture_context, BOOK_KEY},
};

/// Demonstrates cloning a Book three ways using the new harness:
///   A) from a fresh **Transient** // Expected failure BadRequest
///   B) from the **Staged** Book produced by the setup helper
///   C) from the **Saved** Book (same token, after commit flip)
///
/// Strategy:
/// - All step inputs are TestReference tokens
/// - Commit is parameterless; after scheduling it, call `fixture_holons.commit()`
///   to flip staged→saved expectations on the fixture side.
/// - Counts and content assertions derive from `FixtureHolons`.
pub fn stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "stage_new_from_clone",
        "Clone from transient, staged, and saved; mutate staged clones; assert counts+content",
    );
    let fixture_context = init_fixture_context();
    let mut fixture_holons = FixtureHolons::new();

    // Assert DB starts with 1 (space Holon)
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // ── PHASE A — Clone FROM a fresh TRANSIENT ────────────────────────────────────
    let transient_source_key = MapString("book:transient-source".to_string());
    let transient_source = new_holon(fixture_context.as_ref(), Some(transient_source_key.clone()))?;
    // Mint transient source token
    let transient_token = fixture_holons.add_transient(transient_source);
    // Expect BadRequest
    test_case.add_stage_new_from_clone_step(
        &*fixture_context,
        &mut fixture_holons,
        transient_token,
        transient_source_key.clone(),
        ResponseStatusCode::BadRequest,
    )?;
    // TODO:  Find a better way to attempt a non-OK expected response for this step without minting a token and having to subtract from fixture holons saved count

    // ── PHASE B — Setup canonical holons, then clone FROM STAGED ──────────────────
    let fixture_tuple = setup_book_author_steps_with_context(
        &*fixture_context,
        &mut test_case,
        &mut fixture_holons,
    )?;

    let _relationship_name = fixture_tuple.0;
    let fixture_bindings = fixture_tuple.1;

    let book_key = MapString(BOOK_KEY.to_string());
    let from_staged_key = MapString("book:clone:from-staged".to_string());
    let book_staged_token = fixture_bindings.get_token(&MapString("Book".to_string())).expect("Expected setup fixure return_items to contain a staged-intent token associated with 'Book' label").clone();

    //  Stage New From Clone  //
    let clone_from_staged_staged = test_case.add_stage_new_from_clone_step(
        &*fixture_context,
        &mut fixture_holons,
        book_staged_token.clone(),
        from_staged_key.clone(),
        ResponseStatusCode::OK,
    )?;

    // Add Properties
    let mut phase_b_expected_properties = PropertyMap::new();
    phase_b_expected_properties
        .insert("Description".to_property_name(), "Cloning from staged".to_base_value());
    phase_b_expected_properties.insert("TITLE".to_property_name(), "Dune".to_base_value());
    phase_b_expected_properties.insert("EDITION".to_property_name(), 2.to_base_value());

    test_case.add_with_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        clone_from_staged_staged,
        phase_b_expected_properties,
        ResponseStatusCode::OK,
    )?;

    //  COMMIT - Round 1  //
    let saved_tokens = test_case.add_commit_step(
        &*fixture_context,
        &mut fixture_holons,
        ResponseStatusCode::OK,
    )?;
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved() - 1))?; // Subtract 1 to account for invalid attempt

    // ── PHASE C — Clone FROM SAVED  ───────────────
    // At this point, BOOK_KEY’s token (and any staged tokens included in the commit)
    // have expected_state == Saved inside `fixture_holons`.
    let from_saved_key = MapString("book:clone:from-saved".to_string());

    // Retrieve book saved-intent token
    let book_saved_token: TestReference = saved_tokens
        .iter()
        .filter(|t| {
            t.expected_content().essential_content(&*fixture_context).unwrap().key.unwrap()
                == book_key
        })
        .collect::<Vec<&TestReference>>()[0]
        .clone();

    //  Stage New From Clone  //
    let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
        &*fixture_context,
        &mut fixture_holons,
        book_saved_token,
        from_saved_key.clone(),
        ResponseStatusCode::OK,
    )?;

    //  Add properties  //
    let mut phase_c_expected_properties = PropertyMap::new();
    phase_c_expected_properties
        .insert("Description".to_property_name(), "Cloning from saved".to_base_value());
    phase_c_expected_properties
        .insert("TITLE".to_property_name(), "Saved Clone of Dune".to_base_value());
    phase_c_expected_properties.insert("EDITION".to_property_name(), 3.to_base_value());
    phase_c_expected_properties.insert("TYPE".to_property_name(), "Book Clone".to_base_value());

    test_case.add_with_properties_step(
        &*fixture_context,
        &mut fixture_holons,
        clone_from_saved_staged,
        phase_c_expected_properties,
        ResponseStatusCode::OK,
    )?;

    //  COMMIT - Round 2  //
    let _saved_tokens = test_case.add_commit_step(
        &*fixture_context,
        &mut fixture_holons,
        ResponseStatusCode::OK,
    )?;
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved() - 1))?; // Subtract 1 to account for invalid attempt

    //  MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
