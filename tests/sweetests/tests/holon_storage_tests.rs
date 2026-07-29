//! Live-conductor sweettests for the version-aware holon node storage API (Issue #607).
//!
//! These bypass the dance DSL: they spin up a conductor with `setup_test_conductor()` and call
//! the `holon_storage_*` zome externs directly, asserting the real DHT outcomes that pure unit
//! tests cannot reach — which substrate action each write intent becomes, what the record-derived
//! metadata says afterward, and how the batch read behaves positionally.
//!
//! The lineage rules themselves (root resolution, mismatch rejection) are unit-tested in
//! `core_types::holon_storage`. What is proven here is that they are wired to real `Create` and
//! root-addressed `Update` actions, and that Integrity accepts the topology this layer produces
//! while rejecting the topology it refuses to produce.

use base_types::{BaseValue, MapString};
use core_types::{HolonWriteRequest, LineageId, StoredHolonNode};
use holochain::prelude::ActionHash;
use holons_test::harness::helpers::{assert_commit_rejected_with_pvl, setup_test_conductor};
use holons_test::MockConductorConfig;
use integrity_core_types::{HolonNodeModel, LocalId, PropertyName};
use std::collections::BTreeMap;

const ZOME: &str = "holons";

/// The Integrity rejection for an update that does not target a lineage-root `Create`.
const EXPECTED_UPDATE_TARGET_REJECTION: &str = "MAP-PVL-1301: update target is invalid";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn node(title: &str) -> HolonNodeModel {
    HolonNodeModel::new(BTreeMap::from([(
        PropertyName(MapString("title".to_string())),
        BaseValue::StringValue(MapString(title.to_string())),
    )]))
}

fn title_of(stored: &StoredHolonNode) -> String {
    match stored.holon_node.property_map.get(&PropertyName(MapString("title".to_string()))) {
        Some(BaseValue::StringValue(value)) => value.0.clone(),
        other => panic!("expected a string title, got {other:?}"),
    }
}

async fn persist(backend: &MockConductorConfig, request: HolonWriteRequest) -> StoredHolonNode {
    backend.conductor.call(&backend.cell.zome(ZOME), "holon_storage_persist", request).await
}

/// Publishes a lineage root and returns it.
async fn publish_root(backend: &MockConductorConfig, title: &str) -> StoredHolonNode {
    persist(backend, HolonWriteRequest::PublishRoot { holon_node: node(title) }).await
}

/// Publishes a version against the given predecessors and returns it.
async fn publish_version(
    backend: &MockConductorConfig,
    title: &str,
    predecessor_ids: Vec<LocalId>,
) -> StoredHolonNode {
    persist(backend, HolonWriteRequest::PublishVersion { holon_node: node(title), predecessor_ids })
        .await
}

async fn get_holon(backend: &MockConductorConfig, id: &LocalId) -> Option<StoredHolonNode> {
    backend.conductor.call(&backend.cell.zome(ZOME), "holon_storage_get", id.clone()).await
}

async fn get_holons(
    backend: &MockConductorConfig,
    ids: Vec<LocalId>,
) -> Vec<Option<StoredHolonNode>> {
    backend.conductor.call(&backend.cell.zome(ZOME), "holon_storage_get_many", ids).await
}

/// A well-formed action hash that was never persisted.
///
/// Built through `ActionHash` rather than from hand-written bytes so the multihash prefix is
/// correct by construction: a wrong prefix would fail hash conversion and test the wrong thing.
fn unpersisted_id() -> LocalId {
    LocalId(ActionHash::from_raw_36(vec![0x5a; 36]).get_raw_39().to_vec())
}

// ---------------------------------------------------------------------------
// Publish: action selection and record-derived metadata
// ---------------------------------------------------------------------------

