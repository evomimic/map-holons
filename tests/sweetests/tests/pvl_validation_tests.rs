//! Conductor-level PVL tests that exercise authoring directly below the dance layer.
//!
//! Coordinator tests prove typed preflight behavior, while narrowly scoped raw-authoring probes
//! separately prove Integrity enforcement. Keeping those paths distinct prevents a coordinator
//! guest error from being mistaken for consensus validation coverage.

use holochain::prelude::Record;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    assert_commit_rejected_with_pvl, assert_preflight_rejected_with_pvl, setup_test_conductor,
};
use integrity_core_types::{HolonNodeModel, LocalId};

const EXPECTED_PROPERTY_COUNT_REJECTION: &str = "MAP-PVL-1101: property count exceeds 256";
const EXPECTED_EMPTY_PROPERTY_NAME_REJECTION: &str = "MAP-PVL-1102: property name is empty";

/// Proves that coordinator preflight rejects an invalid canonical model before authoring.
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

    // HolonNodeModel has the same serialized field layout as the guest HolonNode. Passing it
    // directly exercises the canonical coordinator boundary without depending on guest types.
    let holon_node = HolonNodeModel::new(property_map);
    let result = backend
        .conductor
        .call_fallible::<_, Record>(&backend.cell.zome("holons"), "create_holon_node", holon_node)
        .await;

    assert_preflight_rejected_with_pvl(result, EXPECTED_PROPERTY_COUNT_REJECTION);
}

/// Proves that property-level PVL violations use the same exact preflight rejection path.
#[tokio::test(flavor = "multi_thread")]
async fn rejects_empty_property_name_using_exact_pvl_message() {
    let backend = setup_test_conductor().await;
    let property_map = [("".to_property_name(), MapString("value".to_string()).to_base_value())]
        .into_iter()
        .collect();
    let holon_node = HolonNodeModel::new(property_map);
    let result = backend
        .conductor
        .call_fallible::<_, Record>(&backend.cell.zome("holons"), "create_holon_node", holon_node)
        .await;

    assert_preflight_rejected_with_pvl(result, EXPECTED_EMPTY_PROPERTY_NAME_REJECTION);
}

/// Proves that Integrity independently rejects malformed HolonNode content after preflight made
/// that operation unreachable through production persistence APIs.
#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_holon_node_with_257_properties_using_exact_pvl_message() {
    let backend = setup_test_conductor().await;
    let property_map = (0..257)
        .map(|index| {
            (
                format!("property-{index:03}").to_property_name(),
                MapString("value".to_string()).to_base_value(),
            )
        })
        .collect();
    let holon_node = HolonNodeModel::new(property_map);

    let result = backend
        .conductor
        .call_fallible::<_, LocalId>(
            &backend.cell.zome("holons"),
            "holon_storage_author_create_for_test",
            holon_node,
        )
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_PROPERTY_COUNT_REJECTION);
}
