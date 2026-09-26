use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitCarrierFinding, ExpectedCommitStatus, ExpectedRejectedHolon,
    ExpectedValidationFinding, ExpectedValidationSubject, FixtureHolons, TestCaseInit,
    TestReference,
};
use std::sync::Arc;

fn saved(
    context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    key: &str,
) -> Result<TestReference, HolonError> {
    let key = MapString(key.into());
    let stub = context.mutation().new_holon(Some(key.clone()))?;
    test_case.add_lookup_saved_holon_by_key_step(holons, stub, key, None, None)
}

/// Staging only a descriptor assesses its saved owning Schema. The foreign
/// property target requires a direct dependency, so the Schema finding travels
/// through the response carrier. Correcting the descriptor permits a retry.
pub fn commit_unstaged_schema_finding_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Unstaged Schema Commit finding",
        "A staged descriptor exposes a saved Schema dependency finding and accepts correction",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let root =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "HolonType.TypeDescriptor")?;
    let meta = saved(
        &fixture_context,
        &mut test_case,
        &mut fixture_holons,
        "MetaHolonType.MetaTypeDescriptor",
    )?;
    let owner =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "BookAuthorInverseSchema")?;
    let foreign =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "ThemeName.PropertyType")?;
    let local = saved(&fixture_context, &mut test_case, &mut fixture_holons, "Title.PropertyType")?;

    let key = MapString("Book.SchemaReadiness.HolonType".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    // Staging precedes DescribedBy authoring, so its required default is not populated yet.
    let properties: PropertyMap = [
        ("TypeName".to_property_name(), "BookSchemaReadiness".to_base_value()),
        ("TypeNamePlural".to_property_name(), "BookSchemaReadinessTypes".to_base_value()),
        ("DisplayName".to_property_name(), "Book schema readiness".to_base_value()),
        ("DisplayNamePlural".to_property_name(), "Book schema readiness types".to_base_value()),
        ("Description".to_property_name(), "Commit validation fixture".to_base_value()),
        ("DefinesInstanceTypeKind".to_property_name(), false.to_base_value()),
        ("IsAbstractType".to_property_name(), false.to_base_value()),
        ("InstanceDeletionAllowed".to_property_name(), true.to_base_value()),
    ]
    .into();
    let descriptor = test_case.add_new_holon_step(
        &mut fixture_holons,
        source,
        properties,
        Some(key),
        None,
        None,
    )?;
    let descriptor = test_case.add_stage_holon_step(&mut fixture_holons, descriptor, None, None)?;
    let descriptor = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::Extends.as_relationship_name(),
        vec![root],
        None,
        None,
    )?;
    let descriptor = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::ComponentOf.as_relationship_name(),
        vec![owner],
        None,
        None,
    )?;
    let descriptor = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![foreign.clone()],
        None,
        None,
    )?;
    let descriptor = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![meta],
        None,
        None,
    )?;

    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_carrier_finding_step(
        ExpectedCommitCarrierFinding {
            schema_key: "BookAuthorInverseSchema".into(),
            rule_code: "DS-SCHEMA-002".into(),
            rule_key: "CrossSchemaDependenciesDeclared.ValidationRule".into(),
            expected_rejected_holons: vec![ExpectedRejectedHolon {
                token: descriptor.clone(),
                validation_state: ValidationState::Invalid,
                findings: vec![ExpectedValidationFinding {
                    kind: CommitValidationViolationKind::UnresolvedLocalDependency,
                    rule_key: None,
                    subject: ExpectedValidationSubject::Holon,
                }],
            }],
        },
        None,
    )?;
    let descriptor = test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![foreign],
        None,
        None,
    )?;
    let descriptor = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        descriptor,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![local],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;

    // The accepted descriptor now defines a valid additive property. A later
    // version may add members, but reusing an inherited identity is a defect.
    test_case.add_begin_transaction_step(None, None)?;
    let inherited_key =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "Key.PropertyType")?;
    let replacement = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        descriptor,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let replacement = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        replacement,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![inherited_key.clone()],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: replacement.clone(),
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::RuleViolation {
                    code: "DS-CONTRACT-001".into(),
                },
                rule_key: Some("NoInheritedMemberRedeclaration.ValidationRule".into()),
                subject: ExpectedValidationSubject::Holon,
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        replacement,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![inherited_key],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// A directly staged Schema is assessed even when its only authored change is
/// the DependsOn graph. A self dependency is the smallest versioned cycle.
pub fn commit_schema_cycle_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Staged Schema dependency cycle",
        "Public Commit rejects a directly staged Schema with a cyclic DependsOn graph",
    );
    test_case.add_begin_transaction_step(None, None)?;
    let schema_type =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "Schema.HolonType")?;
    let key = MapString("MAP Cycle Test Schema-v0.1.0".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    let properties: PropertyMap = [
        ("SchemaName".to_property_name(), key.clone().to_base_value()),
        ("Description".to_property_name(), "Cycle fixture".to_base_value()),
    ]
    .into();
    let schema = test_case.add_new_holon_step(
        &mut fixture_holons,
        source,
        properties,
        Some(key),
        None,
        None,
    )?;
    let schema = test_case.add_stage_holon_step(&mut fixture_holons, schema, None, None)?;
    let schema = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        schema.clone(),
        CoreRelationshipTypeName::DependsOn.as_relationship_name(),
        vec![schema],
        None,
        None,
    )?;
    let schema = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        schema,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![schema_type],
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: schema,
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: CommitValidationViolationKind::RuleViolation { code: "DS-SCHEMA-001".into() },
                rule_key: Some("SchemaDependenciesAcyclic.ValidationRule".into()),
                subject: ExpectedValidationSubject::Holon,
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