/// `PublishRoot` must author a `Create`, which reads back as a lineage root: it is its own
/// version and carries no lineage pointer.
#[tokio::test(flavor = "multi_thread")]
async fn publish_root_returns_create_metadata_and_round_trips_through_get_holon() {
    let backend = setup_test_conductor().await;

    let root = publish_root(&backend, "original").await;

    assert!(
        root.version_metadata.is_lineage_root(),
        "a PublishRoot write must be recorded as a Create, so it has no lineage pointer"
    );
    assert_eq!(root.version_metadata.lineage_id, None);
    assert_eq!(
        root.version_metadata.lineage_root(),
        LineageId(root.version_metadata.version_id.clone()),
        "a root is its own lineage"
    );

    let fetched = get_holon(&backend, &root.version_metadata.version_id)
        .await
        .expect("a just-published root must be readable by its own id");

    assert_eq!(fetched, root, "the exact read must reproduce what the write reported");
    assert_eq!(title_of(&fetched), "original");
}

/// `PublishVersion` must author an `Update` rooted at the lineage `Create`. Reading it back
/// proves the lineage pointer came from the record: nothing wrote it into the entry.
#[tokio::test(flavor = "multi_thread")]
async fn publish_version_is_rooted_at_the_create_and_round_trips() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "original").await;
    let root_id = root.version_metadata.version_id.clone();

    let version = publish_version(&backend, "revised", vec![root_id.clone()]).await;

    assert_ne!(
        version.version_metadata.version_id, root_id,
        "a new version must have its own distinct identity"
    );
    assert_eq!(
        version.version_metadata.lineage_id,
        Some(LineageId(root_id.clone())),
        "the version must record the lineage it descends from"
    );

    let fetched = get_holon(&backend, &version.version_metadata.version_id)
        .await
        .expect("a just-published version must be readable by its own id");
    assert_eq!(fetched, version);
    assert_eq!(title_of(&fetched), "revised");

    // The root is untouched by the update: an exact read is version-addressed, not head-addressed.
    let fetched_root =
        get_holon(&backend, &root_id).await.expect("the lineage root must remain readable");
    assert_eq!(title_of(&fetched_root), "original");
    assert!(fetched_root.version_metadata.is_lineage_root());
}

/// A version of a version must stay rooted at the original `Create`, never at its immediate
/// predecessor. This is the rule that keeps a lineage one hop deep in the substrate, and the
/// reason Integrity's update-target check can insist on a `Create`.
#[tokio::test(flavor = "multi_thread")]
async fn a_version_of_a_version_is_still_rooted_at_the_original_create() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "v1").await;
    let root_id = root.version_metadata.version_id.clone();

    let second = publish_version(&backend, "v2", vec![root_id.clone()]).await;
    let third =
        publish_version(&backend, "v3", vec![second.version_metadata.version_id.clone()]).await;

    assert_eq!(
        third.version_metadata.lineage_id,
        Some(LineageId(root_id.clone())),
        "the third generation must root at the original Create, not at v2"
    );
    assert_ne!(
        third.version_metadata.lineage_id,
        Some(LineageId(second.version_metadata.version_id.clone())),
        "rooting at the immediate predecessor would build an Update->Update chain"
    );

    // All three generations agree on one lineage.
    for version in [&root, &second, &third] {
        assert_eq!(
            version.version_metadata.lineage_root(),
            LineageId(root_id.clone()),
            "every generation must resolve to the same lineage root"
        );
    }
}

/// Storage permits branching: two versions may descend from the same root. Choosing between
/// branches is a higher-layer concern, so storage must not silently prevent them.
#[tokio::test(flavor = "multi_thread")]
async fn sibling_versions_of_one_root_share_a_lineage_id() {
    let backend = setup_test_conductor().await;
    let root_id = publish_root(&backend, "shared").await.version_metadata.version_id;

    let left = publish_version(&backend, "left", vec![root_id.clone()]).await;
    let right = publish_version(&backend, "right", vec![root_id.clone()]).await;

    assert_ne!(
        left.version_metadata.version_id, right.version_metadata.version_id,
        "siblings must be distinct versions"
    );
    assert_eq!(left.version_metadata.lineage_id, Some(LineageId(root_id.clone())));
    assert_eq!(right.version_metadata.lineage_id, Some(LineageId(root_id)));

    assert_eq!(title_of(&left), "left");
    assert_eq!(title_of(&right), "right");
}

