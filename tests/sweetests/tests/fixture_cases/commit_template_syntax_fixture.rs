use super::described_instance_helpers::saved;
use super::query_qry2_seed_expand_fixture::{instance_properties, stage_described};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, ExpectedCommitStatus, ExpectedRejectedHolon, ExpectedValidationFinding,
    ExpectedValidationKind, ExpectedValidationSubject, TestCaseInit,
};

/// Commit reports the Core syntax constraint on TemplateString and accepts a corrected retry.
pub fn commit_template_syntax_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Commit positional template syntax",
        "Correct a malformed template on a one-parameter FormatRule in the same transaction",
    );
    let mut lookup = |key| saved(&fixture_context, &mut test_case, &mut fixture_holons, key);
    let format_rule = lookup("FormatRule.KeyRuleType")?;
    let schema = lookup("MAP Core Schema-v0.0.7")?;
    let parameter = lookup("HolonSpaceName.PropertyType")?;
    let constraint = lookup("ValidSyntax.PositionalStringTemplateSyntaxConstraint")?;
    let constraint_type = lookup("PositionalStringTemplateSyntaxConstraint.ConstraintType")?;
    let descriptor = lookup("PositionalStringTemplateValueType.MapStringValueType")?;

    let mut properties = instance_properties("TypeName", "TemplateSyntaxRetry");
    properties.insert("TemplateString".to_property_name(), "{0}[.{1}".to_base_value());
    let mut rule = stage_described(
        &mut test_case,
        &fixture_context,
        &mut fixture_holons,
        "TemplateSyntaxRetry.FormatRule",
        properties,
        &format_rule,
        "FormatRule",
    )?;
    for (name, target) in [("RuleOf", schema), ("TemplateParameters", parameter)] {
        rule = test_case.add_add_related_holons_step(
            &mut fixture_holons,
            rule,
            name.to_relationship_name(),
            vec![target],
            None,
            None,
        )?;
    }
    test_case.add_commit_step_with_rejection_checks(&fixture_holons, vec![], None)?;
    test_case.add_verify_commit_rejection_step(
        vec![ExpectedRejectedHolon {
            token: rule.clone(),
            validation_state: ValidationState::Invalid,
            findings: vec![ExpectedValidationFinding {
                kind: ExpectedValidationKind::ConstraintViolation {
                    code: "PositionalStringTemplateSyntaxInvalid".into(),
                    constraint,
                    constraint_type,
                    descriptor,
                },
                rule_key: Some("PropertyValueConformance.ValidationRule".into()),
                subject: ExpectedValidationSubject::Value("TemplateString".into()),
            }],
        }],
        MapInteger(1),
        None,
    )?;
    test_case.add_with_properties_step(
        &mut fixture_holons,
        rule,
        instance_properties("TemplateString", "{0}"),
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
