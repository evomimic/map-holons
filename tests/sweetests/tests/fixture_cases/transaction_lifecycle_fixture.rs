use holons_prelude::prelude::*;
use holons_test::harness::helpers::ENSURE_DB_EMPTY;
use holons_test::{DancesTestCase, TestCaseInit};
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
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        fixture_bindings: _,
    } = TestCaseInit::new(
        "Transaction Lifecycle Test",
        "Commit → rejection on committed tx → begin new tx → continue",
    );

    // ── Phase 1: First transaction — create and commit ──

    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some(ENSURE_DB_EMPTY.to_string()),
    )?;

    let book_key = MapString("book-lifecycle-1".to_string());
    let book_transient =
        fixture_context.mutation().new_holon(Some(book_key.clone()))?;

    let mut book_props = BTreeMap::new();
    book_props.insert(
        "Title".to_property_name(),
        "Lifecycle Book".to_base_value(),
    );
    book_props.insert(
        "Description".to_property_name(),
        "A holon for testing transaction lifecycle".to_base_value(),
    );

    let book_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_transient,
        book_props,
        Some(book_key),
        None,
        Some("Create Book in first transaction".to_string()),
    )?;

    test_case.add_stage_holon_step(
        &mut fixture_holons,
        book_token,
        None,
        Some("Stage Book".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        None,
        Some("Commit first transaction".to_string()),
    )?;

    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some("Verify Book saved".to_string()),
    )?;

    test_case.add_match_saved_content_step()?;

    // ── Phase 2: Committed transaction rejects mutations ──

    // Attempt to create a new holon on the committed transaction — should be
    // rejected at the Runtime lifecycle gate with TransactionAlreadyCommitted.
    let rejected_transient = fixture_context
        .mutation()
        .new_holon(Some(MapString("rejected".to_string())))?;

    let mut rejected_props = BTreeMap::new();
    rejected_props.insert(
        "Title".to_property_name(),
        "Should Not Exist".to_base_value(),
    );

    test_case.add_new_holon_step(
        &mut fixture_holons,
        rejected_transient,
        rejected_props,
        Some(MapString("rejected".to_string())),
        Some(HolonErrorKind::TransactionAlreadyCommitted),
        Some("NewHolon rejected on committed tx".to_string()),
    )?;

    // Attempt to re-commit the already-committed transaction.
    test_case.add_commit_step(
        &mut fixture_holons,
        Some(HolonErrorKind::TransactionAlreadyCommitted),
        Some("Re-commit rejected on committed tx".to_string()),
    )?;

    // ── Phase 3: Begin fresh transaction and continue work ──

    test_case.add_begin_transaction_step(
        None,
        Some("Begin second transaction".to_string()),
    )?;

    let article_key = MapString("article-lifecycle-1".to_string());
    let article_transient =
        fixture_context.mutation().new_holon(Some(article_key.clone()))?;

    let mut article_props = BTreeMap::new();
    article_props.insert(
        "Title".to_property_name(),
        "Lifecycle Article".to_base_value(),
    );
    article_props.insert(
        "Description".to_property_name(),
        "Created in the second transaction".to_base_value(),
    );

    let article_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        article_transient,
        article_props,
        Some(article_key),
        None,
        Some("Create Article in second transaction".to_string()),
    )?;

    test_case.add_stage_holon_step(
        &mut fixture_holons,
        article_token,
        None,
        Some("Stage Article".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        None,
        Some("Commit second transaction".to_string()),
    )?;

    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some("Verify both Book and Article saved".to_string()),
    )?;

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
