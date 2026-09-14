use super::described_instances::add_described_instance;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};
use integrity_core_types::HolonErrorKind;
use rstest::*;
use std::collections::BTreeMap;

/// Validates the multi-transaction lifecycle:
///
/// Phase 1 — Create and commit a holon in the first transaction.
/// Phase 2 — Prove the committed transaction rejects further mutations.
/// Phase 3 — Begin a fresh transaction and successfully create + commit a second holon.
///
/// This exercises:
/// - `Runtime::execute_command()` lifecycle gating (`requires_open_tx`)
/// - `RuntimeSession` multi-transaction ownership
/// - `BeginTransaction` → `activate_transaction` with fixture transient import
/// - Cross-transaction persistence (both holons visible after second commit)
#[fixture]
pub fn transaction_lifecycle_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, fixture_bindings: _ } =
        TestCaseInit::new(
            "Transaction Lifecycle Test",
            "Commit → rejection on committed tx → begin new tx → continue",
        );

    // ── Phase 1: First transaction — create and commit ──

    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.TransactionLifecycle.1",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit first transaction".to_string()),
    )?;

    test_case.add_match_saved_content_step()?;

    // ── Phase 2: Committed transaction rejects mutations ──

    // Attempt to create a new holon on the committed transaction — should be
    // rejected at the Runtime lifecycle gate with TransactionAlreadyCommitted.
    let rejected_transient = fixture_context
        .mutation()
        .new_holon(Some(MapString("Rejected.TransactionLifecycle".to_string())))?;

    let mut rejected_props = BTreeMap::new();
    rejected_props.insert("Title".to_property_name(), "Should Not Exist".to_base_value());

    test_case.add_new_holon_step(
        &mut fixture_holons,
        rejected_transient,
        rejected_props,
        Some(MapString("Rejected.TransactionLifecycle".to_string())),
        Some(HolonErrorKind::TransactionAlreadyCommitted),
        Some("NewHolon rejected on committed tx".to_string()),
    )?;

    // Attempt to re-commit the already-committed transaction.
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        Some(HolonErrorKind::TransactionAlreadyCommitted),
        Some("Re-commit rejected on committed tx".to_string()),
    )?;

    // ── Phase 3: Begin fresh transaction and continue work ──

    test_case.add_begin_transaction_step(None, Some("Begin second transaction".to_string()))?;

    add_described_instance(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "Book.TransactionLifecycle.2",
        "Title",
        BOOK_DESCRIPTOR_KEY,
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit second transaction".to_string()),
    )?;

    // Finalize
    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
