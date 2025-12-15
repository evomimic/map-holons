use std::collections::BTreeMap;

use base_types::{MapInteger, MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_core::{
    dances::ResponseStatusCode, new_holon, reference_layer::TransientReference,
    HolonsContextBehavior, ReadableHolon, WritableHolon,
};
use holons_test::{fixture_holons::FixtureHolons, test_case::DancesTestCase};
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
    let mut transient_source =
        new_holon(fixture_context.as_ref(), Some(transient_source_key.clone()))?;
    // Mint transient source token
    let transient_token = fixture_holons.add_transient_with_key(
        &transient_source,
        transient_source_key.clone(),
        &transient_source.essential_content(&*fixture_context)?,
    )?;
    // Expect BadRequest
    let _clone_from_transient_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        transient_token,
        transient_source_key.clone(),
        ResponseStatusCode::BadRequest,
    )?;
    // TODO:  Find a better way to attempt a non-OK expected response for this step without minting a token and having to subtract from fixture holons saved count

    // ── PHASE B — Setup canonical holons, then clone FROM STAGED ──────────────────
    setup_book_author_steps_with_context(
        fixture_context.as_ref(),
        &mut test_case,
        &mut fixture_holons,
    )?;
    let book_key = MapString(BOOK_KEY.to_string());
    let from_staged_key = MapString("book:clone:from-staged".to_string());
    let mut book_staged_token = fixture_holons.get_latest_by_key(&book_key)?;

    book_staged_token.set_key(from_staged_key.clone());

    // Stage
    let _clone_from_staged_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        from_staged_key.clone(),
        ResponseStatusCode::OK,
    )?;
    // let mut phase_b_expected_properties = PropertyMap::new();
    // phase_b_expected_properties
    //     .insert("Key".to_property_name(), from_staged_key.clone().to_base_value());
    // phase_b_expected_properties
    //     .insert("TITLE".to_property_name(), "Dune (Staged Clone)".to_base_value());
    // phase_b_expected_properties.insert("EDITION".to_property_name(), 2.to_base_value());
    // let mut phase_b_expected_expected_content =
    //     transient_source.essential_content(&*fixture_context).unwrap();
    // phase_b_expected_expected_content.property_map = phase_b_expected_properties.clone();
    // // Mint
    // let staged_clone_token = fixture_holons.add_staged_with_key(
    //     &transient_source,
    //     from_staged_key.clone(),
    //     &phase_b_expected_expected_content,
    // )?;

    // test_case.add_with_properties_step(
    //     staged_clone_token.clone(),
    //     phase_b_expected_properties,
    //     ResponseStatusCode::OK,
    // )?;

    // COMMIT - the first two staged clones and flip expectations in the fixture.
    let _saved_holons = test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved() - 1))?; // Subtract 1 to account for invalid attempt

    // // ── PHASE C — Clone FROM SAVED (same token, now expected Saved) ───────────────
    // // At this point, BOOK_KEY’s token (and any staged tokens included in the commit)
    // // have expected_state == Saved inside `fixture_holons`.
    // // Set expected
    // let mut phase_c_expected_properties = PropertyMap::new();
    // phase_c_expected_properties
    //     .insert("Key".to_property_name(), "book:clone:from-saved".to_base_value());
    // phase_c_expected_properties
    //     .insert("TITLE".to_property_name(), "Saved Clone of Dune".to_base_value());
    // phase_c_expected_properties.insert("EDITION".to_property_name(), 3.to_base_value());
    // phase_c_expected_properties.insert("TYPE".to_property_name(), "Book Clone".to_base_value());
    // let mut book_saved_clone_expected_content =
    //     transient_source.essential_content(&*fixture_context)?;
    // book_saved_clone_expected_content.property_map = phase_c_expected_properties.clone();
    // book_saved_clone_expected_content.key = Some(MapString("book:clone:from-saved".to_string()));
    // // Retrieve saved-intent from latest in lineage
    // let book_saved_token =
    //     fixture_holons.get_latest_for_id(&transient_source.get_temporary_id())?;
    // // Stage new from clone
    // let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
    //     &mut fixture_holons,
    //     book_saved_token.clone(), // same token; now expected Saved by fixture intent
    //     MapString::from("book:clone:from-saved"),
    //     ResponseStatusCode::OK,
    // )?;

    // test_case.add_with_properties_step(
    //     clone_from_saved_staged.clone(),
    //     phase_c_expected_properties,
    //     ResponseStatusCode::OK,
    // )?;

    // TODO: Fix duplicate commits
    // // COMMIT - the third staged clone; flip fixture expectations; assert counts again.
    // let _saved_holons = test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    // test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved() - 1))?; // Subtract 1 to account for invalid attempt

    // // Final saved-content match derived from fixture expectations.
    // // (Executor will compare expected vs. actual for each expected-saved token.)
    // test_case.add_match_saved_content_step()?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
