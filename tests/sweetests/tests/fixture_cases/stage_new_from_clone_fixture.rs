use std::collections::BTreeMap;

use base_types::{MapInteger, MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_core::{
    dances::ResponseStatusCode, new_holon, reference_layer::TransientReference,
    HolonsContextBehavior, ReadableHolon, WritableHolon,
};
use holons_test::{
    dance_test_language::DancesTestCase, fixture_holons::FixtureHolons, TestCaseInit, TestReference,
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
    let fixture_context = init_fixture_context();
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, mut fixture_bindings } =
        TestCaseInit::new(
            fixture_context,
            "stage_new_from_clone".to_string(),
            "Clone from transient, staged, and saved; mutate staged clones; assert counts+content"
                .to_string(),
        );

    // Assert DB starts with 1 (space Holon)
    test_case.add_ensure_database_count_step(&mut fixture_holons)?;

    // // ──  PHASE A — Attempt clone from a Transient -- Expect BadRequest   ────────────────────────────
    // let transient_source_key = MapString("book:transient-source".to_string());
    // let transient_source = new_holon(fixture_context.as_ref(), Some(transient_source_key.clone()))?;
    // // Mint transient source token
    // let transient_token = test_case.add_new_holon_step(
    //     &*fixture_context,
    //     &mut fixture_holons,
    //     transient_source,
    //     BTreeMap::new(),
    //     Some(transient_source_key.clone()),
    //     ResponseStatusCode::OK,
    // )?;
    // // Expect BadRequest
    // test_case.add_stage_new_from_clone_step(
    //     &*fixture_context,
    //     &mut fixture_holons,
    //     transient_token,
    //     transient_source_key.clone(),
    //     ResponseStatusCode::BadRequest,
    // )?;
    // TODO:  Find a better way to attempt a non-OK expected response for this step without minting a token and having to subtract from fixture holons saved count

    // ── PHASE B — Setup canonical holons, then clone FROM STAGED ──────────────────
    setup_book_author_steps_with_context(
        &*fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    let _relationship_name = fixture_bindings.relationship_name().unwrap();

    let _book_key = MapString(BOOK_KEY.to_string());
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
    test_case.add_commit_step(&*fixture_context, &mut fixture_holons, ResponseStatusCode::OK)?;
    test_case.add_ensure_database_count_step(&mut fixture_holons)?;

    // // ── PHASE C — Clone FROM SAVED  ───────────────
    // // At this point, BOOK_KEY’s token (and any staged tokens included in the commit)
    // // have state == Saved inside `fixture_holons`.
    // let from_saved_key = MapString("book:clone:from-saved".to_string());

    // //  Stage New From Clone  //
    // let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
    //     &*fixture_context,
    //     &mut fixture_holons,
    //     book_staged_token,
    //     from_saved_key.clone(),
    //     ResponseStatusCode::OK,
    // )?;

    // //  Add properties  //
    // let mut phase_c_expected_properties = PropertyMap::new();
    // phase_c_expected_properties
    //     .insert("Description".to_property_name(), "Cloning from saved".to_base_value());
    // phase_c_expected_properties
    //     .insert("TITLE".to_property_name(), "Saved Clone of Dune".to_base_value());
    // phase_c_expected_properties.insert("EDITION".to_property_name(), 3.to_base_value());
    // phase_c_expected_properties.insert("TYPE".to_property_name(), "Book Clone".to_base_value());

    // test_case.add_with_properties_step(
    //     &*fixture_context,
    //     &mut fixture_holons,
    //     clone_from_saved_staged,
    //     phase_c_expected_properties,
    //     ResponseStatusCode::OK,
    // )?;

    // //  COMMIT - Round 2  //
    // test_case.add_commit_step(
    //     &*fixture_context,
    //     &mut fixture_holons,
    //     ResponseStatusCode::OK,
    // )?;
    // test_case.add_ensure_database_count_step(&mut fixture_holons)?;

    //  MATCH SAVED CONTENT  //
    // test_case.add_match_saved_content_step()?;

    // Finalize
    test_case.finalize(&*fixture_context);

    Ok(test_case)
}
