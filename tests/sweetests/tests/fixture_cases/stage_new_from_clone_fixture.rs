use std::collections::BTreeMap;

use base_types::{MapInteger, MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_core::{
    dances::ResponseStatusCode, new_holon, HolonsContextBehavior, ReadableHolon, WritableHolon,
};
use holons_test::{fixture_holons::FixtureHolons, test_case::DancesTestCase};
use type_names::ToPropertyName;

use crate::{
    fixture_cases::setup_book_and_authors_fixture::*,
    helpers::{init_fixture_context, BOOK_KEY},
};

/// Demonstrates cloning a Book three ways using the new harness:
///   A) from a fresh **Transient**
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

    // // ── PHASE A — Clone FROM a fresh TRANSIENT ────────────────────────────────────
    const TRANSIENT_SOURCE_KEY: &str = "book:transient-source";
    let mut transient_source =
        new_holon(fixture_context.as_ref(), Some(MapString::from(TRANSIENT_SOURCE_KEY)))?;
    transient_source
        .with_property_value(fixture_context.as_ref(), "TITLE", TRANSIENT_SOURCE_KEY)?
        .with_property_value(fixture_context.as_ref(), "TYPE", "Book")?;
    // Set expected
    let mut expected_clone_properties = PropertyMap::new();
    expected_clone_properties
        .insert("Key".to_property_name(), "book:clone:from-transient".to_base_value());
    expected_clone_properties
        .insert("TITLE".to_property_name(), TRANSIENT_SOURCE_KEY.to_base_value());
    expected_clone_properties.insert("TYPE".to_property_name(), "Book".to_base_value());

    // TODO: expected response ResponseStatuseCode::BadRequest

    // let clone_from_transient_staged = test_case.add_stage_new_from_clone_step(
    //     &mut fixture_holons,
    //     transient_source_token.clone(),
    //     MapString::from("book:clone:from-transient"),
    //     ResponseStatusCode::OK,
    // )?;

    // ── PHASE B — Setup canonical holons, then clone FROM STAGED ──────────────────
    setup_book_author_steps_with_context(
        fixture_context.as_ref(),
        &mut test_case,
        &mut fixture_holons,
    )?;
    // Set expected
    expected_clone_properties
        .insert("Key".to_property_name(), "book:clone:from-staged".to_base_value());
    let mut book_staged_clone_expected_content =
        transient_source.essential_content(&*fixture_context)?;
    book_staged_clone_expected_content.property_map = expected_clone_properties.clone();
    book_staged_clone_expected_content.key = Some(MapString("book:clone:from-staged".to_string()));
    // Mint a staged-intent token
    let book_staged_token = fixture_holons.add_staged_with_key(
        &transient_source,
        MapString::from("book:clone:from-staged"),
        &book_staged_clone_expected_content,
    )?;
    // Stage
    let clone_from_staged_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        MapString::from("book:clone:from-staged"),
        ResponseStatusCode::OK,
    )?;

    // // // let mut properties2 = PropertyMap::new();
    // // // properties2.insert("TITLE".to_property_name(), "Dune (Staged Clone)".to_base_value());
    // // // properties2.insert("EDITION".to_property_name(), 2.to_base_value());
    // // // test_case.add_with_properties_step(
    // // //     clone_from_staged_staged.clone(),
    // // //     properties2,
    // // //     ResponseStatusCode::OK,
    // // // )?;

    // // COMMIT - the first two staged clones and flip expectations in the fixture.
    // let _saved_holons = test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    // test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // // ── PHASE C — Clone FROM SAVED (same token, now expected Saved) ───────────────
    // // At this point, BOOK_KEY’s token (and any staged tokens included in the commit)
    // // have expected_state == Saved inside `fixture_holons`.
    // // Set expected
    // expected_clone_properties
    //     .insert("Key".to_property_name(), "book:clone:from-saved".to_base_value());
    // let mut book_saved_clone_expected_content =
    //     transient_source.essential_content(&*fixture_context)?;
    // book_saved_clone_expected_content.property_map = expected_clone_properties.clone();
    // book_saved_clone_expected_content.key = Some(MapString("book:clone:from-saved".to_string()));
    // // Mint a saved-intent token
    // let book_saved_token = fixture_holons.add_staged_with_key(
    //     &transient_source,
    //     MapString::from(TRANSIENT_SOURCE_KEY),
    //     &book_saved_clone_expected_content,
    // )?;
    // // Stage
    // let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
    //     &mut fixture_holons,
    //     book_saved_token.clone(), // same token; now expected Saved by fixture intent
    //     MapString::from("book:clone:from-saved"),
    //     ResponseStatusCode::OK,
    // )?;

    // let mut properties3 = PropertyMap::new();
    // properties3.insert("TITLE".to_property_name(), "Dune (Saved Clone)".to_base_value());
    // properties3.insert("EDITION".to_property_name(), 3.to_base_value());
    // test_case.add_with_properties_step(
    //     clone_from_saved_staged.clone(),
    //     properties3,
    //     ResponseStatusCode::OK,
    // )?;

    // COMMIT - the third staged clone; flip fixture expectations; assert counts again.
    let _saved_holons = test_case.add_commit_step(&mut fixture_holons, ResponseStatusCode::OK)?;
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // Final saved-content match derived from fixture expectations.
    // (Executor will compare expected vs. actual for each expected-saved token.)
    test_case.add_match_saved_content_step()?;

    // Load test_session_state
    test_case.load_test_session_state(&*fixture_context);

    Ok(test_case)
}
