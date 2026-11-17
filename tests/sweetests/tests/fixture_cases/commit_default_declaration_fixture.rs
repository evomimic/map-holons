use super::{described_instance_helpers::saved, enum_default_helpers::*};
use holons_prelude::prelude::*;
use holons_test::*;

pub fn commit_default_declaration_fixture() -> Result<DancesTestCase, HolonError> {
    let mut init = start("Default declaration conformance")?;
    let (e, _, _) = enum_type(&mut init, "DefaultAcceptance")?;
    let mut p =
        property(&mut init, "DefaultAcceptance.Tone", "Tone", e, false, Some(token("Café")))?;
    reject(
        &mut init,
        vec![(
            p.clone(),
            vec![finding(
                "DS-DEFAULT-001",
                "DefaultsRequireRequiredProperties",
                ExpectedValidationSubject::Holon,
            )],
        )],
        1,
    )?;
    p = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        p,
        "IsValueRequired",
        true.to_base_value(),
    )?;
    for (value, code, rule) in [
        (12_i64.to_base_value(), "BaseValueKindMismatch", "BaseValueKindMatchesEnum"),
        (token("Absent"), "DS-ENUM-002", "EnumTokenMembership"),
    ] {
        p = set(&mut init.test_case, &mut init.fixture_holons, p, "DefaultValue", value)?;
        reject(
            &mut init,
            vec![(
                p.clone(),
                vec![finding(code, rule, ExpectedValidationSubject::Value("DefaultValue".into()))],
            )],
            1,
        )?;
    }
    p = set(&mut init.test_case, &mut init.fixture_holons, p, "DefaultValue", token("Café"))?;
    let tight = saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "ShortText.MapStringValueType",
    )?;
    let constraint = saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "ShortText.StringLengthConstraint",
    )?;
    let constraint_type = saved(
        &init.fixture_context,
        &mut init.test_case,
        &mut init.fixture_holons,
        "StringLengthConstraint.ConstraintType",
    )?;
    let narrow = property(
        &mut init,
        "DefaultAcceptance.Narrow",
        "Narrow",
        tight.clone(),
        true,
        Some("abcdef".to_base_value()),
    )?;
    reject(
        &mut init,
        vec![(
            narrow.clone(),
            vec![ExpectedValidationFinding {
                kind: ExpectedValidationKind::ConstraintViolation {
                    code: "StringLengthOutOfRange".into(),
                    constraint,
                    constraint_type,
                    descriptor: tight,
                },
                rule_key: Some("DefaultValueConformance.ValidationRule".into()),
                subject: ExpectedValidationSubject::Value("DefaultValue".into()),
            }],
        )],
        1,
    )?;
    set(
        &mut init.test_case,
        &mut init.fixture_holons,
        narrow,
        "DefaultValue",
        "abcde".to_base_value(),
    )?;
    let t = record_type(&mut init, "DefaultAcceptance.Record", p)?;
    let value = instance(&mut init, "DefaultAcceptance.Omitted", &t, PropertyMap::new())?;
    reject(
        &mut init,
        vec![(
            value.clone(),
            vec![finding(
                "DS-PROP-001",
                "RequiredPropertyPresence",
                ExpectedValidationSubject::Property("Tone".into()),
            )],
        )],
        1,
    )?;
    set(&mut init.test_case, &mut init.fixture_holons, value, "Tone", token("Café"))?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    finish(init)
}
