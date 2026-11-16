use super::{described_instance_helpers::saved, enum_default_helpers::*};
use holons_prelude::prelude::*;
use holons_test::*;

pub fn commit_conformance_fixture() -> Result<DancesTestCase, HolonError> {
    let mut init = start("Holon conformance")?;
    let string = saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "MapStringValueType.StringValueType",
    )?;
    let p =
        property(&mut init, "ConformanceAcceptance.First", "Value", string.clone(), false, None)?;
    let t = record_type(&mut init, "ConformanceAcceptance.Record", p)?;
    let t = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        t,
        "IsAbstractType",
        true.to_base_value(),
    )?;
    let v = instance(
        &mut init,
        "ConformanceAcceptance.Value",
        &t,
        [("Value".to_property_name(), "present".to_base_value())].into(),
    )?;
    reject(
        &mut init,
        vec![(
            v,
            vec![finding(
                "DS-CONFORM-001",
                "ConcreteDescribingType",
                ExpectedValidationSubject::Holon,
            )],
        )],
        1,
    )?;
    set(&mut init.test_case, &mut init.fixture_holons, t, "IsAbstractType", false.to_base_value())?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    init.test_case.add_begin_transaction_step(None, None)?;
    let first =
        property(&mut init, "ConformanceCollision.First", "Clash", string.clone(), false, None)?;
    let second = property(&mut init, "ConformanceCollision.Second", "Clash", string, false, None)?;
    let contract = record_type(&mut init, "ConformanceCollision.Record", first)?;
    let contract = relate(
        &mut init.test_case,
        &mut init.fixture_holons,
        contract,
        "InstanceProperties",
        second.clone(),
    )?;
    let populated = instance(
        &mut init,
        "ConformanceCollision.Populated",
        &contract,
        [("Clash".to_property_name(), "value".to_base_value())].into(),
    )?;
    let empty = instance(&mut init, "ConformanceCollision.Empty", &contract, PropertyMap::new())?;
    reject(
        &mut init,
        vec![
            (
                contract,
                vec![finding(
                    "DS-CONTRACT-002",
                    "UniqueSemanticMemberNames",
                    ExpectedValidationSubject::Holon,
                )],
            ),
            (
                populated,
                vec![finding(
                    "DS-BIND-001",
                    "UniquePropertyMemberBinding",
                    ExpectedValidationSubject::Holon,
                )],
            ),
            (
                empty,
                vec![ExpectedValidationFinding {
                    kind: core_types::CommitValidationViolationKind::UnresolvedLocalDependency
                        .into(),
                    rule_key: None,
                    subject: ExpectedValidationSubject::Holon,
                }],
            ),
        ],
        3,
    )?;
    set(
        &mut init.test_case,
        &mut init.fixture_holons,
        second,
        "TypeName",
        "Other".to_base_value(),
    )?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    init.test_case.add_begin_transaction_step(None, None)?;
    // An abstract property family may omit category-specific requiredness, but not universal metadata.
    let abstract_property = definition(
        &mut init,
        "ConformanceMinimum.AbstractProperty",
        "AbstractProperty",
        "MetaPropertyType.MetaTypeDescriptor",
        "PropertyType.TypeDescriptor",
        [("IsAbstractType".to_property_name(), true.to_base_value())].into(),
    )?;
    let abstract_property = init.test_case.add_remove_properties_step(
        &mut init.fixture_holons,
        abstract_property,
        [("Description".to_property_name(), "Private descriptor fixture".to_base_value())].into(),
        None,
        None,
    )?;
    reject(
        &mut init,
        vec![(
            abstract_property.clone(),
            vec![finding(
                "DS-PROP-001",
                "RequiredPropertyPresence",
                ExpectedValidationSubject::Property("Description".into()),
            )],
        )],
        1,
    )?;
    let abstract_property = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        abstract_property,
        "Description",
        "Abstract property minimums".to_base_value(),
    )?;
    let abstract_property = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        abstract_property,
        "IsValueRequired",
        "wrong kind".to_base_value(),
    )?;
    reject(
        &mut init,
        vec![(
            abstract_property.clone(),
            vec![finding(
                "BaseValueKindMismatch",
                "BaseValueKindMatchesBoolean",
                ExpectedValidationSubject::Value("IsValueRequired".into()),
            )],
        )],
        1,
    )?;
    init.test_case.add_remove_properties_step(
        &mut init.fixture_holons,
        abstract_property,
        [("IsValueRequired".to_property_name(), "wrong kind".to_base_value())].into(),
        None,
        None,
    )?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    finish(init)
}
