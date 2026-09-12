use super::described_instances::add_described_instance;
use base_types::{MapString, ToBaseValue};
use core_types::{HolonError, PropertyMap};
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use std::collections::BTreeMap;
use type_names::ToPropertyName;

/// Demonstrates cloning a Book three ways using the new harness:
///   A) from a fresh **Transient** // Expected failure BadRequest
///   B) from a described **Staged** Book
///   C) from the **Saved** Book (same token, after commit flip)
///
/// Strategy:
/// - All step inputs are TestReference tokens
/// - The Commit adder advances fixture heads; callers keep using their existing tokens.
/// - Content assertions derive from `FixtureHolons`.
pub fn stage_new_from_clone_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, fixture_bindings: _ } =
        TestCaseInit::new(
            "stage_new_from_clone",
            "Clone from transient, staged, and saved; mutate staged clones; assert counts+content",
        );

    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;

    // ──  PHASE A — Attempt clone from a Transient -- Expect BadRequest   ────────────────────────────
    let transient_source_key = MapString("Book.StageNewFromClone.TransientSource".to_string());
    let transient_source =
        fixture_context.mutation().new_holon(Some(transient_source_key.clone()))?;
    // Mint transient source token
    let transient_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        transient_source,
        BTreeMap::new(),
        Some(transient_source_key.clone()),
        None,
        Some("Creating transient holon for BadRequest attempt.".to_string()),
    )?;
    // Expect BadRequest
    test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        transient_token,
        transient_source_key.clone(),
        Some(HolonErrorKind::InvalidHolonReference),
        Some("Attempting Stage New From Clone for BadRequest (Transient)".to_string()),
    )?;
    // TODO:  Find a better way to attempt a non-OK expected response for this step without minting a token and having to subtract from fixture holons saved count

    // Phase B: cloning retains the source Book's descriptor.
    let book_staged_token = add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.StageNewFromClone.Source",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;
    let from_staged_key = MapString("Book.StageNewFromClone.FromStaged".to_string());

    //  Stage New From Clone  //
    let clone_from_staged_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token.clone(),
        from_staged_key.clone(),
        None,
        Some("Stage New From Clone -- clone from staged book.".to_string()),
    )?;

    // Add Properties
    let mut phase_b_expected_properties = PropertyMap::new();
    phase_b_expected_properties.insert("TITLE".to_property_name(), "Dune".to_base_value());

    test_case.add_with_properties_step(
        &mut fixture_holons,
        clone_from_staged_staged,
        phase_b_expected_properties,
        None,
        Some("With Properties --- staged book".to_string()),
    )?;

    //  COMMIT - Round 1  //
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- Round 1".to_string()),
    )?;
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before cloning from saved book".to_string()),
    )?;

    // ── PHASE C — Clone FROM SAVED  ───────────────
    // At this point, the source Book token (and any staged tokens included in the commit)
    // have state == Saved inside `fixture_holons`.
    let from_saved_key = MapString("Book.StageNewFromClone.FromSaved".to_string());

    //  Stage New From Clone  //
    let clone_from_saved_staged = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        book_staged_token,
        from_saved_key.clone(),
        None,
        Some("Stage New From Clone --  saved book.".to_string()),
    )?;

    //  Add properties  //
    let mut phase_c_expected_properties = PropertyMap::new();
    phase_c_expected_properties
        .insert("TITLE".to_property_name(), "Saved Clone of Dune".to_base_value());

    test_case.add_with_properties_step(
        &mut fixture_holons,
        clone_from_saved_staged,
        phase_c_expected_properties,
        None,
        Some("With Properties --- book clone from saved".to_string()),
    )?;

    //  COMMIT - Round 2  //
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit --- Round 2".to_string()),
    )?;
    // MATCH SAVED CONTENT  //
    test_case.add_match_saved_content_step()?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
