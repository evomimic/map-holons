use crate::shared_test::test_data_types::{DanceTestExecutionState, DanceTestStep};
use holons_core::dances::ResponseStatusCode;
use holons_core::reference_layer::{
    holon_operations_api, HolonsContextBehavior, ReadableHolon, TransientReference,
};
use holons_prelude::prelude::*;
use tracing::info;

/// Read a string property from a transient response holon.
fn read_string_property(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
    property: CorePropertyTypeName,
) -> Result<String, HolonError> {
    match response.property_value(context, &property.as_property_name())? {
        Some(PropertyValue::StringValue(s)) => Ok(s.0),
        other => Err(HolonError::InvalidParameter(format!(
            "Expected string value for {:?}, got {:?}",
            property, other
        ))),
    }
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

/// Execute the `LoadHolons` step by calling the client operation that delegates to the dancer,
/// which calls the guest-side loader controller. Then assert each response property.
pub async fn execute_load_holons(
    state: &mut DanceTestExecutionState,
    bundle: TransientReference,
    expect_status: ResponseStatusCode,
    expect_staged: MapInteger,
    expect_committed: MapInteger,
    expect_links_created: MapInteger,
    expect_errors: MapInteger,
) {
    info!("--- TEST STEP: Load Holons ---");
    let context = state.context();

    // Always exercise the dancer path (façade present). Panic on failure like other executors.
    let response_reference = load_holons(context, bundle, Some(state.dance()))
        .unwrap_or_else(|e| panic!("load_holons() failed: {e:?}"));

    let actual_status = match read_string_property(
        context,
        &response_reference,
        CorePropertyTypeName::ResponseStatusCode,
    ) {
        Ok(s) => s,
        Err(e) => {
            let props = dump_property_names(context, &response_reference);
            panic!(
                "read ResponseStatusCode failed: {e:?}\nResponse holon properties present: {}",
                props
            );
        }
    };

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
            .unwrap_or_else(|e| panic!("read ErrorCount failed: {e:?}")) as i64;

    // Compare against expectations. We compare status by Debug-printing the enum ("OK", etc.).
    let expect_status_string = format!("{:?}", expect_status);

    assert_eq!(
        actual_status, expect_status_string,
        "Expected ResponseStatusCode={}, got {}",
        expect_status_string, actual_status
    );
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

/// Utility: dump all property names on a transient holon (for debugging).
fn dump_property_names(
    context: &dyn HolonsContextBehavior,
    response: &TransientReference,
) -> String {
    // Best-effort; ignore errors while dumping
    if let Ok(map) = response.get_raw_property_map(context) {
        let mut names: Vec<String> = map
            .keys()
            .map(|pname| pname.0 .0.clone()) // PropertyName(MapString(...))
            .collect();
        names.sort();
        format!("[{}]", names.join(", "))
    } else {
        "<unavailable>".to_string()
    }
}
