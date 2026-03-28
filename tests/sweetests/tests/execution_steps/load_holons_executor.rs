use core_types::TemporaryId;
use holons_core::reference_layer::{ReadableHolon, TransientReference};
use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::info;

/// Read an integer property from a transient response holon.
fn read_integer_property(
    response: &TransientReference,
    property: CorePropertyTypeName,
) -> Result<i64, HolonError> {
    match response.property_value(&property)? {
        Some(PropertyValue::IntegerValue(MapInteger(i))) => Ok(i),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected integer value for {:?}, got {:?}",
            property, other
        ))),
    }
}

/// Dump the `EssentialHolonContent` for any holon-like reference in a stable,
/// human-readable format (test debugging aid).
fn dump_essential(_state: &mut TestExecutionState, holon_reference: &impl ReadableHolon) -> String {
    let essential_content = match holon_reference.essential_content() {
        Ok(content) => content,
        Err(error) => {
            return format!("<error reading essential content: {:?}>", error);
        }
    };

    let mut output = String::new();
    output.push_str("\n==== EssentialContent ===\n");

    let mut sorted_entries: Vec<_> = essential_content.property_map.iter().collect();
    sorted_entries.sort_by(|(name_a, _), (name_b, _)| name_a.0 .0.cmp(&name_b.0 .0));

    for (property_name, property_value) in sorted_entries {
        output.push_str(&format!("  {} = {:?}\n", property_name.0 .0, property_value));
    }

    if let Some(key_value) = essential_content.key.clone() {
        output.push_str(&format!("  (key) = {}\n", key_value.0));
    }

    if !essential_content.errors.is_empty() {
        output.push_str("  errors:\n");
        for error in essential_content.errors {
            output.push_str(&format!("    - {:?}\n", error));
        }
    }

    output.push_str("==== end ====\n");
    output
}

/// Dump the full HolonLoadResponse holon for debugging.
fn dump_full_response(state: &mut TestExecutionState, response: &TransientReference) -> String {
    let mut out = String::new();
    out.push_str("\n===== HolonLoadResponse dump =====\n");
    out.push_str(&dump_essential(state, response));

    out.push_str("----- important (best-effort loader fields) -----\n");
    for p_name in [
        CorePropertyTypeName::ErrorCount.as_property_name(),
        CorePropertyTypeName::HolonsStaged.as_property_name(),
        CorePropertyTypeName::HolonsCommitted.as_property_name(),
        CorePropertyTypeName::LinksCreated.as_property_name(),
        CorePropertyTypeName::DanceSummary.as_property_name(),
        CorePropertyTypeName::TotalBundles.as_property_name(),
        CorePropertyTypeName::TotalLoaderHolons.as_property_name(),
    ]
    .iter()
    {
        if let Ok(Some(val)) = response.property_value(p_name) {
            out.push_str(&format!("  {} => {:?}\n", p_name.0 .0, val));
        }
    }

    out.push_str("===== end HolonLoadResponse dump =====\n");
    out
}

/// Dump error holons attached via `HasLoadError` relationship.
fn dump_error_holons_from_response(
    state: &mut TestExecutionState,
    response_reference: &TransientReference,
) -> String {
    let mut output = String::new();

    let relationship_name = CoreRelationshipTypeName::HasLoadError;
    let collection_handle = match response_reference.related_holons(&relationship_name) {
        Ok(collection) => collection,
        Err(_) => return output,
    };

    let members = match collection_handle.read() {
        Ok(guard) => guard.get_members().clone(),
        Err(_) => {
            output.push_str("[loader-test] <failed to read HasLoadError collection>\n");
            return output;
        }
    };

    if members.is_empty() {
        return output;
    }

    output.push_str("\n===== Loader Error Holons (HasLoadError) =====\n");

    for (index, holon_reference) in members.into_iter().enumerate() {
        let transient_reference = match holon_reference.clone_holon() {
            Ok(reference) => reference,
            Err(error) => {
                output.push_str(&format!(
                    "  [{}] <failed to clone error holon: {:?}>\n",
                    index, error
                ));
                continue;
            }
        };

        output.push_str(&format!("  --- error holon #{} ---\n", index + 1));

        let essential_dump = dump_essential(state, &transient_reference);
        for line in essential_dump.lines() {
            output.push_str("    ");
            output.push_str(line);
            output.push('\n');
        }
    }

    output.push_str("===== end Loader Error Holons =====\n");
    output
}

