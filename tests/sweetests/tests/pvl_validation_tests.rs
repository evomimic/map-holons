//! Conductor-level PVL tests that exercise authoring directly below the dance layer.
//!
//! PR 2 intentionally adds only the property-count rejection here. Size, decode, and
//! non-canonical rejection paths remain adapter-level tests until broader conductor
//! coverage is added in PVL PR 8.

use holochain::prelude::Record;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{assert_commit_rejected_with_pvl, setup_test_conductor};
use integrity_core_types::HolonNodeModel;

const EXPECTED_PROPERTY_COUNT_REJECTION: &str = "MAP-PVL-1101: property count exceeds 256";
const EXPECTED_EMPTY_PROPERTY_NAME_REJECTION: &str = "MAP-PVL-1102: property name is empty";

/// Proves that the Integrity callback's PVL message reaches the authoring
/// conductor path without being replaced by an implicit guest-error message.
#[tokio::test(flavor = "multi_thread")]
async fn rejects_holon_node_with_257_properties_using_exact_pvl_message() {
    let backend = setup_test_conductor().await;
    let property_map = (0..257)
        .map(|index| {
            (
                format!("property-{index:03}").to_property_name(),
                MapString("value".to_string()).to_base_value(),
            )
        })
        .collect();

    // HolonNodeModel has the same serialized field layout as the guest HolonNode.
    // Passing it directly keeps this test independent of guest-only Rust types while
    // exercising the coordinator's real create_entry authoring path.
    let holon_node = HolonNodeModel::new(None, property_map);
    let result = backend
        .conductor
        .call_fallible::<_, Record>(&backend.cell.zome("holons"), "create_holon_node", holon_node)
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_PROPERTY_COUNT_REJECTION);
}

/// Proves that property-level PVL violations use the same exact authoring rejection path.
#[tokio::test(flavor = "multi_thread")]
async fn rejects_empty_property_name_using_exact_pvl_message() {
    let backend = setup_test_conductor().await;
    let property_map = [("".to_property_name(), MapString("value".to_string()).to_base_value())]
        .into_iter()
        .collect();
    let holon_node = HolonNodeModel::new(None, property_map);
    let result = backend
        .conductor
        .call_fallible::<_, Record>(&backend.cell.zome("holons"), "create_holon_node", holon_node)
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_EMPTY_PROPERTY_NAME_REJECTION);
}
