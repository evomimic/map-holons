use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep};
use holons_core::dances::ResponseStatusCode;
use holons_core::reference_layer::{
    holon_operations_api, HolonsContextBehavior, ReadableHolon, TransientReference,
};
use holons_prelude::prelude::*;
use tracing::info;

// temporary workspace workaround to ensure the holons_loader crate is linked in tests
#[allow(dead_code)]
fn _build_anchor_holons_loader() {
    // Touching something stable so Cargo links the loader crate.
    let _ = holons_loader::CRATE_LINK; // e.g., an inert constant
}

/// Read an integer property from a transient response holon.
fn read_integer_property(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
    property: CorePropertyTypeName,
) -> Result<i64, HolonError> {
    match response.property_value(context, &property.as_property_name())? {
        Some(PropertyValue::IntegerValue(MapInteger(i))) => Ok(i),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected integer value for {:?}, got {:?}",
            property, other
        ))),
    }
}

/// Utility: dump all properties (name + value) on the HolonLoadResponse holon,
/// plus the important fields like response_status_code, error_count, etc.
fn dump_full_response(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
) -> String {
    let mut out = String::new();
    out.push_str("\n===== HolonLoadResponse dump =====\n");

    match response.get_raw_property_map(context) {
        Ok(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            // Sort by property name for stable diffs
            entries.sort_by(|(k1, _), (k2, _)| k1.0 .0.cmp(&k2.0 .0));

            for (pname, pvalue) in entries {
                let name = pname.0 .0;
                let val_str = match pvalue {
                    PropertyValue::StringValue(s) => format!("String({})", s.0),
                    PropertyValue::IntegerValue(MapInteger(i)) => format!("Int({})", i),
                    PropertyValue::BooleanValue(b) => format!("Bool({})", b.0),
                    PropertyValue::EnumValue(v) => format!("Enum({})", v.0),
                };
                out.push_str(&format!("  {} = {}\n", name, val_str));
            }
        }
        Err(e) => {
            out.push_str(&format!("  <failed to read property map: {:?}>\n", e));
        }
    }

    // Pull out common “loader” properties for quick eyeballing
    out.push_str("----- important (best-effort) -----\n");
    for pname in [
        CorePropertyTypeName::ErrorCount.as_property_name(),
        CorePropertyTypeName::HolonsStaged.as_property_name(),
        CorePropertyTypeName::HolonsCommitted.as_property_name(),
        CorePropertyTypeName::LinksCreated.as_property_name(),
        CorePropertyTypeName::DanceSummary.as_property_name(),
    ]
    .iter()
    {
        if let Ok(val_opt) = response.property_value(context, pname) {
            if let Some(val) = val_opt {
                out.push_str(&format!("  {} => {:?}\n", pname.0 .0, val));
            }
        }
    }

    out.push_str("===== end HolonLoadResponse dump =====\n");
    out
}

/// Utility: if the response holon has **error holons** attached via
/// `HasLoadError`, print each one’s properties. This is what will tell us the
/// *actual* Pass-2 resolver error.
fn dump_error_holons_from_response(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
) -> String {
    let mut out = String::new();

    // Try to follow the relationship. If it doesn't exist, just return empty.
    let rel_name = CoreRelationshipTypeName::HasLoadError;
    let collection_handle = match response.related_holons(context, &rel_name) {
        Ok(c) => c,
        Err(_) => return out, // no error holons, or link not present
    };

    let members = {
        let guard = match collection_handle.read() {
            Ok(g) => g,
            Err(_) => {
                out.push_str("[loader-test] <failed to read HasLoadError collection>\n");
                return out;
            }
        };
        guard.get_members().clone()
    };

    if members.is_empty() {
        return out;
    }

    out.push_str("\n===== Loader Error Holons (HasLoadError) =====\n");
    for (idx, error_ref) in members.into_iter().enumerate() {
        // Work on a detached copy so we can read its properties
        let error_transient = match error_ref.clone_holon(context) {
            Ok(t) => t,
            Err(e) => {
                out.push_str(&format!("  [{}] <failed to clone error holon: {:?}>\n", idx, e));
                continue;
            }
        };

        out.push_str(&format!("  --- error holon #{} ---\n", idx + 1));

        match error_transient.get_raw_property_map(context) {
            Ok(map) => {
                let mut entries: Vec<_> = map.into_iter().collect();
                entries.sort_by(|(k1, _), (k2, _)| k1.0 .0.cmp(&k2.0 .0));

                for (pname, pvalue) in entries {
                    out.push_str(&format!("    {} = {:?}\n", pname.0 .0, pvalue));
                }
            }
            Err(e) => {
                out.push_str(&format!("    <failed to read error holon properties: {:?}>\n", e));
            }
        }
    }
    out.push_str("===== end Loader Error Holons =====\n");
    out
}

