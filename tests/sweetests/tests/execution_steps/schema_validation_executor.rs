use std::collections::{BTreeMap, BTreeSet};

use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    build_core_schema_bootstrap_content_set, expected_descriptor_keys,
};
use holons_test::TestExecutionState;
use holons_validation::{
    validate_holon, validate_value, HolonValidationContext, HolonValidationSubject,
    ValidationCollector, ValueValidationSubject,
};
use type_names::CoreValidationRuleName;

use super::descriptor_verification_executor::loaded_holons_with_context;

/// Assesses committed canonical inputs without replacing outcomes or invoking Commit.
pub async fn execute_verify_schema_validation_conformance(state: &mut TestExecutionState) {
    let step = "verify_schema_validation_conformance";
    let content_set = build_core_schema_bootstrap_content_set()
        .expect("conformance requires the manifest-selected bootstrap inputs");
    let mut remaining_descriptors = expected_descriptor_keys(&content_set);
    let descriptor_count = remaining_descriptors.len();
    let mut remaining_holons = BTreeMap::new();
    for file in &content_set.files_to_load {
        let document: serde_json::Value = serde_json::from_str(&file.raw_contents)
            .unwrap_or_else(|error| panic!("invalid schema JSON in {}: {error}", file.filename));
        for holon in document["holons"].as_array().expect("schema must contain holons") {
            let key = holon["key"].as_str().expect("canonical holon must have a key");
            assert!(
                remaining_holons.insert(key.to_owned(), file.filename.clone()).is_none(),
                "duplicate canonical holon key: {key}"
            );
        }
    }
    assert!(!remaining_holons.is_empty(), "conformance must assess canonical holons");
    let holon_count = remaining_holons.len();
    let (transaction, holons) = loaded_holons_with_context(state, step).await;
    // GetAllHolons excludes the current space, which is itself a manifest-selected input.
    let space = transaction
        .get_space_holon()
        .expect("conformance requires a readable current space reference")
        .expect("bootstrap must provision the current space holon");
    let context = HolonValidationContext::resolve(&transaction)
        .expect("conformance requires resolved anchors in the subjects' transaction");
    let mut collector = ValidationCollector::default();
    let mut assessed_keys = BTreeSet::new();
    let mut identities = BTreeMap::new();
    let mut bound_rules = BTreeSet::new();

    for holon in holons.get_members().iter().chain(std::iter::once(&space)) {
        let Some(key) = holon.key().expect("loaded holon key must be readable") else {
            continue;
        };
        assert!(!assessed_keys.contains(&key.0), "duplicate loaded canonical holon: {key}");
        let Some(filename) = remaining_holons.remove(&key.0) else {
            // Bootstrap also creates operational state outside the authored corpus.
            continue;
        };
        identities.insert(holon.reference_id_string(), (key.0.clone(), filename.clone()));
        if remaining_descriptors.remove(&key.0) {
            for binding in HolonDescriptor::from_holon(holon.clone())
                .effective_validation_bindings()
                .unwrap_or_else(|error| panic!("{key}: binding discovery failed: {error:?}"))
            {
                bound_rules.insert(
                    binding
                        .member
                        .key()
                        .expect("binding key read")
                        .expect("canonical rules must be keyed")
                        .0,
                );
            }
        }
        validate_holon(HolonValidationSubject { holon }, &context, &mut collector).unwrap_or_else(
            |error| panic!("{filename} :: {key}: incomplete assessment: {error:?}"),
        );
        assessed_keys.insert(key.0);
    }

    assert!(remaining_holons.is_empty(), "canonical holons not assessed: {remaining_holons:?}");
    assert!(
        remaining_descriptors.is_empty(),
        "canonical descriptors not assessed: {remaining_descriptors:?}"
    );
    assert_eq!(
        bound_rules,
        expected_rule_keys(),
        "all seven effective bindings must be discovered"
    );
    let observations = collector.observations().clone();
    assert_eq!(
        observations.effective_constraint_count, 0,
        "C1 holon/property/value traversal must reach no effective constraints"
    );
    let report = collector.into_report();
    assert!(
        report.violations.is_empty(),
        "canonical corpus produced {} findings: {:#?}\nSubject identities: {identities:#?}",
        report.violation_count(),
        report.violations
    );

    // The canonical corpus currently has no populated Bytes-valued property.
    // Keep its acceptance independent of this dispatch probe against the loaded schema.
    let bytes_descriptor = ValueDescriptor::from_holon(
        holons
            .get_by_key(&MapString::from("MapBytesValueType.BytesValueType"))
            .expect("bytes descriptor lookup")
            .expect("canonical bytes descriptor must be loaded"),
    );
    let bytes = BaseValue::BytesValue(MapBytes(vec![0, 1, 127, 255]));
    let bytes_path = core_types::ValidationSubjectPath::Value {
        holon_identity: "schema-validation-bytes-probe".to_owned(),
        property: "MapBytes".to_owned(),
    };
    let mut probe_collector = ValidationCollector::default();
    validate_value(
        ValueValidationSubject { descriptor: &bytes_descriptor, value: &bytes, path: &bytes_path },
        context.value_context(),
        &mut probe_collector,
    )
    .expect("Bytes coverage probe must complete");
    let probe_observations = probe_collector.observations().clone();
    let bytes_rule = CoreValidationRuleName::BaseValueKindMatchesBytes.as_str().to_owned();
    assert_eq!(probe_observations.discovered_rule_keys, BTreeSet::from([bytes_rule.clone()]));
    assert_eq!(probe_observations.dispatched_rule_keys, BTreeSet::from([bytes_rule.clone()]));
    assert_eq!(probe_observations.effective_constraint_count, 0);
    let probe_report = probe_collector.into_report();
    assert!(
        probe_report.is_accepted(),
        "Bytes coverage probe findings: {:#?}",
        probe_report.violations
    );

    let corpus_rules: BTreeSet<_> =
        expected_rule_keys().into_iter().filter(|key| key != &bytes_rule).collect();
    assert!(
        corpus_rules.is_subset(&observations.discovered_rule_keys),
        "canonical traversal missed bindings: {:?}",
        corpus_rules.difference(&observations.discovered_rule_keys).collect::<Vec<_>>()
    );
    assert!(
        corpus_rules.is_subset(&observations.dispatched_rule_keys),
        "canonical traversal missed handlers: {:?}",
        corpus_rules.difference(&observations.dispatched_rule_keys).collect::<Vec<_>>()
    );
    assert_eq!(
        observations
            .discovered_rule_keys
            .union(&probe_observations.discovered_rule_keys)
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected_rule_keys()
    );
    assert_eq!(
        observations
            .dispatched_rule_keys
            .union(&probe_observations.dispatched_rule_keys)
            .cloned()
            .collect::<BTreeSet<_>>(),
        expected_rule_keys()
    );

    tracing::info!(
        holon_count,
        descriptor_count,
        ?observations,
        ?probe_observations,
        "verified report-only canonical schema conformance and separate Bytes dispatch coverage"
    );
}

fn expected_rule_keys() -> BTreeSet<String> {
    use CoreValidationRuleName::*;
    [
        RequiredPropertyPresence,
        NoUndescribedProperties,
        BaseValueKindMatchesString,
        BaseValueKindMatchesInteger,
        BaseValueKindMatchesBoolean,
        BaseValueKindMatchesBytes,
        BaseValueKindMatchesEnum,
    ]
    .into_iter()
    .map(|rule| rule.as_str().to_owned())
    .collect()
}