/// A predecessor may itself be a derived version, and a root and its own version resolve to the
/// same lineage — so naming both is consistent, not a conflict.
#[tokio::test(flavor = "multi_thread")]
async fn multiple_predecessors_of_one_lineage_are_accepted() {
    let backend = setup_test_conductor().await;
    let root_id = publish_root(&backend, "v1").await.version_metadata.version_id;
    let second = publish_version(&backend, "v2", vec![root_id.clone()]).await;

    let merged = publish_version(
        &backend,
        "merged",
        vec![root_id.clone(), second.version_metadata.version_id.clone(), root_id.clone()],
    )
    .await;

    assert_eq!(merged.version_metadata.lineage_id, Some(LineageId(root_id)));
}

// ---------------------------------------------------------------------------
// Read: exact-version and positional batch semantics
// ---------------------------------------------------------------------------

/// A missing id is an absence, not an error: `None` rather than a failed call.
#[tokio::test(flavor = "multi_thread")]
async fn get_holon_returns_none_for_an_unpersisted_id() {
    let backend = setup_test_conductor().await;

    assert_eq!(get_holon(&backend, &unpersisted_id()).await, None);
}

/// The batch read owes one slot per requested id, in order, including repeats and gaps. Anything
/// that compacted the result would silently misalign a caller's parallel data.
#[tokio::test(flavor = "multi_thread")]
async fn get_holons_preserves_order_duplicates_and_missing_slots() {
    let backend = setup_test_conductor().await;
    let root = publish_root(&backend, "original").await;
    let root_id = root.version_metadata.version_id.clone();
    let version = publish_version(&backend, "revised", vec![root_id.clone()]).await;
    let version_id = version.version_metadata.version_id.clone();
    let missing = unpersisted_id();

    let requested =
        vec![root_id.clone(), missing.clone(), version_id.clone(), root_id.clone(), missing];
    let slots = get_holons(&backend, requested.clone()).await;

    assert_eq!(slots.len(), requested.len(), "one slot is owed per requested id");
    assert_eq!(slots[0].as_ref().map(title_of), Some("original".to_string()));
    assert!(slots[1].is_none(), "a gap must stay in position rather than compacting the result");
    assert_eq!(slots[2].as_ref().map(title_of), Some("revised".to_string()));
    assert_eq!(slots[3], slots[0], "a duplicated id must yield a duplicated slot");
    assert!(slots[4].is_none());

    // Each slot carries its own record-derived metadata, not the batch's or its neighbour's.
    assert!(slots[0].as_ref().unwrap().version_metadata.is_lineage_root());
    assert_eq!(slots[2].as_ref().unwrap().version_metadata.lineage_id, Some(LineageId(root_id)));
}

/// An empty request is answered, not refused.
#[tokio::test(flavor = "multi_thread")]
async fn get_holons_accepts_an_empty_request() {
    let backend = setup_test_conductor().await;

    assert_eq!(get_holons(&backend, Vec::new()).await, Vec::new());
}

/// An id that is not a well-formed action hash is a defect, not an absence.
///
/// Reporting `None` here would let a malformed id masquerade as a missing holon, which is exactly
/// the confusion the read contract is meant to prevent.
#[tokio::test(flavor = "multi_thread")]
async fn get_holon_fails_rather_than_reporting_absence_for_a_malformed_id() {
    let backend = setup_test_conductor().await;

    let result = backend
        .conductor
        .call_fallible::<_, Option<StoredHolonNode>>(
            &backend.cell.zome(ZOME),
            "holon_storage_get",
            LocalId(vec![0x00; 39]),
        )
        .await;

    let error = format!("{:?}", result.expect_err("a malformed id must not read as absent"));
    assert!(
        error.contains("Invalid ActionHash"),
        "expected a hash-conversion failure, got {error}"
    );
}

// ---------------------------------------------------------------------------
// Rejections
// ---------------------------------------------------------------------------

