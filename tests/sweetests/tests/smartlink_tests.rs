//! Live-conductor sweettests for the SmartLink storage API (Issue #594).
//!
//! These bypass the dance DSL: they spin up a conductor with `setup_test_conductor()`
//! and call the `smartlink_*` zome externs directly, asserting the real DHT outcomes
//! (`Inserted` / `AlreadyPresent` / `Conflict` / `Deleted` / `AlreadyAbsent`, plus the
//! `StartsWith("")` guard) that the pure comparator unit tests cannot exercise.
//!
//! Endpoints are real committed holon nodes (created via the `create_holon_node` extern),
//! because SmartLink integrity validation requires 39-byte action-hash source/target ids.

use base_types::{BaseValue, MapString};
use core_types::{
    CanonicalKey, CanonicalKeyPrefix, DeleteSmartLinkOutcome, HolonId, KeyMatch, PreparedSmartLink,
    PropertyName, PutSmartLinkOutcome, SmartLink, TargetPropertyCacheCandidate,
};
use holochain::prelude::Record;
use holons_test::setup_test_conductor;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};
use serde::Serialize;
use std::collections::BTreeMap;

/// Mirrors `holons_guest_integrity::HolonNode` for the `create_holon_node` extern input,
/// so this test needs no dependency on the (hdi-based) integrity crate.
#[derive(Serialize, Debug)]
struct HolonNodeInput {
    original_id: Option<LocalId>,
    property_map: PropertyMap,
}

const ZOME: &str = "holons";

fn rel(name: &str) -> RelationshipName {
    RelationshipName(MapString(name.to_string()))
}

fn key(value: &str) -> CanonicalKey {
    CanonicalKey::new(value).unwrap()
}

fn string_props(name: &str, value: &str) -> PropertyMap {
    BTreeMap::from([(
        PropertyName(MapString(name.to_string())),
        BaseValue::StringValue(MapString(value.to_string())),
    )])
}

fn prepared(source: &LocalId, target: &LocalId, relationship: &str, k: &str) -> PreparedSmartLink {
    PreparedSmartLink {
        source_id: source.clone(),
        target_id: HolonId::Local(target.clone()),
        relationship_name: rel(relationship),
        canonical_key: key(k),
        occurrence_id: None,
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    }
}

/// Commits an empty holon node and returns its physical id (a valid 39-byte action hash).
async fn new_endpoint(backend: &holons_test::MockConductorConfig) -> LocalId {
    let record: Record = backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "create_holon_node",
            HolonNodeInput { original_id: None, property_map: PropertyMap::new() },
        )
        .await;
    LocalId(record.action_address().get_raw_39().to_vec())
}

#[tokio::test(flavor = "multi_thread")]
async fn put_insert_present_conflict() {
    let backend = setup_test_conductor().await;
    let zome = backend.cell.zome(ZOME);
    let source = new_endpoint(&backend).await;
    let target = new_endpoint(&backend).await;

    // First put => Inserted.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target, "Likes", "target-key"))
        .await;
    let id = match out {
        PutSmartLinkOutcome::Inserted(id) => id,
        other => panic!("expected Inserted, got {other:?}"),
    };

    // Identical put => AlreadyPresent with the same physical id.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target, "Likes", "target-key"))
        .await;
    assert_eq!(out, PutSmartLinkOutcome::AlreadyPresent(id.clone()));

    // Same identity + key + rel props, but a different target-property cache => still AlreadyPresent.
    let mut cached = prepared(&source, &target, "Likes", "target-key");
    cached.target_property_cache_candidates = vec![TargetPropertyCacheCandidate {
        property_name: PropertyName(MapString("title".to_string())),
        value: BaseValue::StringValue(MapString("cached".to_string())),
    }];
    let out: PutSmartLinkOutcome = backend.conductor.call(&zome, "smartlink_put", cached).await;
    assert_eq!(out, PutSmartLinkOutcome::AlreadyPresent(id.clone()));

    // Same identity, different canonical key => Conflict, and no second link is written.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target, "Likes", "different-key"))
        .await;
    assert_eq!(out, PutSmartLinkOutcome::Conflict(id.clone()));

    // Same identity, different authoritative relationship props => Conflict.
    let mut diff_props = prepared(&source, &target, "Likes", "target-key");
    diff_props.relationship_property_values = string_props("weight", "heavy");
    let out: PutSmartLinkOutcome = backend.conductor.call(&zome, "smartlink_put", diff_props).await;
    assert_eq!(out, PutSmartLinkOutcome::Conflict(id.clone()));

    // Exactly one live link exists despite the conflicting puts.
    let links: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].smartlink_id, id);
    assert_eq!(links[0].canonical_key, key("target-key"));
}

