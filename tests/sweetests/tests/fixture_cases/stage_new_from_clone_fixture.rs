use base_types::{MapInteger, MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_core::{dances::ResponseStatusCode, new_holon, HolonsContextBehavior, WritableHolon};
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

    let fixture_context = init_fixture_context().as_ref();

    let mut fixture_holons = FixtureHolons::new();

    // Assert DB starts with 1 (space Holon)
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // ── PHASE A — Clone FROM a fresh TRANSIENT ────────────────────────────────────
    const TRANSIENT_SOURCE_KEY: &str = "book:transient-source";
    let transient_source = new_holon(fixture_context, MapString::from(TRANSIENT_SOURCE_KEY))?
        .with_property_value(fixture_context, "TITLE", TRANSIENT_SOURCE_KEY)?
        .with_property_value(fixture_context, "TYPE", "Book")?;

    // Mint a transient-intent token and index it by key so we can refer to it later.
    let transient_source_token = fixture_holons
        .add_transient_with_key(&transient_source, MapString::from(TRANSIENT_SOURCE_KEY))?;

    // Stage a new holon cloned from that transient source (returns a staged-intent token).
    let clone_from_transient_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        transient_source_token.clone(),
        MapString::from("book:clone:from-transient"),
        ResponseStatusCode::OK,
    )?;

    // Mutate the staged clone (as separate step).
    let mut p = PropertyMap::new();
    p.insert("TITLE".to_property_name(), "Dune (Transient Clone)".to_base_value());
    p.insert("EDITION".to_property_name(), 1.to_base_value());
    test_case.add_with_properties_step(clone_from_transient_staged, p, ResponseStatusCode::OK)?;

    // ── PHASE B — Setup canonical holons, then clone FROM STAGED ──────────────────
    setup_book_author_steps_with_context(fixture_context, &mut test_case, &mut fixture_holons)?;

    // The helper staged the canonical Book and indexed it under BOOK_KEY.
    let book_staged_token = fixture_holons
        .get_by_key(&MapString::from(BOOK_KEY))
        .expect("BOOK_KEY token must exist after setup");

    let clone_from_staged_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        MapString::from("book:clone:from-staged"),
        ResponseStatusCode::OK,
    )?;

    let mut p2 = PropertyMap::new();
    p2.insert("TITLE".to_property_name(), "Dune (Staged Clone)".to_base_value());
    p2.insert("EDITION".to_property_name(), 2.to_base_value());
    test_case.add_with_properties_step(
        clone_from_staged_staged.clone(),
        p2,
        ResponseStatusCode::OK,
    )?;

    // Commit the first two staged clones and flip expectations in the fixture.
    test_case.add_commit_step(ResponseStatusCode::OK)?;
    fixture_holons.commit();
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // ── PHASE C — Clone FROM SAVED (same token, now expected Saved) ───────────────
    // At this point, BOOK_KEY’s token (and any staged tokens included in the commit)
    // have expected_state == Saved inside `fixture_holons`.
    let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token.clone(), // same token; now expected Saved by fixture intent
        MapString::from("book:clone:from-saved"),
        ResponseStatusCode::OK,
    )?;

    let mut p3 = PropertyMap::new();
    p3.insert("TITLE".to_property_name(), "Dune (Saved Clone)".to_base_value());
    p3.insert("EDITION".to_property_name(), 3.to_base_value());
    test_case.add_with_properties_step(
        clone_from_saved_staged.clone(),
        p3,
        ResponseStatusCode::OK,
    )?;

    // Commit the third staged clone; flip fixture expectations; assert counts again.
    test_case.add_commit_step(ResponseStatusCode::OK)?;
    fixture_holons.commit();
    test_case.add_ensure_database_count_step(MapInteger(fixture_holons.count_saved()))?;

    // Final saved-content match derived from fixture expectations.
    // (Executor will compare expected vs. actual for each expected-saved token.)
    test_case.add_match_saved_content_step(fixture_holons.expected_saved_references())?;

    Ok(test_case)
}
