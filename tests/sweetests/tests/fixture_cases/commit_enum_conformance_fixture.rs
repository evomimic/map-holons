use super::enum_default_helpers::*;
use holons_prelude::prelude::*;
use holons_test::*;

pub fn commit_enum_conformance_fixture() -> Result<DancesTestCase, HolonError> {
    let mut init = start("Enum conformance")?;
    let (e, _, b) = enum_type(&mut init, "EnumAcceptance")?;
    let p = property(&mut init, "EnumAcceptance.Tone", "Tone", e.clone(), true, None)?;
    let t = record_type(&mut init, "EnumAcceptance.Record", p)?;
    let mut value = instance(
        &mut init,
        "EnumAcceptance.Value",
        &t,
        [("Tone".to_property_name(), token("Café"))].into(),
    )?;
    for bad in ["undeclared", "EnumAcceptance.Café", "Coffee display", "CAFÉ", "Cafe\u{301}"] {
        value = set(&mut init.test_case, &mut init.fixture_holons, value, "Tone", token(bad))?;
        reject(
            &mut init,
            vec![(
                value.clone(),
                vec![finding(
                    "DS-ENUM-002",
                    "EnumTokenMembership",
                    ExpectedValidationSubject::Value("Tone".into()),
                )],
            )],
            1,
        )?;
    }
    let value = set(&mut init.test_case, &mut init.fixture_holons, value, "Tone", token("Café"))?;
    let b =
        set(&mut init.test_case, &mut init.fixture_holons, b, "TypeName", "Café".to_base_value())?;
    // The consumer now also sees the ambiguous token.
    reject(
        &mut init,
        vec![
            (
                e.clone(),
                vec![finding(
                    "DS-ENUM-001",
                    "EnumMemberNamesUnique",
                    ExpectedValidationSubject::Holon,
                )],
            ),
            (
                value,
                vec![finding(
                    "DS-ENUM-002",
                    "EnumTokenMembership",
                    ExpectedValidationSubject::Value("Tone".into()),
                )],
            ),
        ],
        2,
    )?;
    set(&mut init.test_case, &mut init.fixture_holons, b, "TypeName", "Blue".to_base_value())?;
    let anchor = super::described_instance_helpers::saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "EnumVariantValueType.ValueType",
    )?;
    let e = relate(&mut init.test_case, &mut init.fixture_holons, e, "Variants", anchor.clone())?;
    reject(
        &mut init,
        vec![(
            e.clone(),
            vec![finding("DS-ENUM-001", "EnumMemberNamesUnique", ExpectedValidationSubject::Holon)],
        )],
        1,
    )?;
    init.test_case.add_remove_related_holons_step(
        &mut init.fixture_holons,
        e,
        "Variants".to_relationship_name(),
        vec![anchor],
        None,
        None,
    )?;
    let outside = definition(
        &mut init,
        "EnumAcceptance.Outside",
        "Outside",
        "MetaValueType.MetaTypeDescriptor",
        "MapEnumVariantValueType.EnumVariantValueType",
        PropertyMap::new(),
    )?;
    reject(
        &mut init,
        vec![(
            outside.clone(),
            vec![finding(
                "DS-KIND-004",
                "DescribingCategoryCompatibility",
                ExpectedValidationSubject::Holon,
            )],
        )],
        1,
    )?;
    let correct_meta = super::described_instance_helpers::saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "MetaEnumVariantValueType.MetaValueType",
    )?;
    init.test_case.add_with_descriptor_step(
        &mut init.fixture_holons,
        outside,
        correct_meta,
        PropertyMap::new(),
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
