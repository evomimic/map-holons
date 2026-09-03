use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, TestCaseInit};

const OPERATIONAL_EXTENSION_DESCRIPTOR_KEYS: [&str; 6] = [
    "CommandValidationRule.HolonType",
    "MetaDanceType.MetaHolonType",
    "CommandType.HolonType",
    "Query.HolonType",
    "QueryDance.DanceType",
    "Visualizer.HolonType",
];

/// Verifies the complete schema composition provisioned into the first
/// CoreSchemaSpace. This does not load schemas: production startup and every
/// Sweettest runtime must already have committed the manifest-selected
/// operational packages before an ordinary transaction can begin.
pub fn bootstrap_operational_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "bootstrap_operational_schema",
        "Verify the manifest-selected operational schema composition is available after bootstrap",
    );

    test_case.add_verify_core_schema_descriptors_step(None)?;
    test_case.add_verify_core_schema_descriptor_subtypes_step(None)?;
    test_case.add_verify_core_schema_command_affordances_step(None)?;
    test_case.add_verify_core_schema_value_semantics_step(None)?;
    test_case.add_verify_validation_bindings_descriptor_contract_step(None)?;

    for descriptor_key in OPERATIONAL_EXTENSION_DESCRIPTOR_KEYS {
        let stub =
            fixture_context.mutation().new_holon(Some(MapString(descriptor_key.to_string())))?;
        test_case.add_lookup_saved_holon_by_key_step(
            &mut fixture_holons,
            stub,
            MapString(descriptor_key.to_string()),
            None,
            None,
        )?;
    }

    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