/// Execute the `LoadHolons` step by initiating the LoadHolons dance (test → guest via TrustChannel),
/// then assert each response property on the returned response holon.
///
/// We also now *always* print any error holons that were attached via `HasLoadError`,
/// because the controller attaches real resolver errors there and it is the fastest
/// way to see *why* Pass-2 said `UnprocessableEntity`.
pub async fn execute_load_holons(
    test_state: &mut DanceTestExecutionState,
    bundle: TransientReference,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
) {
    info!("--- TEST STEP: Load Holons ---");
    let context = test_state.context();

    // Build the DanceRequest for the loader.
    let request = build_load_holons_dance_request(bundle)
        .unwrap_or_else(|e| panic!("build_load_holons_dance_request() failed: {e:?}"));

    // Initiate the dance using the test harness (TrustChannel-backed initiator).
    let dance_response = test_state.invoke_dance(request).await;

    // Convert the DanceResponse into a TransientReference for property assertions.
    let response_reference: TransientReference = match dance_response.body {
        ResponseBody::HolonReference(HolonReference::Transient(t)) => t,
        ResponseBody::HolonReference(other_ref) => {
            // If we got a non-transient HolonReference (e.g. staged/saved),
            // clone it into a transient so we can read properties the same way.
            other_ref.clone_holon(context).unwrap_or_else(|e| {
                panic!("LoadHolons returned non-transient reference and clone_holon failed: {e:?}")
            })
        }
        other => panic!("LoadHolons: expected ResponseBody::HolonReference, got {:?}", other),
    };

    // Read response properties from the returned HolonLoadResponse holon.
    let actual_staged =
        read_integer_property(context, &response_reference, CorePropertyTypeName::HolonsStaged)
            .unwrap_or_else(|e| panic!("read HolonsStaged failed: {e:?}")) as i64;
    let actual_committed =
        read_integer_property(context, &response_reference, CorePropertyTypeName::HolonsCommitted)
            .unwrap_or_else(|e| panic!("read HolonsCommitted failed: {e:?}")) as i64;
    let actual_links_created =
        read_integer_property(context, &response_reference, CorePropertyTypeName::LinksCreated)
            .unwrap_or_else(|e| panic!("read LinksCreated failed: {e:?}")) as i64;
    let actual_error_count =
        read_integer_property(context, &response_reference, CorePropertyTypeName::ErrorCount)
            .unwrap_or_else(|ev| panic!("read ErrorCount failed: {ev:?}")) as i64;

    // Always print any attached error holons if there are any.
    if actual_error_count > 0 {
        info!(
            "[loader-test] response reported {} error(s); dumping attached error holons...",
            actual_error_count
        );
        info!("{}", dump_error_holons_from_response(context, &response_reference));
    }

    // If *anything* is off, dump the whole response to make debugging fast.
    if actual_staged != expect_staged.0
        || actual_committed != expect_committed.0
        || actual_links_created != expect_links_created.0
        || actual_error_count != expect_errors.0
    {
        info!(
            "[loader-test] EXPECTED: staged={}, committed={}, links_created={}, errors={}",
            expect_staged.0, expect_committed.0, expect_links_created.0, expect_errors.0,
        );
        info!(
            "[loader-test]   ACTUAL: staged={}, committed={}, links_created={}, errors={}",
            actual_staged, actual_committed, actual_links_created, actual_error_count,
        );
        info!("{}", dump_full_response(context, &response_reference));
        // we already printed error holons above if any existed
    }

    // Final assertions (same as before).
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
}
