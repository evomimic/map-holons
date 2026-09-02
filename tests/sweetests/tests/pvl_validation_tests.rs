//! Conductor-level PVL tests that exercise authoring directly below the dance layer.
//!
//! These tests distinguish supported production behavior, typed coordinator preflight, and
//! peer-authored Integrity enforcement through narrowly scoped raw-authoring probes. Keeping those
//! paths distinct prevents a coordinator guest error from being mistaken for consensus validation
//! coverage or a probe operation from being mistaken for supported setup.
//! Every test conductor setup also completes genesis/CreateAgent, providing the positive joining
//! path while the focused adapter tests pin its predecessor policy and dependency behavior.

use core_types::{
    CanonicalKey, DeleteSmartLinkOutcome, HolonId, HolonWriteRequest, PreparedSmartLink,
    PutSmartLinkOutcome, StoredHolonNode,
};
use holochain::prelude::{ActionHash, Record};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    assert_commit_rejected_with_message, assert_commit_rejected_with_pvl,
    assert_preflight_rejected_with_pvl, assert_unanchored_ordinary_session_is_rejected,
    setup_probe_enabled_conductor, setup_test_conductor,
};
use holons_test::MockConductorConfig;
use integrity_core_types::{HolonNodeModel, LocalId, RelationshipName};
use serde::Serialize;

const ZOME: &str = "holons";
const PROBE_ZOME: &str = "holons_test_probes";
const EXPECTED_PROPERTY_COUNT_REJECTION: &str = "MAP-PVL-1101: property count exceeds 256";
const EXPECTED_EMPTY_PROPERTY_NAME_REJECTION: &str = "MAP-PVL-1102: property name is empty";
const EXPECTED_EMPTY_RELATIONSHIP_REJECTION: &str = "MAP-PVL-2101: relationship name is empty";
const EXPECTED_MALFORMED_SMARTLINK_REJECTION: &str =
    "MAP-PVL-2001: malformed SmartLink (invalid discriminant at TagHeader)";

// These serde mirrors are pinned to the closed probe inputs in `holons_test_probes`. The root-index
// enum also serializes compatibly with the same-named production `LinkTypes` variants for the
// retained path getter used by the bootstrap assertion.
#[derive(Clone, Copy, Debug, Serialize)]
enum RootIndexLinkType {
    AllHolonNodes,
    LocalHolonSpace,
}

#[derive(Debug, Serialize)]
struct NonCanonicalBaseInput {
    link_type: RootIndexLinkType,
    path: String,
    target_id: LocalId,
}

#[derive(Debug, Serialize)]
struct RootIndexUpdateTargetInput {
    link_type: RootIndexLinkType,
    target_id: LocalId,
}

fn node(title: &str) -> HolonNodeModel {
    HolonNodeModel::new(
        [("title".to_property_name(), MapString(title.to_string()).to_base_value())]
            .into_iter()
            .collect(),
    )
}

fn action_hash(local_id: &LocalId) -> ActionHash {
    ActionHash::try_from_raw_39(local_id.0.clone()).expect("persisted id must be an action hash")
}

async fn publish_root(backend: &MockConductorConfig, title: &str) -> StoredHolonNode {
    backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishRoot { holon_node: node(title) },
        )
        .await
}

async fn publish_version(
    backend: &MockConductorConfig,
    title: &str,
    predecessor: LocalId,
) -> StoredHolonNode {
    backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishVersion {
                holon_node: node(title),
                predecessor_ids: vec![predecessor],
            },
        )
        .await
}

fn prepared_smartlink(
    source: LocalId,
    target: LocalId,
    relationship_name: &str,
) -> PreparedSmartLink {
    PreparedSmartLink {
        source_id: source,
        target_id: HolonId::Local(target),
        relationship_name: RelationshipName(MapString(relationship_name.to_string())),
        canonical_key: CanonicalKey::new("").expect("empty canonical key is valid"),
        occurrence_id: None,
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    }
}

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

    let holon_node = HolonNodeModel::new(property_map);
    let result = backend
        .conductor
        .call_fallible::<_, StoredHolonNode>(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishRoot { holon_node },
        )
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
        .call_fallible::<_, StoredHolonNode>(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishRoot { holon_node },
        )
        .await;

    assert_preflight_rejected_with_pvl(result, EXPECTED_EMPTY_PROPERTY_NAME_REJECTION);
}

/// Proves that Integrity independently rejects malformed HolonNode content after preflight made
/// that operation unreachable through production persistence APIs.
#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_holon_node_with_257_properties_using_exact_pvl_message() {
    let backend = setup_probe_enabled_conductor().await;
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
            &backend.cell.zome(PROBE_ZOME),
            "holon_storage_author_create_for_test",
            holon_node,
        )
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_PROPERTY_COUNT_REJECTION);
}