/// Grafting one lineage onto another is refused, and refused *before* anything is written.
#[tokio::test(flavor = "multi_thread")]
async fn publish_version_rejects_predecessors_from_different_lineages() {
    let backend = setup_test_conductor().await;
    let first_root = publish_root(&backend, "first").await.version_metadata.version_id;
    let second_root = publish_root(&backend, "second").await.version_metadata.version_id;

    let result = backend
        .conductor
        .call_fallible::<_, StoredHolonNode>(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishVersion {
                holon_node: node("grafted"),
                predecessor_ids: vec![first_root.clone(), second_root.clone()],
            },
        )
        .await;

    let error = format!("{:?}", result.expect_err("mismatched lineage roots must be rejected"));
    assert!(
        error.contains("different lineage roots"),
        "expected a lineage-mismatch rejection, got {error}"
    );

    // Neither lineage gained a version: the write was refused, not partially applied.
    for root_id in [first_root, second_root] {
        let root = get_holon(&backend, &root_id).await.expect("each root must remain readable");
        assert!(root.version_metadata.is_lineage_root());
    }
}

/// A version needs a predecessor to inherit a lineage from; an empty set has no answer.
#[tokio::test(flavor = "multi_thread")]
async fn publish_version_rejects_an_empty_predecessor_set() {
    let backend = setup_test_conductor().await;

    let result = backend
        .conductor
        .call_fallible::<_, StoredHolonNode>(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishVersion {
                holon_node: node("orphan"),
                predecessor_ids: Vec::new(),
            },
        )
        .await;

    let error = format!("{:?}", result.expect_err("a predecessorless version must be rejected"));
    assert!(
        error.contains("at least one predecessor"),
        "expected a missing-predecessor rejection, got {error}"
    );
}

/// A lineage cannot be resolved from an id that names nothing.
#[tokio::test(flavor = "multi_thread")]
async fn publish_version_rejects_an_unpersisted_predecessor() {
    let backend = setup_test_conductor().await;

    let result = backend
        .conductor
        .call_fallible::<_, StoredHolonNode>(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishVersion {
                holon_node: node("dangling"),
                predecessor_ids: vec![unpersisted_id()],
            },
        )
        .await;

    let error = format!("{:?}", result.expect_err("an unpersisted predecessor must be rejected"));
    assert!(error.contains("is not persisted"), "expected a not-persisted rejection, got {error}");
}

/// Integrity must reject an update aimed at another update rather than at a lineage root.
///
/// `persist_holon` cannot construct this topology — it always addresses the resolved root — so
/// this drives the test-only probe extern to author it deliberately. The rejection is the
/// assertion: it is what makes the one-hop lineage invariant enforced rather than merely
/// intended.
#[tokio::test(flavor = "multi_thread")]
async fn integrity_rejects_an_update_targeting_an_update() {
    let backend = setup_test_conductor().await;
    let root_id = publish_root(&backend, "v1").await.version_metadata.version_id;
    let second = publish_version(&backend, "v2", vec![root_id]).await;

    let result = backend
        .conductor
        .call_fallible::<_, LocalId>(
            &backend.cell.zome(ZOME),
            "holon_storage_author_update_for_test",
            (second.version_metadata.version_id.clone(), node("v3")),
        )
        .await;

    assert_commit_rejected_with_pvl(result, EXPECTED_UPDATE_TARGET_REJECTION);
}

/// An update aimed at a lineage root is the topology `persist_holon` produces, so the same probe
/// must be accepted — proving the rejection above is about topology, not about the probe itself.
#[tokio::test(flavor = "multi_thread")]
async fn integrity_accepts_an_update_targeting_a_lineage_root() {
    let backend = setup_test_conductor().await;
    let root_id = publish_root(&backend, "v1").await.version_metadata.version_id;

    let authored: LocalId = backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "holon_storage_author_update_for_test",
            (root_id.clone(), node("v2")),
        )
        .await;

    let stored =
        get_holon(&backend, &authored).await.expect("the authored update must be readable");
    assert_eq!(stored.version_metadata.lineage_id, Some(LineageId(root_id)));
}
