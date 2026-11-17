//! Shared authoring vocabulary for exact enum and default Commit scenarios.
use super::{
    described_instance_helpers::saved,
    schema_authoring_helpers::{descriptor_properties, stage_definition},
};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::*;

pub(super) fn relate(
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
pub(super) fn fields(name: &str) -> PropertyMap {
    descriptor_properties(name)
}
pub(super) fn token(value: &str) -> BaseValue {
    BaseValue::EnumValue(MapEnumValue(value.into()))
}
pub(super) fn set(
    case: &mut DancesTestCase,
    holons: &mut FixtureHolons,
    subject: TestReference,
    name: &str,
    value: BaseValue,
) -> Result<TestReference, HolonError> {
    case.add_with_properties_step(
        holons,
        subject,
        [(name.to_property_name(), value)].into(),
        None,
        None,
    )
}
pub(super) fn definition(
    init: &mut TestCaseInit,
    key: &str,
    name: &str,
    meta: &str,
    parent: &str,
    mut properties: PropertyMap,
) -> Result<TestReference, HolonError> {
    let TestCaseInit { test_case, fixture_context, fixture_holons, .. } = init;
    let owner = saved(fixture_context, test_case, fixture_holons, "BookAuthorInverseSchema")?;
    let meta = saved(fixture_context, test_case, fixture_holons, meta)?;
    let parent = saved(fixture_context, test_case, fixture_holons, parent)?;
    let mut all = fields(name);
    all.append(&mut properties);
    stage_definition(
        fixture_context,
        test_case,
        fixture_holons,
        key,
        all,
        &[("Extends", &parent), ("ComponentOf", &owner), ("DescribedBy", &meta)],
    )
}
pub(super) fn property(
    init: &mut TestCaseInit,
    key: &str,
    name: &str,
    value: TestReference,
    required: bool,
    default: Option<BaseValue>,
) -> Result<TestReference, HolonError> {
    let mut properties: PropertyMap =
        [("IsValueRequired".to_property_name(), required.to_base_value())].into();
    if let Some(default) = default {
        properties.insert("DefaultValue".to_property_name(), default);
    }
    let p = definition(
        init,
        key,
        name,
        "MetaPropertyType.MetaTypeDescriptor",
        "PropertyType.TypeDescriptor",
        properties,
    )?;
    relate(&mut init.test_case, &mut init.fixture_holons, p, "ValueType", value)
}
pub(super) fn record_type(
    init: &mut TestCaseInit,
    key: &str,
    property: TestReference,
) -> Result<TestReference, HolonError> {
    let t = definition(
        init,
        key,
        key,
        "MetaHolonType.MetaTypeDescriptor",
        "HolonType.TypeDescriptor",
        [("InstanceDeletionAllowed".to_property_name(), true.to_base_value())].into(),
    )?;
    relate(&mut init.test_case, &mut init.fixture_holons, t, "InstanceProperties", property)
}
pub(super) fn instance(
    init: &mut TestCaseInit,
    key: &str,
    descriptor: &TestReference,
    properties: PropertyMap,
) -> Result<TestReference, HolonError> {
    super::query_qry2_seed_expand_fixture::stage_described(
        &mut init.test_case,
        &init.fixture_context,
        &mut init.fixture_holons,
        key,
        properties,
        descriptor,
        "enum/default consumer",
    )
}
pub(super) fn enum_type(
    init: &mut TestCaseInit,
    prefix: &str,
) -> Result<(TestReference, TestReference, TestReference), HolonError> {
    let e = definition(
        init,
        &format!("{prefix}.MapEnumValueType"),
        prefix,
        "MetaEnumValueType.MetaValueType",
        "MapEnumValueType.EnumValueType",
        PropertyMap::new(),
    )?;
    let a = definition(
        init,
        &format!("{prefix}.Café"),
        "Café",
        "MetaEnumVariantValueType.MetaValueType",
        "MapEnumVariantValueType.EnumVariantValueType",
        PropertyMap::new(),
    )?;
    let a = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        a,
        "DisplayName",
        "Coffee display".to_base_value(),
    )?;
    let b = definition(
        init,
        &format!("{prefix}.Blue"),
        "Blue",
        "MetaEnumVariantValueType.MetaValueType",
        "MapEnumVariantValueType.EnumVariantValueType",
        PropertyMap::new(),
    )?;
    let e = relate(&mut init.test_case, &mut init.fixture_holons, e, "Variants", a.clone())?;
    let e = relate(&mut init.test_case, &mut init.fixture_holons, e, "Variants", b.clone())?;
    Ok((e, a, b))
}
pub(super) fn finding(
    code: &str,
    rule: &str,
    subject: ExpectedValidationSubject,
) -> ExpectedValidationFinding {
    ExpectedValidationFinding {
        kind: core_types::CommitValidationViolationKind::RuleViolation { code: code.into() }.into(),
        rule_key: Some(format!("{rule}.ValidationRule")),
        subject,
    }
}
pub(super) fn reject(
    init: &mut TestCaseInit,
    expected: Vec<(TestReference, Vec<ExpectedValidationFinding>)>,
    total: i64,
) -> Result<(), HolonError> {
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Rejected,
        None,
        None,
    )?;
    init.test_case.add_verify_commit_rejection_step(
        expected
            .into_iter()
            .map(|(token, findings)| ExpectedRejectedHolon {
                token,
                findings,
                validation_state: ValidationState::Invalid,
            })
            .collect(),
        MapInteger(total),
        None,
    )
}
pub(super) fn start(name: &str) -> Result<TestCaseInit, HolonError> {
    let mut init = TestCaseInit::new(name, name);
    init.test_case.verify_committed_workset = true;
    init.test_case.add_load_book_person_inverse_test_schema_step(None)?;
    init.test_case.add_begin_transaction_step(None, None)?;
    Ok(init)
}
pub(super) fn finish(mut init: TestCaseInit) -> Result<DancesTestCase, HolonError> {
    init.test_case.add_match_saved_content_step()?;
    init.test_case.finalize(&init.fixture_context, &init.fixture_holons)?;
    Ok(init.test_case)
}
