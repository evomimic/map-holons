use core_types::ContentSet;
use holons_prelude::prelude::*;
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use tracing::info;

use holons_test::{ExpectedLoadStatus, TestExecutionState};

fn read_int_property(reference: &TransientReference, property: CorePropertyTypeName) -> i64 {
    match reference.property_value(&property) {
        Ok(Some(PropertyValue::IntegerValue(MapInteger(i)))) => i,
        Ok(Some(other)) => panic!("Expected integer for {:?}, got {:?}", property, other),
        Ok(None) => panic!("Property {:?} missing on response holon", property),
        Err(err) => panic!("Failed to read {:?} from response holon: {:?}", property, err),
    }
}

fn read_string_property(reference: &TransientReference, property: CorePropertyTypeName) -> String {
    match reference.property_value(&property) {
        Ok(Some(PropertyValue::StringValue(MapString(s)))) => s,
        Ok(Some(other)) => panic!("Expected string for {:?}, got {:?}", property, other),
        Ok(None) => panic!("Property {:?} missing on response holon", property),
        Err(err) => panic!("Failed to read {:?} from response holon: {:?}", property, err),
    }
}

/// Execute public LoadHolons ingress end-to-end: dispatch MAP command,
/// validate/parse files through the loader client, run the dance, and assert
/// loader response properties.
pub async fn execute_load_holons_client(
    test_state: &mut TestExecutionState,
    content_set: ContentSet,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
    expect_total_bundles: MapInteger,
    expect_total_loader_holons: MapInteger,
    expect_status: ExpectedLoadStatus,
) {
    let context = test_state.context();

    let command = MapCommand::Transaction(TransactionCommand {
        context: context.clone(),
        action: TransactionAction::LoadHolons { content_set },
    });
    let result = test_state
        .dispatch_command(command, "load_holons_client")
        .await
        .unwrap_or_else(|e| panic!("load_holons_client failed: {e:?}"));

    let response_reference = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("LoadHolons: expected Reference(Transient), got {other:?}"),
    };

    let staged = read_int_property(&response_reference, CorePropertyTypeName::HolonsStaged);
    let committed = read_int_property(&response_reference, CorePropertyTypeName::HolonsCommitted);
    let links_created = read_int_property(&response_reference, CorePropertyTypeName::LinksCreated);
    let errors = read_int_property(&response_reference, CorePropertyTypeName::ErrorCount);
    let total_bundles = read_int_property(&response_reference, CorePropertyTypeName::TotalBundles);
    let total_loader_holons =
        read_int_property(&response_reference, CorePropertyTypeName::TotalLoaderHolons);
    let commit_status =
        read_string_property(&response_reference, CorePropertyTypeName::LoadCommitStatus);

    let full_dump = dump_full_response(&response_reference);
    let error_dump = if errors > 0 {
        dump_error_holons_from_response(&response_reference)
    } else {
        String::new()
    };

    info!("[loader-client] response_full_dump:\n{}", full_dump);
    if !error_dump.is_empty() {
        info!("[loader-client] response_error_dump:\n{}", error_dump);
    }
    info!(
        "[loader-client] metrics observed: staged={}, committed={}, links_created={}, errors={}, total_bundles={}, total_loader_holons={}, status={}; expected: staged={}, committed={}, links_created={}, errors={}, total_bundles={}, total_loader_holons={}, status={}",
        staged,
        committed,
        links_created,
        errors,
        total_bundles,
        total_loader_holons,
        commit_status,
        expect_staged.0,
        expect_committed.0,
        expect_links_created.0,
        expect_errors.0,
        expect_total_bundles.0,
        expect_total_loader_holons.0,
        expect_status
    );

    let expected_status_string = expect_status.to_string();
    let mut mismatches = Vec::new();
    if staged != expect_staged.0 {
        mismatches.push(format!("HolonsStaged expected {}, got {}", expect_staged.0, staged));
    }
    if committed != expect_committed.0 {
        mismatches.push(format!("HolonsCommitted expected {}, got {}", expect_committed.0, committed));
    }
    if links_created != expect_links_created.0 {
        mismatches.push(format!(
            "LinksCreated expected {}, got {}",
            expect_links_created.0, links_created
        ));
    }
    if errors != expect_errors.0 {
        mismatches.push(format!("ErrorCount expected {}, got {}", expect_errors.0, errors));
    }
    if total_bundles != expect_total_bundles.0 {
        mismatches.push(format!(
            "TotalBundles expected {}, got {}",
            expect_total_bundles.0, total_bundles
        ));
    }
    if total_loader_holons != expect_total_loader_holons.0 {
        mismatches.push(format!(
            "TotalLoaderHolons expected {}, got {}",
            expect_total_loader_holons.0, total_loader_holons
        ));
    }
    if commit_status != expected_status_string {
        mismatches.push(format!(
            "LoadCommitStatus expected {}, got {}",
            expected_status_string, commit_status
        ));
    }

    if !mismatches.is_empty() {
        let mut report = String::new();
        report.push_str("LoadHolons expected ");
        if expect_errors.0 == 0 {
            report.push_str("success");
        } else {
            report.push_str(&format!("{} errors", expect_errors.0));
        }
        report.push_str(&format!(
            " but loader returned {} errors.\n",
            errors
        ));
        report.push_str("Mismatches:\n");
        for mismatch in &mismatches {
            report.push_str("  - ");
            report.push_str(mismatch);
            report.push('\n');
        }
        report.push_str("\n");
        report.push_str(&full_dump);
        if !error_dump.is_empty() {
            report.push('\n');
            report.push_str(&error_dump);
        }
        panic!("{report}");
    }
}

/// Utility: dump all properties on the response holon plus key loader fields.
fn dump_full_response(response: &TransientReference) -> String {
    let mut out = String::new();
    out.push_str("\n===== HolonLoadResponse dump =====\n");

    // Full essential-content dump (sorted, complete property map)
    out.push_str(&dump_essential(response));

    out.push_str("===== end HolonLoadResponse dump =====\n");
    out
}

/// Dump attached error holons (HasLoadError) for quick diagnostics.
fn dump_error_holons_from_response(response_reference: &TransientReference) -> String {
    let mut output = String::new();

    let relationship_name = CoreRelationshipTypeName::HasLoadError;
    let collection_handle = match response_reference.related_holons(&relationship_name) {
        Ok(collection) => collection,
        Err(_) => return output,
    };

    let members = match collection_handle.read() {
        Ok(guard) => guard.get_members().clone(),
        Err(_) => {
            output.push_str("[loader-client] <failed to read HasLoadError collection>\n");
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

        let essential_dump = dump_essential(&transient_reference);
        for line in essential_dump.lines() {
            output.push_str("    ");
            output.push_str(line);
            output.push('\n');
        }
    }

    output.push_str("===== end Loader Error Holons =====\n");
    output
}

/// Compact dump of essential holon content (property map, key, errors).
fn dump_essential(holon_reference: &impl ReadableHolon) -> String {
    let essential_content_result = holon_reference.essential_content();

    let essential_content = match essential_content_result {
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