/// Execute the `LoadHolons` step via `TransactionAction::LoadHolons`,
/// then assert each response property on the returned response holon.
pub async fn execute_load_holons(
    test_state: &mut TestExecutionState,
    load_set_id: TemporaryId,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
    expect_total_bundles: MapInteger,
    expect_total_loader_holons: MapInteger,
) {
    info!("--- TEST STEP: Load Holons ---");
    let context = test_state.context();

    // Reconstruct the load-set reference inside the active transaction using the
    // fixture-time TemporaryId. The active transaction has already been seeded
    // with the fixture transient pool by the harness.
    let context_handle = TransactionContextHandle::new(context.clone());
    let rebound_set_reference = TransientReference::from_temporary_id(context_handle, &load_set_id);

    // Dispatch via TransactionAction::LoadHolons
    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::LoadHolons {
            bundle: HolonReference::Transient(rebound_set_reference),
        },
    });
    let result =
        test_state.dispatch_command(command, "load_holons").await.expect("load_holons failed");

    // Extract the response TransientReference
    let response_reference: TransientReference = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        MapResult::Reference(other_ref) => other_ref.clone_holon().unwrap_or_else(|e| {
            panic!("LoadHolons returned non-transient reference and clone_holon failed: {e:?}")
        }),
        other => panic!("LoadHolons: expected Reference(Transient), got {:?}", other),
    };

    // Read response properties
    let actual_staged =
        read_integer_property(&response_reference, CorePropertyTypeName::HolonsStaged)
            .unwrap_or_else(|e| panic!("read HolonsStaged failed: {e:?}")) as i64;
    let actual_committed =
        read_integer_property(&response_reference, CorePropertyTypeName::HolonsCommitted)
            .unwrap_or_else(|e| panic!("read HolonsCommitted failed: {e:?}")) as i64;
    let actual_links_created =
        read_integer_property(&response_reference, CorePropertyTypeName::LinksCreated)
            .unwrap_or_else(|e| panic!("read LinksCreated failed: {e:?}")) as i64;
    let actual_error_count =
        read_integer_property(&response_reference, CorePropertyTypeName::ErrorCount)
            .unwrap_or_else(|ev| panic!("read ErrorCount failed: {ev:?}")) as i64;
    let actual_total_bundles =
        read_integer_property(&response_reference, CorePropertyTypeName::TotalBundles)
            .unwrap_or_else(|e| panic!("read TotalBundles failed: {e:?}")) as i64;
    let actual_total_loader_holons =
        read_integer_property(&response_reference, CorePropertyTypeName::TotalLoaderHolons)
            .unwrap_or_else(|e| panic!("read TotalLoaderHolons failed: {e:?}")) as i64;

    // Always dump error holons if present
    if actual_error_count > 0 {
        info!(
            "[loader-test] response reported {} error(s); dumping attached error holons...",
            actual_error_count
        );
        info!("{}", dump_error_holons_from_response(test_state, &response_reference));
    }

    // Dump full response if any expectation is off
    if actual_staged != expect_staged.0
        || actual_committed != expect_committed.0
        || actual_links_created != expect_links_created.0
        || actual_error_count != expect_errors.0
        || actual_total_bundles != expect_total_bundles.0
        || actual_total_loader_holons != expect_total_loader_holons.0
    {
        info!(
            "[loader-test] EXPECTED: staged={}, committed={}, links_created={}, errors={}, total_bundles={}, total_loader_holons={}",
            expect_staged.0, expect_committed.0, expect_links_created.0, expect_errors.0, expect_total_bundles.0, expect_total_loader_holons.0
        );
        info!(
            "[loader-test]   ACTUAL: staged={}, committed={}, links_created={}, errors={}, total_bundles={}, total_loader_holons={}",
            actual_staged, actual_committed, actual_links_created, actual_error_count, actual_total_bundles, actual_total_loader_holons
        );
        info!("{}", dump_full_response(test_state, &response_reference));
    }

    // Final assertions
    assert_eq!(
        actual_staged, expect_staged.0,
        "Expected HolonsStaged={}, got {}",
        expect_staged.0, actual_staged
    );
    assert_eq!(
        actual_committed, expect_committed.0,
        "Expected HolonsCommitted={}, got {}",
        expect_committed.0, actual_committed
    );
    assert_eq!(
        actual_links_created, expect_links_created.0,
        "Expected LinksCreated={}, got {}",
        expect_links_created.0, actual_links_created
    );
    assert_eq!(
        actual_error_count, expect_errors.0,
        "Expected ErrorCount={}, got {}",
        expect_errors.0, actual_error_count
    );
    assert_eq!(
        actual_total_bundles, expect_total_bundles.0,
        "Expected TotalBundles={}, got {}",
        expect_total_bundles.0, actual_total_bundles
    );
    assert_eq!(
        actual_total_loader_holons, expect_total_loader_holons.0,
        "Expected TotalLoaderHolons={}, got {}",
        expect_total_loader_holons.0, actual_total_loader_holons
    );
}