/// Proves the typed coordinator rejects an encode-valid SmartLink before any DHT lookup.
#[tokio::test(flavor = "multi_thread")]
async fn smartlink_preflight_rejects_empty_relationship_using_exact_pvl_message() {
    let backend = setup_test_conductor().await;
    let source = publish_root(&backend, "source").await;
    let target = publish_root(&backend, "target").await;
    let result = backend
        .conductor
        .call_fallible::<_, PutSmartLinkOutcome>(
            &backend.cell.zome(ZOME),
            "smartlink_put",
            prepared_smartlink(source.version_id().clone(), target.version_id().clone(), ""),
        )
        .await;

    assert_preflight_rejected_with_pvl(result, EXPECTED_EMPTY_RELATIONSHIP_REJECTION);
}

/// Proves Integrity independently rejects malformed peer-authored Tag v1 bytes.
#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_malformed_smartlink_tag_using_exact_pvl_message() {
    let backend = setup_probe_enabled_conductor().await;
    let source = publish_root(&backend, "source").await;
    let target = publish_root(&backend, "target").await;
    let result = backend
        .conductor
        .call_fallible::<_, LocalId>(
            &backend.cell.zome(PROBE_ZOME),
            "smartlink_author_raw_tag_for_test",
            (source.version_id().clone(), target.version_id().clone(), vec![0; 3]),
        )
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_MALFORMED_SMARTLINK_REJECTION);
}

#[tokio::test(flavor = "multi_thread")]
async fn valid_root_version_and_smartlink_create_delete_are_accepted() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "root").await;
    let version = publish_version(&backend, "version", root.version_id().clone()).await;
    assert_eq!(
        version.version_metadata.lineage_root().into_local_id(),
        root.version_id().clone(),
        "the accepted version must remain root-addressed"
    );

    let outcome: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "smartlink_put",
            prepared_smartlink(
                root.version_id().clone(),
                version.version_id().clone(),
                "Successor",
            ),
        )
        .await;
    let smartlink_id = match outcome {
        PutSmartLinkOutcome::Inserted(id) => id,
        other => panic!("expected SmartLink insertion, got {other:?}"),
    };
    let deleted: DeleteSmartLinkOutcome =
        backend.conductor.call(&backend.cell.zome(ZOME), "smartlink_delete", smartlink_id).await;
    assert_eq!(deleted, DeleteSmartLinkOutcome::Deleted);
}

#[tokio::test(flavor = "multi_thread")]
async fn unanchored_ordinary_session_requires_a_persisted_local_holon_space() {
    let backend = setup_test_conductor().await;
    assert_unanchored_ordinary_session_is_rejected(&backend).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn canonical_all_holon_nodes_creation_uses_production_persistence() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "all-index-target").await;
    let indexed: Vec<Record> =
        backend.conductor.call(&backend.cell.zome(ZOME), "get_all_holon_nodes", ()).await;

    assert!(
        indexed.iter().any(|record| record.action_address() == &action_hash(root.version_id())),
        "PublishRoot must add the persisted lineage root to AllHolonNodes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_noncanonical_infrastructure_bases() {
    let backend = setup_probe_enabled_conductor().await;
    let target = publish_root(&backend, "target").await;

    for (link_type, link_name, canonical_path) in [
        (RootIndexLinkType::AllHolonNodes, "AllHolonNodes", "all_holon_nodes"),
        (RootIndexLinkType::LocalHolonSpace, "LocalHolonSpace", "local_holon_space"),
    ] {
        let result = backend
            .conductor
            .call_fallible::<_, LocalId>(
                &backend.cell.zome(PROBE_ZOME),
                "infrastructure_author_noncanonical_base_for_test",
                NonCanonicalBaseInput {
                    link_type,
                    path: "noncanonical_path".into(),
                    target_id: target.version_id().clone(),
                },
            )
            .await;
        assert_commit_rejected_with_message(
            result,
            &format!("{link_name} links must use the canonical `{canonical_path}` path base"),
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_update_targets_for_root_infrastructure_indexes() {
    let backend = setup_probe_enabled_conductor().await;
    let root = publish_root(&backend, "root").await;
    let version = publish_version(&backend, "version", root.version_id().clone()).await;

    for (link_type, link_name) in [
        (RootIndexLinkType::AllHolonNodes, "AllHolonNodes"),
        (RootIndexLinkType::LocalHolonSpace, "LocalHolonSpace"),
    ] {
        let result = backend
            .conductor
            .call_fallible::<_, LocalId>(
                &backend.cell.zome(PROBE_ZOME),
                "infrastructure_author_update_target_for_test",
                RootIndexUpdateTargetInput { link_type, target_id: version.version_id().clone() },
            )
            .await;
        assert_commit_rejected_with_message(
            result,
            &format!("{link_name} links must target a HolonNode lineage-root Create action"),
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn all_holon_nodes_delete_is_rejected() {
    let backend = setup_probe_enabled_conductor().await;
    let target = publish_root(&backend, "target").await;

    let result = backend
        .conductor
        .call_fallible::<_, LocalId>(
            &backend.cell.zome(PROBE_ZOME),
            "all_holon_nodes_delete_for_test",
            target.version_id().clone(),
        )
        .await;
    assert_commit_rejected_with_message(result, "AllHolonNodes links cannot be deleted");
}
