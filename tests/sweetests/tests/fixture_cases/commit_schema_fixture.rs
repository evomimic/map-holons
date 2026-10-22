use core_types::CommitValidationViolationKind;
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitCandidate, ExpectedCommitStatus, ExpectedDisposition,
    ExpectedRejectedHolon, ExpectedValidationFinding, ExpectedValidationSubject, FixtureHolons,
    TestCaseInit, TestReference,
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

/// Declared cross-schema dependency coverage is temporarily unenforced. A foreign
/// property reference commits, while inherited-member redeclaration still rejects.
pub fn commit_cross_schema_dependency_gap_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Cross-schema dependency coverage gap",
        "A cross-schema reference commits while inherited-member redeclaration still rejects",
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
        vec![foreign],
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

    // DS-SCHEMA-002 is unbound until SchemaDependency restores holon-local
    // dependency coverage, so the undeclared foreign property commits. Once it is
    // re-bound, this Commit should be Rejected with a DS-SCHEMA-002 finding on the
    // staged descriptor itself rather than a carrier finding on the unstaged Schema.
    // Other descriptor checks continue through public Commit.
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;

    // The accepted descriptor defines an additive foreign property. A later
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
    let replacement = test_case.add_remove_related_holons_step(
        &mut fixture_holons,
        replacement,
        CoreRelationshipTypeName::InstanceProperties.as_relationship_name(),
        vec![inherited_key],
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(replacement, ExpectedDisposition::NewVersion)],
        vec![],
        None,
        None,
    )?;
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

/// Required Core definitions remain readable, but publishing their successors is
/// temporarily unsupported. Rejection preserves the source and all other candidates.
pub fn commit_required_anchor_update_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Required Core anchor update",
        "No-op staging passes; rekeying an anchor successor rejects before any write",
    );
    test_case.add_begin_transaction_step(None, None)?;
    let anchor =
        saved(&fixture_context, &mut test_case, &mut fixture_holons, "HolonType.TypeDescriptor")?;
    let unchanged = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        anchor,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    test_case.add_commit_step_with_dispositions(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![ExpectedCommitCandidate::new(unchanged.clone(), ExpectedDisposition::NoAction)],
        vec![],
        None,
        None,
    )?;
    test_case.add_begin_transaction_step(None, None)?;
    let successor = test_case.add_stage_new_version_step(
        &mut fixture_holons,
        unchanged,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let successor = test_case.add_with_properties_step(
        &mut fixture_holons,
        successor,
        [("Key".to_property_name(), "Rekeyed.RequiredCoreAnchor".to_base_value())].into(),
        None,
        None,
    )?;
    // A second, independent error must still be reported by the common assessment.
    let key = MapString("Undescribed.AnchorRestriction.Control".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    let control = test_case.add_new_holon_step(
        &mut fixture_holons,
        source,
        PropertyMap::new(),
        Some(key),
        None,
        None,
    )?;
    let control = test_case.add_stage_holon_step(&mut fixture_holons, control, None, None)?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Rejected, None, None)?;
    test_case.add_verify_commit_rejection_step(
        vec![
            ExpectedRejectedHolon {
                token: successor,
                validation_state: ValidationState::Invalid,
                findings: vec![ExpectedValidationFinding {
                    kind: CommitValidationViolationKind::RuleViolation {
                        code: "UnsupportedRequiredAnchorUpdate".into(),
                    },
                    rule_key: None,
                    subject: ExpectedValidationSubject::Holon,
                }],
            },
            ExpectedRejectedHolon {
                token: control,
                validation_state: ValidationState::NoDescriptor,
                findings: vec![ExpectedValidationFinding {
                    kind: CommitValidationViolationKind::NoDescriptor,
                    rule_key: None,
                    subject: ExpectedValidationSubject::Holon,
                }],
            },
        ],
        MapInteger(2),
        None,
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}

/// Loader references may name a saved write source. A real definitional edit
/// must reach the same anchor restriction as an ordinary staged successor.
pub fn loader_required_anchor_update_fixture() -> Result<DancesTestCase, HolonError> {
    use super::load_holons_internal_fixture::{
        add_loader_relationship_reference, make_load_set_from_bundles, BundleWithFilename,
    };
    use holons_test::ExpectedLoadStatus;
    let TestCaseInit { mut test_case, fixture_context, fixture_holons, .. } = TestCaseInit::new(
        "Loader required anchor update",
        "Loader promotion of a saved Core write source rejects before persistence",
    );
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut carrier = fixture_context.mutation().new_holon(Some("AnchorEdit.Carrier".into()))?;
    add_loader_relationship_reference(
        &fixture_context,
        &mut carrier,
        "InstanceProperties",
        "HolonType.TypeDescriptor",
        &["Title.PropertyType"],
    )?;
    let mut bundle = fixture_context.mutation().new_holon(Some("AnchorEdit.Bundle".into()))?;
    bundle.add_related_holons(CoreRelationshipTypeName::BundleMembers, vec![carrier.into()])?;
    let load_set = make_load_set_from_bundles(
        &fixture_context,
        "AnchorEdit.LoadSet",
        vec![BundleWithFilename::new(bundle, "anchor-edit.json")],
    )?;
    test_case.add_load_holons_internal_step(
        load_set,
        MapInteger(1),
        MapInteger(0),
        MapInteger(1),
        MapInteger(0),
        MapInteger(1),
        MapInteger(1),
        ExpectedLoadStatus::Rejected,
        Some(MapInteger(2)),
    )?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
