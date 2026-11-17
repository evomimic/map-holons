//! Exact prospective references select staged configuration and staged governing definitions.
use super::described_instance_helpers::saved;
use super::query_qry2_seed_expand_fixture::{instance_properties, stage_described};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationKind,
    ExpectedValidationSubject, FixtureHolons, TestCaseInit, TestReference,
};
use std::sync::Arc;

fn descriptor_fields(name: &str) -> PropertyMap {
    [
        ("TypeName".to_property_name(), name.to_base_value()),
        ("DisplayName".to_property_name(), name.to_base_value()),
        ("Description".to_property_name(), "Prospective scalar contract".to_base_value()),
        ("IsAbstractType".to_property_name(), false.to_base_value()),
        ("DefinesInstanceTypeKind".to_property_name(), false.to_base_value()),
    ]
    .into()
}

fn relate(
    case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    source: TestReference,
    name: &str,
    target: TestReference,
) -> Result<TestReference, HolonError> {
    case.add_add_related_holons_step(
        holons,
        source,
        name.to_relationship_name(),
        vec![target],
        None,
        None,
    )
}

fn reject(
    case: &mut DancesTestCase,
    subject: TestReference,
    constraint: TestReference,
    constraint_type: TestReference,
    descriptor: TestReference,
    property: &str,
) -> Result<(), HolonError> {
    case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: subject,
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: ExpectedValidationKind::ConstraintViolation {
                    code: "StringLengthOutOfRange".into(),
                    constraint,
                    constraint_type,
                    descriptor,
                },
                rule_key: Some("PropertyValueConformance.ValidationRule".into()),
                subject: ExpectedValidationSubject::Value(property.into()),
            }],
        }],
        MapInteger(1),
        None,
    )
}

fn definition(
    case: &mut DancesTestCase,
    context: &Arc<TransactionContext>,
    holons: &mut FixtureHolons,
    key: &str,
    fields: PropertyMap,
    meta: &str,
    parent: TestReference,
    schema: TestReference,
) -> Result<TestReference, HolonError> {
    let meta = saved(context, case, holons, meta)?;
    let subject =
        stage_described(case, context, holons, key, fields, &meta, "prospective definition")?;
    let subject = relate(case, holons, subject, "Extends", parent)?;
    relate(case, holons, subject, "ComponentOf", schema)
}

/// Saved roots shared by every prospective contract in this fixture.
struct Roots {
    schema: TestReference,
    string_root: TestReference,
    property_root: TestReference,
    holon_root: TestReference,
    constraint_type: TestReference,
}

/// A staged record type whose single `name` property is bounded to three graphemes.
struct Contract {
    constraint: TestReference,
    value_type: TestReference,
    record_type: TestReference,
}

fn contract(
    case: &mut DancesTestCase,
    context: &Arc<TransactionContext>,
    holons: &mut FixtureHolons,
    roots: &Roots,
    name: &str,
) -> Result<Contract, HolonError> {
    let mut parameters = instance_properties("ConstraintName", &format!("{name}Bound"));
    parameters.insert("Maximum".to_property_name(), 3_i64.to_base_value());
    parameters.insert("MaximumIsInclusive".to_property_name(), true.to_base_value());
    let constraint = stage_described(
        case,
        context,
        holons,
        &format!("{name}Bound.StringLengthConstraint"),
        parameters,
        &roots.constraint_type,
        "StringLengthConstraint",
    )?;
    let constraint = relate(case, holons, constraint, "RuleOf", roots.schema.clone())?;
    let value_type = definition(
        case,
        context,
        holons,
        &format!("{name}String.MapStringValueType"),
        descriptor_fields(&format!("{name}String")),
        "MetaStringValueType.MetaValueType",
        roots.string_root.clone(),
        roots.schema.clone(),
    )?;
    let value_type = relate(case, holons, value_type, "Constraints", constraint.clone())?;
    let mut fields = descriptor_fields(name);
    fields.insert("IsValueRequired".to_property_name(), false.to_base_value());
    let property = definition(
        case,
        context,
        holons,
        &format!("{name}.PropertyType"),
        fields,
        "MetaPropertyType.MetaTypeDescriptor",
        roots.property_root.clone(),
        roots.schema.clone(),
    )?;
    let property = relate(case, holons, property, "ValueType", value_type.clone())?;
    let mut fields = descriptor_fields(&format!("{name}Record"));
    fields.insert("InstanceDeletionAllowed".to_property_name(), true.to_base_value());
    let record_type = definition(
        case,
        context,
        holons,
        &format!("{name}Record.HolonType"),
        fields,
        "MetaHolonType.MetaTypeDescriptor",
        roots.holon_root.clone(),
        roots.schema.clone(),
    )?;
    let record_type = relate(case, holons, record_type, "InstanceProperties", property)?;
    Ok(Contract { constraint, value_type, record_type })
}

