use holons_core::dances::ResponseStatusCode;
use holons_core::reference_layer::{
    holon_operations_api, HolonsContextBehavior, ReadableHolon, TransientReference,
};
use holons_prelude::prelude::*;
use holons_test::TestExecutionState;
use tracing::info;

/// Read an integer property from a transient response holon.
fn read_integer_property(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
    property: CorePropertyTypeName,
) -> Result<i64, HolonError> {
    match response.property_value(context, &property)? {
        Some(PropertyValue::IntegerValue(MapInteger(i))) => Ok(i),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected integer value for {:?}, got {:?}",
            property, other
        ))),
    }
}
/// Dump the `EssentialHolonContent` for any holon-like reference (including
/// `TransientReference`, `StagedReference`, or `SavedReference`) in a stable,
/// human-readable format.
///
/// This helper is used primarily by sweetests during holon-loader testing,
/// where TypeDescriptors are not yet available and we cannot rely on
/// `property_value()` calls using known property names.
///
/// Instead of querying individual properties, `essential_content()` returns:
///   * the complete `property_map` (all properties discovered on the holon),
///   * the holon’s key, if present,
///   * accumulated read-path errors for debugging.
///
/// The output is alphabetically sorted for stable diffs and easy debugging.
///
/// This function should **never** be used by production code; it is strictly
/// a test/sweetest debugging aid for introspecting holons before descriptors
/// exist or during early-bootstrapping scenarios.
fn dump_essential(
    state: &mut TestExecutionState,
    holon_reference: &impl ReadableHolon,
) -> String {
    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    // Attempt to get essential content (properties, key, errors)
    let essential_content_result = holon_reference.essential_content(context);

    // Early return if reading essential content failed
    let essential_content = match essential_content_result {
        Ok(content) => content,
        Err(error) => {
            return format!("<error reading essential content: {:?}>", error);
        }
    };

    let mut output = String::new();
    output.push_str("\n==== EssentialContent ===\n");

    // Sort properties by property name
    let mut sorted_entries: Vec<_> = essential_content.property_map.iter().collect();
    sorted_entries.sort_by(|(name_a, _), (name_b, _)| name_a.0 .0.cmp(&name_b.0 .0));

    for (property_name, property_value) in sorted_entries {
        output.push_str(&format!("  {} = {:?}\n", property_name.0 .0, property_value));
    }

    // Include optional key
    if let Some(key_value) = essential_content.key.clone() {
        output.push_str(&format!("  (key) = {}\n", key_value.0));
    }

    // Include any validation or resolver errors
    if !essential_content.errors.is_empty() {
        output.push_str("  errors:\n");
        for error in essential_content.errors {
            output.push_str(&format!("    - {:?}\n", error));
        }
    }

    output.push_str("==== end ====\n");
    output
}

/// Utility: dump all properties (name + value) on the HolonLoadResponse holon,
/// plus the important fields like response_status_code, error_count, etc.
fn dump_full_response(
    state: &mut TestExecutionState,
    response: &TransientReference,
) -> String {
    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    let mut out = String::new();
    out.push_str("\n===== HolonLoadResponse dump =====\n");

    // 1. Full essential-content dump (sorted, complete property map)
    out.push_str(&dump_essential(state, response));

    // 2. Pull out common loader-specific properties (best-effort)
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
        if let Ok(Some(val)) = response.property_value(context, p_name) {
            out.push_str(&format!("  {} => {:?}\n", p_name.0 .0, val));
        }
    }

    out.push_str("===== end HolonLoadResponse dump =====\n");
    out
}

/// Utility: if the response holon has **error holons** attached via
/// `HasLoadError`, print each one’s properties. This is what will tell us the
/// *actual* Pass-2 resolver error.
fn dump_error_holons_from_response(
    state: &mut TestExecutionState,
    response_reference: &TransientReference,
) -> String {
    let ctx_arc = state.context();
    let context = ctx_arc.as_ref();

    let mut output = String::new();

    // Try to follow the HasLoadError relationship.
    let relationship_name = CoreRelationshipTypeName::HasLoadError;
    let collection_handle = match response_reference.related_holons(context, &relationship_name) {
        Ok(collection) => collection,
        Err(_) => return output, // No error holons present.
    };

    // Get members from the collection.
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
        // Clone the holon so we can read its properties safely.
        let transient_reference = match holon_reference.clone_holon(context) {
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

        // Use unified essential-content dump
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

/// Execute the `LoadHolons` step by initiating the LoadHolons dance (test → guest via TrustChannel),
/// then assert each response property on the returned response holon.
///
/// We also now *always* print any error holons that were attached via `HasLoadError`,
/// because the controller attaches real resolver errors there and it is the fastest
/// way to see *why* Pass-2 said `UnprocessableEntity`.
pub async fn execute_load_holons(
    test_state: &mut TestExecutionState,
    load_set_reference: TransientReference,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
    expect_total_bundles: MapInteger,
    expect_total_loader_holons: MapInteger,
) {
    info!("--- TEST STEP: Load Holons ---");
    let ctx_arc = test_state.context(); // Arc lives until end of scope
    let context = ctx_arc.as_ref();

    // Build the DanceRequest for the loader.
    let request = build_load_holons_dance_request(load_set_reference)
        .unwrap_or_else(|e| panic!("build_load_holons_dance_request() failed: {e:?}"));

    // Initiate the dance using the test harness (TrustChannel-backed initiator).
    let dance_initiator = context.get_dance_initiator().unwrap();
    let dance_response = dance_initiator.initiate_dance(context, request).await;

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
    let actual_total_bundles =
        read_integer_property(context, &response_reference, CorePropertyTypeName::TotalBundles)
            .unwrap_or_else(|e| panic!("read TotalBundles failed: {e:?}")) as i64;
    let actual_total_loader_holons = read_integer_property(
        context,
        &response_reference,
        CorePropertyTypeName::TotalLoaderHolons,
    )
    .unwrap_or_else(|e| panic!("read TotalLoaderHolons failed: {e:?}"))
        as i64;

    // Always print any attached error holons if there are any.
    if actual_error_count > 0 {
        info!(
            "[loader-test] response reported {} error(s); dumping attached error holons...",
            actual_error_count
        );
        info!("{}", dump_error_holons_from_response(test_state, &response_reference));
    }

    // If *anything* is off, dump the whole response to make debugging fast.
    if actual_staged != expect_staged.0
        || actual_committed != expect_committed.0
        || actual_links_created != expect_links_created.0
        || actual_error_count != expect_errors.0
        || actual_total_bundles != expect_total_bundles.0
        || actual_total_loader_holons != expect_total_loader_holons.0
    // || true // "true" forces dump even if no missed expectations
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
        // we already printed error holons above if any existed
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
