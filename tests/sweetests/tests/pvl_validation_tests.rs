//! Conductor-level PVL tests that exercise authoring directly below the dance layer.
//!
//! Coordinator tests prove typed preflight behavior, while narrowly scoped raw-authoring probes
//! separately prove Integrity enforcement. Keeping those paths distinct prevents a coordinator
//! guest error from being mistaken for consensus validation coverage.
//! Every test conductor setup also completes genesis/CreateAgent, providing the positive joining
//! path while the focused adapter tests pin its predecessor policy and dependency behavior.

use core_types::{
    CanonicalKey, DeleteSmartLinkOutcome, HolonId, HolonWriteRequest, PreparedSmartLink,
    PutSmartLinkOutcome, StoredHolonNode,
};
use hdi::prelude::Path;
use holochain::prelude::ActionHash;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    assert_commit_rejected_with_message, assert_commit_rejected_with_pvl,
    assert_preflight_rejected_with_pvl, setup_probe_enabled_conductor, setup_test_conductor,
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

#[derive(Clone, Copy, Debug, Serialize)]
enum LinkTypeInput {
    AllHolonNodes,
    LocalHolonSpace,
    /// Retired in Storage SL5a (#631). Kept here with no guest counterpart so
    /// `obsolete_updates_link_type_is_no_longer_addressable` can prove the ingress refuses it.
    HolonNodeUpdates,
}

#[derive(Debug, Serialize)]
struct CreatePathInput {
    path: Path,
    link_type: LinkTypeInput,
    target_holon_node_hash: ActionHash,
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

async fn create_path(
    backend: &MockConductorConfig,
    path: &str,
    link_type: LinkTypeInput,
    target: &LocalId,
) -> Result<ActionHash, holochain::conductor::api::error::ConductorApiError> {
    backend
        .conductor
        .call_fallible(
            &backend.cell.zome(ZOME),
            "create_path_to_holon_node",
            CreatePathInput {
                path: Path::from(path),
                link_type,
                target_holon_node_hash: action_hash(target),
            },
        )
        .await
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
async fn canonical_infrastructure_creates_and_local_space_delete_are_accepted() {
    let backend = setup_test_conductor().await;
    let all_target = publish_root(&backend, "all-index-target").await;
    create_path(&backend, "all_holon_nodes", LinkTypeInput::AllHolonNodes, all_target.version_id())
        .await
        .expect("canonical AllHolonNodes create must be accepted");

    let local_target = publish_root(&backend, "local-space-target").await;
    create_path(
        &backend,
        "local_holon_space",
        LinkTypeInput::LocalHolonSpace,
        local_target.version_id(),
    )
    .await
    .expect("canonical LocalHolonSpace create must be accepted");

    let _: ActionHash = backend
        .conductor
        .call(&backend.cell.zome(ZOME), "delete_holon_node", action_hash(local_target.version_id()))
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn noncanonical_infrastructure_bases_are_rejected_at_the_public_ingress() {
    let backend = setup_test_conductor().await;
    let target = publish_root(&backend, "target").await;

    for (link_type, link_name, canonical_path) in [
        (LinkTypeInput::AllHolonNodes, "AllHolonNodes", "all_holon_nodes"),
        (LinkTypeInput::LocalHolonSpace, "LocalHolonSpace", "local_holon_space"),
    ] {
        let result =
            create_path(&backend, "noncanonical_path", link_type, target.version_id()).await;
        assert_commit_rejected_with_message(
            result,
            &format!("{link_name} links must use the canonical `{canonical_path}` path base"),
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn infrastructure_root_indexes_reject_update_targets_at_the_public_ingress() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "root").await;
    let version = publish_version(&backend, "version", root.version_id().clone()).await;

    for (link_type, link_name, canonical_path) in [
        (LinkTypeInput::AllHolonNodes, "AllHolonNodes", "all_holon_nodes"),
        (LinkTypeInput::LocalHolonSpace, "LocalHolonSpace", "local_holon_space"),
    ] {
        let result = create_path(&backend, canonical_path, link_type, version.version_id()).await;
        assert_commit_rejected_with_message(
            result,
            &format!("{link_name} links must target a HolonNode lineage-root Create action"),
        );
    }
}

/// The obsolete revision index is unreachable, not merely rejected.
///
/// Storage SL5a removed `HolonNodeUpdates` from `LinkTypes`, so the public ingress can no longer
/// deserialize the name into a link type at all — a stronger guarantee than the validation
/// rejection it replaces. The failure is an HDK deserialization error, so this asserts only that
/// the call fails; its wording is not ours to pin.
#[tokio::test(flavor = "multi_thread")]
async fn obsolete_updates_link_type_is_no_longer_addressable() {
    let backend = setup_test_conductor().await;
    let target = publish_root(&backend, "target").await;

    let result = create_path(
        &backend,
        "arbitrary_obsolete_base",
        LinkTypeInput::HolonNodeUpdates,
        target.version_id(),
    )
    .await;

    assert!(
        result.is_err(),
        "the retired HolonNodeUpdates link type must not be addressable through the ingress"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn all_holon_nodes_delete_is_rejected() {
    let backend = setup_probe_enabled_conductor().await;
    let target = publish_root(&backend, "target").await;

    let all_link =
        create_path(&backend, "all_holon_nodes", LinkTypeInput::AllHolonNodes, target.version_id())
            .await
            .expect("canonical AllHolonNodes create must succeed before its delete is tested");
    let result = backend
        .conductor
        .call_fallible::<_, LocalId>(
            &backend.cell.zome(PROBE_ZOME),
            "all_holon_nodes_delete_for_test",
            LocalId(all_link.get_raw_39().to_vec()),
        )
        .await;
    assert_commit_rejected_with_message(result, "AllHolonNodes links cannot be deleted");
}
