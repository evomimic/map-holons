//! Live-conductor sweettests for the SmartLink storage API (Issue #594).
//!
//! These bypass the dance DSL: they spin up a conductor with `setup_test_conductor()`
//! and call the `smartlink_*` zome externs directly, asserting the real DHT outcomes
//! (`Inserted` / `AlreadyPresent` / `Conflict` / `Deleted` / `AlreadyAbsent`, plus the
//! `StartsWith("")` guard) that the pure comparator unit tests cannot exercise.
//!
//! Endpoints are real committed holon nodes published through canonical `holon_storage_persist`,
//! because SmartLink integrity validation requires 39-byte action-hash source/target ids.
//!
//! `occurrence_persistence_semantics` covers Storage SL3: occurrence ids as part of
//! insertion identity. Storage stays descriptor-unaware there — it never inspects
//! `AllowsDuplicates`, assigns an occurrence, or pairs inverse occurrences.

use base_types::{BaseValue, MapString};
use core_types::{
    CanonicalKey, CanonicalKeyPrefix, DeleteSmartLinkOutcome, HolonId, HolonWriteRequest, KeyMatch,
    OccurrenceId, PreparedSmartLink, PropertyName, PutSmartLinkOutcome, SmartLink, SmartLinkId,
    StoredHolonNode, TargetPropertyCacheCandidate,
};
use holons_test::setup_test_conductor;
use integrity_core_types::{HolonNodeModel, LocalId, PropertyMap, RelationshipName};
use std::collections::{BTreeMap, HashSet};

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

fn occ(seed: u8) -> OccurrenceId {
    OccurrenceId([seed; 16])
}

fn prepared_with_occurrence(
    source: &LocalId,
    target: &LocalId,
    relationship: &str,
    k: &str,
    occurrence: OccurrenceId,
) -> PreparedSmartLink {
    let mut p = prepared(source, target, relationship, k);
    p.occurrence_id = Some(occurrence);
    p
}

/// Commits an empty holon node and returns its physical id (a valid 39-byte action hash).
async fn new_endpoint(backend: &holons_test::MockConductorConfig) -> LocalId {
    let stored: StoredHolonNode = backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "holon_storage_persist",
            HolonWriteRequest::PublishRoot { holon_node: HolonNodeModel::new(PropertyMap::new()) },
        )
        .await;
    stored.version_id().clone()
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

fn expect_inserted(outcome: PutSmartLinkOutcome, phase: &str) -> SmartLinkId {
    match outcome {
        PutSmartLinkOutcome::Inserted(id) => id,
        other => panic!("{phase}: expected Inserted, got {other:?}"),
    }
}

