//! Scalar bounds through public Commit; pure interval combinations live in holons_core tests.
use super::described_instance_helpers::saved;
use super::query_qry2_seed_expand_fixture::{instance_properties, stage_described};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationKind, ExpectedValidationSubject, NamedRelationshipSubject, TestCaseInit,
};

/// One finding for each scalar family and one for the descriptor field.
pub const EXPECTED_SCALAR_VIOLATIONS: i64 = 4;

struct ScalarCase {
    label: &'static str,
    property: &'static str,
    code: &'static str,
    constraint_key: &'static str,
    type_key: &'static str,
    value_type_key: &'static str,
    invalid: BaseValue,
    boundary: BaseValue,
}

pub fn commit_scalar_constraints_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit scalar constraints",
        "Scalar rejection, attributed wire findings, correction and boundary persistence",
    );
    test_case.verify_committed_workset = true;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut lookup = |key| saved(&fixture_context, &mut test_case, &mut fixture_holons, key);
    let book_type = lookup("Book.HolonType")?;
    let meta_type = lookup("ScalarTestMetaType.MetaHolonType")?;
    let holon_root = lookup("HolonType.TypeDescriptor")?;
    let schema = lookup("BookAuthorInverseSchema")?;
    let mut rejected = Vec::new();
    let mut corrections = Vec::new();
    for ScalarCase {
        label,
        property,
        code,
        constraint_key,
        type_key,
        value_type_key,
        invalid,
        boundary,
    } in [
        ScalarCase {
            label: "String",
            property: "ShortText",
            code: "StringLengthOutOfRange",
            constraint_key: "ShortText.StringLengthConstraint",
            type_key: "StringLengthConstraint.ConstraintType",
            value_type_key: "ShortText.MapStringValueType",
            invalid: "abcdef".to_base_value(),
            boundary: "a\u{301}bcde".to_base_value(),
        },
        ScalarCase {
            label: "Integer",
            property: "SignedScore",
            code: "NumericValueOutOfRange",
            constraint_key: "SignedScore.NumericRangeConstraint",
            type_key: "NumericRangeConstraint.ConstraintType",
            value_type_key: "SignedScore.MapIntegerValueType",
            invalid: (-6_i64).to_base_value(),
            boundary: (-5_i64).to_base_value(),
        },
        ScalarCase {
            label: "Bytes",
            property: "ShortBytes",
            code: "BytesLengthOutOfRange",
            constraint_key: "ShortBytes.BytesLengthConstraint",
            type_key: "BytesLengthConstraint.ConstraintType",
            value_type_key: "ShortBytes.MapBytesValueType",
            invalid: MapBytes(vec![1, 2, 3, 4]).to_base_value(),
            boundary: MapBytes(vec![1, 2, 3]).to_base_value(),
        },
        ScalarCase {
            label: "Descriptor",
            property: "ShortText",
            code: "StringLengthOutOfRange",
            constraint_key: "ShortText.StringLengthConstraint",
            type_key: "StringLengthConstraint.ConstraintType",
            value_type_key: "ShortText.MapStringValueType",
            invalid: "abcdef".to_base_value(),
            boundary: "abcde".to_base_value(),
        },
    ] {
        let descriptor_field = label == "Descriptor";
        let key = if descriptor_field {
            "ScalarAcceptance.HolonType".to_owned()
        } else {
            format!("Book.Scalar{label}")
        };
        let mut properties = if descriptor_field {
            let mut fields = instance_properties("TypeName", "ScalarAcceptance");
            fields.insert("DisplayName".to_property_name(), "Scalar acceptance".to_base_value());
            fields.insert(
                "Description".to_property_name(),
                "Constrained descriptor field".to_base_value(),
            );
            fields.insert("IsAbstractType".to_property_name(), false.to_base_value());
            fields.insert("DefinesInstanceTypeKind".to_property_name(), false.to_base_value());
            fields.insert("InstanceDeletionAllowed".to_property_name(), true.to_base_value());
            fields
        } else {
            instance_properties("Title", &key)
        };
        properties.insert(property.to_property_name(), invalid);
        let subject = stage_described(
            &mut test_case,
            &fixture_context,
            &mut fixture_holons,
            &key,
            properties,
            if descriptor_field { &meta_type } else { &book_type },
            if descriptor_field { "ScalarTestMetaType" } else { "Book" },
        )?;
        let subject = if descriptor_field {
            let subject = test_case.add_add_related_holons_step(
                &mut fixture_holons,
                subject,
                CoreRelationshipTypeName::Extends.as_relationship_name(),
                vec![holon_root.clone()],
                None,
                None,
            )?;
            test_case.add_add_related_holons_step(
                &mut fixture_holons,
                subject,
                CoreRelationshipTypeName::ComponentOf.as_relationship_name(),
                vec![schema.clone()],
                None,
                None,
            )?
        } else {
            subject
        };
        let constraint =
            saved(&fixture_context, &mut test_case, &mut fixture_holons, constraint_key)?;
        let constraint_type =
            saved(&fixture_context, &mut test_case, &mut fixture_holons, type_key)?;
        let descriptor =
            saved(&fixture_context, &mut test_case, &mut fixture_holons, value_type_key)?;
        rejected.push(ExpectedRejectedHolon {
            token: subject.clone(),
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: ExpectedValidationKind::ConstraintViolation {
                    code: code.into(),
                    constraint,
                    constraint_type,
                    descriptor,
                },
                rule_key: Some("PropertyValueConformance.ValidationRule".into()),
                subject: ExpectedValidationSubject::Value(property.into()),
            }],
        });
        corrections.push((subject, property, boundary));
    }
    test_case.add_commit_step_with_rejection_checks(
        &fixture_holons,
        vec![
            NamedRelationshipSubject {
                source: book_type,
                relationship: "Instances".to_relationship_name(),
            },
            NamedRelationshipSubject {
                source: meta_type,
                relationship: "Instances".to_relationship_name(),
            },
            NamedRelationshipSubject {
                source: schema,
                relationship: "Components".to_relationship_name(),
            },
        ],
        None,
    )?;
    test_case.add_verify_commit_rejection_step(
        rejected,
        MapInteger(EXPECTED_SCALAR_VIOLATIONS),
        None,
    )?;
    for (subject, property, value) in corrections {
        test_case.add_with_properties_step(
            &mut fixture_holons,
            subject,
            [(property.to_property_name(), value)].into(),
            None,
            None,
        )?;
    }
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
