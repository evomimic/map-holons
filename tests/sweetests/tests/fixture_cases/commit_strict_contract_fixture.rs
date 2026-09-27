use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::BOOK_DESCRIPTOR_KEY;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationSubject, TestCaseInit,
};

/// Public Commit rejects a populated property and an authored relationship
/// absent from the effective Book contract, regardless of loader permissiveness.
pub fn commit_strict_contract_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit strict described membership",
        "Undescribed property and relationship findings reach the staged client carrier",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let descriptor_key = MapString(BOOK_DESCRIPTOR_KEY.into());
    let descriptor_stub = fixture_context.mutation().new_holon(Some(descriptor_key.clone()))?;
    let descriptor = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        descriptor_stub,
        descriptor_key,
        None,
        None,
    )?;
    let target_key = MapString("Title.PropertyType".into());
    let target_stub = fixture_context.mutation().new_holon(Some(target_key.clone()))?;
    let target = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        target_stub,
        target_key,
        None,
        None,
    )?;
    let key = MapString("Book.StrictMembership".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    let properties: PropertyMap = [
        ("Title".to_property_name(), "Declared title".to_base_value()),
        ("Extraneous".to_property_name(), "not declared".to_base_value()),
    ]
    .into();
    let book = test_case.add_new_holon_step(
        &mut fixture_holons,
        source,
        properties,
        Some(key),
        None,
        None,
    )?;
    let book = test_case.add_stage_holon_step(&mut fixture_holons, book, None, None)?;
    let book = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book,
        "ExtraneousLink".to_relationship_name(),
        vec![target.clone()],
        None,
        None,
    )?;
    let book = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![descriptor],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: book,
            validation_state: ValidationState::Invalid,
            findings: vec![
                ExpectedValidationFinding {
                    kind: CommitValidationViolationKind::RuleViolation {
                        code: "DS-PROP-003".into(),
                    },
                    rule_key: Some("NoUndescribedProperties.ValidationRule".into()),
                    subject: ExpectedValidationSubject::Holon,
                },
                ExpectedValidationFinding {
                    kind: CommitValidationViolationKind::RuleViolation {
                        code: "UndeclaredRelationship".into(),
                    },
                    rule_key: None,
                    subject: ExpectedValidationSubject::Relationship {
                        name: "ExtraneousLink".into(),
                        target,
                    },
                },
            ],
        }],
        MapInteger(2),
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