/// Storage SL3: `occurrence_id` is part of SmartLink insertion identity.
///
/// Runs every occurrence scenario against one conductor, in four phases. Storage stays
/// descriptor-unaware throughout: nothing here resolves `AllowsDuplicates`, generates an
/// occurrence, or pairs occurrences across directions — the test only supplies occurrences
/// and asserts what storage does with them.
#[tokio::test(flavor = "multi_thread")]
async fn occurrence_persistence_semantics() {
    let backend = setup_test_conductor().await;
    let zome = backend.cell.zome(ZOME);
    let source = new_endpoint(&backend).await;
    let target = new_endpoint(&backend).await;

    // -----------------------------------------------------------------------
    // Phase 1: occurrence is part of identity, and round-trips through the DHT.
    // -----------------------------------------------------------------------

    // A set-style link (no occurrence).
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(&zome, "smartlink_put", prepared(&source, &target, "Likes", "a"))
        .await;
    let id_none = expect_inserted(out, "phase 1");

    // Same source/target/relationship/key, but occurrence-bearing => a different
    // identity, so it inserts rather than conflicting with the keyless-occurrence link.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "a", occ(1)),
        )
        .await;
    let id_1 = expect_inserted(out, "phase 1: None and Some(id) are different identities");
    assert_ne!(id_1, id_none);

    // A second, distinct occurrence: two otherwise-identical links coexist.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "a", occ(2)),
        )
        .await;
    let id_2 = expect_inserted(out, "phase 1: distinct occurrences both insert");
    assert_ne!(id_2, id_none);
    assert_ne!(id_2, id_1);

    // Retrying the same occurrence unchanged is idempotent and reports the original id.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "a", occ(1)),
        )
        .await;
    assert_eq!(
        out,
        PutSmartLinkOutcome::AlreadyPresent(id_1.clone()),
        "phase 1: replaying an occurrence must return AlreadyPresent with the original id"
    );

    // All three are live, and each carries back exactly the occurrence it was written with.
    let all: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    let observed: HashSet<(SmartLinkId, Option<OccurrenceId>)> =
        all.iter().map(|l| (l.smartlink_id.clone(), l.occurrence_id)).collect();
    let expected: HashSet<(SmartLinkId, Option<OccurrenceId>)> = HashSet::from([
        (id_none.clone(), None),
        (id_1.clone(), Some(occ(1))),
        (id_2.clone(), Some(occ(2))),
    ]);
    assert_eq!(observed, expected, "phase 1: occurrences must round-trip verbatim");

    // Occurrence bytes sit after the NUL-delimited key region in Tag v1, so they are not
    // prefix-queryable: a key query returns every occurrence sharing that key, and callers
    // that want one occurrence must filter in-process on `SmartLink::occurrence_id`.
    let by_key: Vec<SmartLink> = backend
        .conductor
        .call(
            &zome,
            "smartlink_expand_by_key",
            (source.clone(), rel("Likes"), KeyMatch::Exact(key("a"))),
        )
        .await;
    assert_eq!(by_key.len(), 3, "phase 1: key queries do not discriminate by occurrence");

    // -----------------------------------------------------------------------
    // Phase 2: within one occurrence, key/prop divergence still conflicts.
    // -----------------------------------------------------------------------

    // Same occurrence, different canonical key => Conflict against the original link.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "b", occ(1)),
        )
        .await;
    assert_eq!(
        out,
        PutSmartLinkOutcome::Conflict(id_1.clone()),
        "phase 2: same occurrence with a different key must conflict"
    );

    // Same occurrence, different authoritative relationship props => Conflict.
    let mut diff_props = prepared_with_occurrence(&source, &target, "Likes", "a", occ(1));
    diff_props.relationship_property_values = string_props("weight", "heavy");
    let out: PutSmartLinkOutcome = backend.conductor.call(&zome, "smartlink_put", diff_props).await;
    assert_eq!(
        out,
        PutSmartLinkOutcome::Conflict(id_1.clone()),
        "phase 2: same occurrence with different rel props must conflict"
    );

    // The load-bearing contrast: a *differing* occurrence is never a conflict, even when
    // the canonical key differs — it is simply another identity.
    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "b", occ(3)),
        )
        .await;
    let id_3 = expect_inserted(out, "phase 2: a differing occurrence must insert, not conflict");

    // Neither conflicting put wrote a link, and id_1 kept its original key.
    let all: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    assert_eq!(all.len(), 4, "phase 2: conflicting puts must not insert");
    let live_1 = all
        .iter()
        .find(|l| l.smartlink_id == id_1)
        .expect("phase 2: the conflicted-against link must still be live");
    assert_eq!(live_1.canonical_key, key("a"), "phase 2: a conflict must not rewrite the key");

    // -----------------------------------------------------------------------
    // Phase 3: declared and inverse directions may share one occurrence.
    //
    // Storage proves only the *shape* here. These two PreparedSmartLinks are paired by
    // hand: assigning a shared occurrence to a declared/inverse pair is coordinator work,
    // and storage neither performs nor validates that pairing.
    // -----------------------------------------------------------------------

    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&source, &target, "Likes", "a", occ(9)),
        )
        .await;
    let fwd_9 = expect_inserted(out, "phase 3: forward direction");

    let out: PutSmartLinkOutcome = backend
        .conductor
        .call(
            &zome,
            "smartlink_put",
            prepared_with_occurrence(&target, &source, "LikedBy", "a", occ(9)),
        )
        .await;
    let inv_9 = expect_inserted(out, "phase 3: inverse direction");

    // One shared OccurrenceId, two distinct physical links.
    assert_ne!(fwd_9, inv_9, "phase 3: each direction gets its own SmartLinkId");

    let inverse: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", target.clone()).await;
    assert_eq!(inverse.len(), 1, "phase 3: the inverse link is anchored to the target");
    assert_eq!(inverse[0].smartlink_id, inv_9);
    assert_eq!(inverse[0].occurrence_id, Some(occ(9)));
    assert_eq!(inverse[0].relationship_name, rel("LikedBy"));

    // -----------------------------------------------------------------------
    // Phase 4: deletion is keyed by SmartLinkId, never by OccurrenceId.
    // -----------------------------------------------------------------------

    let out: DeleteSmartLinkOutcome =
        backend.conductor.call(&zome, "smartlink_delete", id_1.clone()).await;
    assert_eq!(out, DeleteSmartLinkOutcome::Deleted, "phase 4");

    let out: DeleteSmartLinkOutcome =
        backend.conductor.call(&zome, "smartlink_delete", id_1.clone()).await;
    assert_eq!(out, DeleteSmartLinkOutcome::AlreadyAbsent, "phase 4: delete stays idempotent");

    // Deleting one occurrence leaves every sibling occurrence untouched.
    let remaining: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", source.clone()).await;
    let survivors: HashSet<SmartLinkId> =
        remaining.iter().map(|l| l.smartlink_id.clone()).collect();
    assert_eq!(
        survivors,
        HashSet::from([id_none, id_2, id_3, fwd_9]),
        "phase 4: deleting one occurrence must not delete its siblings"
    );

    // The inverse link is a separate physical link, so deleting a forward occurrence
    // never reaches it — even though both were written with occ(9).
    let inverse: Vec<SmartLink> =
        backend.conductor.call(&zome, "smartlink_expand_all", target.clone()).await;
    assert_eq!(inverse.len(), 1, "phase 4: the inverse direction is unaffected");
    assert_eq!(inverse[0].smartlink_id, inv_9);
    assert_eq!(inverse[0].occurrence_id, Some(occ(9)));
}
