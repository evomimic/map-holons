use super::setup_book_author_inverse_schema_steps_with_context;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::ENSURE_DB_EMPTY;
use holons_test::{DancesTestCase, TestCaseInit};
use rstest::*;

#[fixture]
pub fn inverse_smartlink_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit {
        mut test_case,
        fixture_context,
        mut fixture_holons,
        mut fixture_bindings,
    } = TestCaseInit::new(
        "Inverse SmartLink Commit Testcase".to_string(),
        "Commits a declared relationship with staged schema and asserts both forward and inverse traversal".to_string(),
    );

    test_case.add_ensure_database_count_step(
        fixture_holons.count_saved(),
        Some(ENSURE_DB_EMPTY.to_string()),
    )?;

    setup_book_author_inverse_schema_steps_with_context(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        &mut fixture_bindings,
    )?;

    let authored_by_relationship = fixture_bindings
        .relationship_by_name(&MapString("BOOK_TO_PERSON".to_string()))
        .expect("Expected BOOK_TO_PERSON relationship binding")
        .clone();
    let authors_inverse_relationship = fixture_bindings
        .relationship_by_name(&MapString("PERSON_TO_BOOK".to_string()))
        .expect("Expected PERSON_TO_BOOK relationship binding")
        .clone();

    let book_token = fixture_bindings
        .get_token(&MapString("Book".to_string()))
        .expect("Expected Book token after schema setup")
        .clone();
    let person_1_token = fixture_bindings
        .get_token(&MapString("Person1".to_string()))
        .expect("Expected Person1 token after schema setup")
        .clone();
    let person_2_token = fixture_bindings
        .get_token(&MapString("Person2".to_string()))
        .expect("Expected Person2 token after schema setup")
        .clone();

    test_case.add_commit_step(
        &mut fixture_holons,
        None,
        Some("Commit staged schema and authored-by relationship".to_string()),
    )?;

    test_case.add_ensure_database_count_step(fixture_holons.count_saved(), None)?;

    test_case.add_assert_related_holons_step(
        &mut fixture_holons,
        book_token.clone(),
        authored_by_relationship,
        vec![person_1_token.clone(), person_2_token.clone()],
        None,
        Some("Assert forward AuthoredBy traversal from Book".to_string()),
    )?;

    test_case.add_assert_related_holons_step(
        &mut fixture_holons,
        person_1_token,
        authors_inverse_relationship.clone(),
        vec![book_token.clone()],
        None,
        Some("Assert inverse Authors traversal from Person1".to_string()),
    )?;

    test_case.add_assert_related_holons_step(
        &mut fixture_holons,
        person_2_token,
        authors_inverse_relationship,
        vec![book_token],
        None,
        Some("Assert inverse Authors traversal from Person2".to_string()),
    )?;

    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