#[tokio::test(flavor = "multi_thread")]
async fn expand_modes() {
    let backend = setup_test_conductor().await;
    let zome = backend.cell.zome(ZOME);
    let source = new_endpoint(&backend).await;
    let target_a = new_endpoint(&backend).await;
    let target_b = new_endpoint(&backend).await;

    // Two "Likes" links with different keys, one "Owns" link.
    for (target, k) in [(&target_a, "apple"), (&target_b, "apricot")] {
        let _: PutSmartLinkOutcome = backend
            .conductor
            .call(&zome, "smartlink_put", prepared(&source, target, "Likes", k))
            .await;
    }
    let _: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target_a, "Owns", "apple"))
        .await;

    // expand_all => all three.
    let all: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    assert_eq!(all.len(), 3);

    // expand by relationship => only the two "Likes".
    let likes: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand", (source.clone(), rel("Likes"))).await;
    assert_eq!(likes.len(), 2);
    assert!(likes.iter().all(|l| l.relationship_name == rel("Likes")));

    // expand by exact key => only the "apple" Likes link.
    let exact: Vec<SmartLink> = backend
        .conductor
        .call(
            &zome,
            "smartlink_expand_by_key",
            (source.clone(), rel("Likes"), KeyMatch::Exact(key("apple"))),
        )
        .await;
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].canonical_key, key("apple"));

    // expand by "ap" prefix => both apple + apricot.
    let prefix: Vec<SmartLink> = backend
        .conductor
        .call(
            &zome,
            "smartlink_expand_by_key",
            (
                source.clone(),
                rel("Likes"),
                KeyMatch::StartsWith(CanonicalKeyPrefix::new("ap").unwrap()),
            ),
        )
        .await;
    assert_eq!(prefix.len(), 2);

    // Empty starts-with prefix is rejected.
    let empty = backend
        .conductor
        .call_fallible::<(LocalId, RelationshipName, KeyMatch), Vec<SmartLink>>(
            &zome,
            "smartlink_expand_by_key",
            (
                source.clone(),
                rel("Likes"),
                KeyMatch::StartsWith(CanonicalKeyPrefix::new("").unwrap()),
            ),
        )
        .await;
    assert!(empty.is_err(), "StartsWith(\"\") must be rejected");
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_idempotent_and_leaves_siblings() {
    let backend = setup_test_conductor().await;
    let zome = backend.cell.zome(ZOME);
    let source = new_endpoint(&backend).await;
    let target_a = new_endpoint(&backend).await;
    let target_b = new_endpoint(&backend).await;

    let a: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target_a, "Likes", "a"))
        .await;
    let id_a = match a {
        PutSmartLinkOutcome::Inserted(id) => id,
        other => panic!("expected Inserted, got {other:?}"),
    };
    let _: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target_b, "Likes", "b"))
        .await;

    // First delete of id_a => Deleted.
    let out: DeleteSmartLinkOutcome =
        backend.conductor.call(&zome, "smartlink_delete", id_a.clone()).await;
    assert_eq!(out, DeleteSmartLinkOutcome::Deleted);

    // Repeat delete => AlreadyAbsent (idempotent).
    let out: DeleteSmartLinkOutcome =
        backend.conductor.call(&zome, "smartlink_delete", id_a.clone()).await;
    assert_eq!(out, DeleteSmartLinkOutcome::AlreadyAbsent);

    // The sibling link survived.
    let remaining: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    assert_eq!(remaining.len(), 1);
    assert_ne!(remaining[0].smartlink_id, id_a);
    assert_eq!(remaining[0].canonical_key, key("b"));
}

#[tokio::test(flavor = "multi_thread")]
async fn expand_by_key_keyless_exact() {
    let backend = setup_test_conductor().await;
    let zome = backend.cell.zome(ZOME);
    let source = new_endpoint(&backend).await;
    let target = new_endpoint(&backend).await;

    // A keyless SmartLink: empty canonical key is valid.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target, "Likes", ""))
        .await;
    let id = match out {
        PutSmartLinkOutcome::Inserted(id) => id,
        other => panic!("expected Inserted, got {other:?}"),
    };

    // Exact match on the empty (keyless) key returns exactly that link.
    let exact: Vec<SmartLink> = backend
        .conductor
        .call(
            &zome,
            "smartlink_expand_by_key",
            (source.clone(), rel("Likes"), KeyMatch::Exact(key(""))),
        )
        .await;
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].smartlink_id, id);
    assert_eq!(exact[0].canonical_key, key(""));
}
