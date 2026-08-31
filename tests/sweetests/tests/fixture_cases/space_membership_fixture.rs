//! Holon space membership across the holon lifecycle (Storage SL5b-V1).
//!
//! Every published lineage acquires `OwnedBy → current HolonSpace`, and commit materializes the
//! reciprocal `Owns` SmartLink on the space. This fixture pins that across the three lifecycle
//! events this fixture can reach without a loaded schema:
//!
//! - **create** — a committed holon becomes a member, and an independent clone becomes a member in
//!   its own right;
//! - **delete** — membership is retracted in both directions.
//!
//! Assertions go through `VerifySpaceMembership`, which traverses `Owns` / `OwnedBy` directly.
//! `GetAllHolons` is deliberately not used: it still reads the `AllHolonNodes` index at this
//! point, so asserting through it would prove nothing about the membership graph.

use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use rstest::*;
use std::collections::BTreeMap;

#[fixture]
pub fn space_membership_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _fixture_bindings,
    } = TestCaseInit::new(
        "Holon Space Membership Testcase",
        "Assert OwnedBy/Owns membership is created on publication, inherited (not duplicated) by \
         a new version, and retracted on delete — all through ordinary relationship traversal.",
    );

    // ── Create ─────────────────────────────────────────────────────────────────
    let book_key = MapString("membership:book".to_string());
    let book_transient = fixture_context.mutation().new_holon(Some(book_key.clone()))?;
    let mut book_properties = BTreeMap::new();
    book_properties.insert("title".to_property_name(), "Owned Book".to_base_value());

    let book_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_transient,
        book_properties,
        Some(book_key.clone()),
        None,
        Some("Creating book holon...".to_string()),
    )?;
    let staged_book = test_case.add_stage_holon_step(
        &mut fixture_holons,
        book_token,
        None,
        Some("Staging book holon...".to_string()),
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_verify_space_membership_step(
        staged_book.clone(),
        vec![book_key.clone()],
        Some("Publishing a lineage makes it an owned member".to_string()),
    )?;

    // ── Independent clone — a new lineage, owned in its own right ──────────────
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before cloning".to_string()),
    )?;
    let clone_key = MapString("membership:book-clone".to_string());
    let cloned_book = test_case.add_stage_new_from_clone_step(
        &mut fixture_holons,
        staged_book.clone(),
        clone_key.clone(),
        None,
        Some("Cloning the book into an independent lineage...".to_string()),
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;

    test_case.add_verify_space_membership_step(
        staged_book.clone(),
        vec![book_key.clone(), clone_key.clone()],
        Some("An independent clone is owned in its own right".to_string()),
    )?;

    // The version-producing leg lives in `stage_new_version_fixture` instead: staging a new
    // version stages `Predecessor`, whose inverse is resolved through the source holon's
    // `DescribedBy`, so it requires a described holon. That fixture is schema-backed; this one is
    // deliberately not, so that membership is shown to work without any schema loaded.

    // ── Delete — membership retracted in both directions ───────────────────────
    test_case.add_begin_transaction_step(
        None,
        Some("Begin new transaction before delete".to_string()),
    )?;
    test_case.add_delete_holon_step(
        &mut fixture_holons,
        cloned_book,
        None,
        Some("Deleting the cloned holon...".to_string()),
    )?;

    test_case.add_verify_space_membership_step(
        staged_book,
        vec![book_key],
        Some("Deleting a holon retracts its membership".to_string()),
    )?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