pub fn commit_staged_scalar_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit prospective scalar contracts",
        "Staged configuration and exact staged governing selections are evaluated by public Commit",
    );
    test_case.verify_committed_workset = true;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut lookup = |key| saved(&fixture_context, &mut test_case, &mut fixture_holons, key);
    let roots = Roots {
        schema: lookup("BookAuthorInverseSchema")?,
        string_root: lookup("MapStringValueType.StringValueType")?,
        property_root: lookup("PropertyType.TypeDescriptor")?,
        holon_root: lookup("HolonType.TypeDescriptor")?,
        constraint_type: lookup("StringLengthConstraint.ConstraintType")?,
    };
    let constraint_type = roots.constraint_type.clone();
    // Step expectations follow relationship snapshots captured when each edge was authored,
    // so the contract whose configuration is edited stays apart from the one versioned below.
    let Contract { record_type, .. } =
        contract(&mut test_case, &fixture_context, &mut fixture_holons, &roots, "Window")?;
    let gauge = contract(&mut test_case, &fixture_context, &mut fixture_holons, &roots, "Gauge")?;
    let record = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "GaugeRecord.StagedConfiguration",
        instance_properties("Gauge", "abcd"),
        &gauge.record_type,
        "GaugeRecord",
    )?;
    test_case.add_commit_step_with_rejection_checks(&fixture_holons, vec![], None)?;
    reject(
        &mut test_case,
        record,
        gauge.constraint.clone(),
        constraint_type,
        gauge.value_type,
        "Gauge",
    )?;
    // Change only the staged configuration: fresh preparation must now accept four graphemes.
    test_case.add_with_properties_step(
        &mut fixture_holons,
        gauge.constraint,
        [("Maximum".to_property_name(), 4_i64.to_base_value())].into(),
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;

    test_case.add_begin_transaction_step(None, None)?;
    let tight_type = saved(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "ShortText.MapStringValueType",
    )?;
    let tight_constraint = saved(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "ShortText.StringLengthConstraint",
    )?;
    let tight_constraint_type = saved(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "StringLengthConstraint.ConstraintType",
    )?;
    // The version adds a staged property and removes nothing: relationship expectations are
    // snapshots, so a target committed since it was linked cannot be removed by its saved head.
    let mut fields = descriptor_fields("Narrow");
    fields.insert("IsValueRequired".to_property_name(), false.to_base_value());
    let narrow = definition(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "Narrow.PropertyType",
        fields,
        "MetaPropertyType.MetaTypeDescriptor",
        roots.property_root.clone(),
        roots.schema.clone(),
    )?;
    let narrow =
        relate(&mut test_case, &mut fixture_holons, narrow, "ValueType", tight_type.clone())?;
    let updated_type = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        record_type,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let updated_type = relate(
        &mut test_case,
        &mut fixture_holons,
        updated_type,
        "InstanceProperties",
        narrow.clone(),
    )?;
    let record = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "WindowRecord.ProspectiveSelection",
        instance_properties("Narrow", "abcdef"),
        &updated_type,
        "WindowRecord",
    )?;
    // Updates are outside the create-only residue verifier; this attempt checks findings only.
    // The saved WindowRecord type declares no Narrow property, so a bound finding on it can
    // only come from the staged type version and its staged property.
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    reject(
        &mut test_case,
        record.clone(),
        tight_constraint,
        tight_constraint_type,
        tight_type,
        "Narrow",
    )?;
    let record = test_case.add_with_properties_step(
        &mut fixture_holons,
        record,
        instance_properties("Narrow", "abcde"),
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(narrow, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(updated_type, ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(record, ExpectedDisposition::NewRoot),
        ],
        vec![],
        None,
        None,
    )?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
