//! Live-conductor coverage for commit's SmartLink conflict policy (Storage SL4, #630).
//!
//! `persist_smartlink` in `commit_functions.rs` converts a `PutSmartLinkOutcome::Conflict` into a
//! `HolonError::CommitFailure`. Pass 2 records that error on the staged holon and downgrades the
//! commit response to `Incomplete`; the commit call itself still returns `Ok`, because Pass 1 has
//! already persisted the holon.
//!
//! **What actually needs pinning is the status downgrade, not the absence of a write.** A conflict
//! never writes either way — `put_smartlink` declines before authoring — so relaxing the `Conflict`
//! arm to `Ok(())` would leave the DHT identical and only change the reported outcome from
//! `Incomplete` to `Complete`. That silent "we committed your relationship" is the actual defect the
//! policy prevents, so it is what this test asserts.
//!
//! Reaching the conflict requires a commit whose SmartLink source is an *already-persisted* id.
//! Creates and version-producing updates both mint a new node id, so their links can never collide
//! with an existing one. Graph-only updates are the exception: `StagedState::ForUpdateGraphOnly`
//! commits to the existing source anchor, so `(source, target, relationship)` can genuinely
//! pre-exist when Pass 2 runs. A non-definitional relationship mutation on a staged-for-update
//! holon is what selects that state.
//!
//! The mutation used here is `Book --ReferencesProperty--> Title.PropertyType`, matching the graph-only
//! phase of `stage_new_version_fixture`. `ReferencesProperty` is declared for Book instances through
//! `Book.HolonType`'s `InstanceProperties` and is non-definitional. The Book instance is described
//! explicitly rather than loaded, because a descriptor is what makes the relationship declaration
//! resolvable at all.
//!
//! # Why the colliding link is planted *after* staging
//!
//! Ordering is the whole trick, and getting it wrong makes the test silently vacuous.
//!
//! Staging a holon for update hydrates its persisted relationships, and `add_related_holons`
//! drops an entry whose *reference identity* already exists as an idempotent no-op — canonical key
//! is not consulted in that comparison. So planting the stale-keyed link *before* staging makes the
//! subsequent add a no-op: the collection is never touched, Pass 2 writes nothing, and the commit
//! reports `Complete` having tested nothing. (That is not hypothetical; it is what this test did
//! before the ordering was fixed.)
//!
//! Planting between staging and commit is therefore not a contrivance — it is the only shape the
//! conflict has. It models the real case the policy defends against: a link authored by someone
//! else, or by an older writer with different key semantics, appearing under an identity this
//! commit is about to write.

use core_types::{
    encode_smartlink_tag, CanonicalKey, ContentSet, HolonId, SmartLink, SmartLinkTagInput,
};
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::{HolonServiceApi, ServiceRoutingPolicy};
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    build_book_person_inverse_content_set, build_core_schema_content_set,
    setup_probe_enabled_conductor, BOOK_DESCRIPTOR_KEY,
};
use holons_test::MockConductorConfig;
use holons_trust_channel::TrustChannel;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};
use map_commands_contract::{
    HolonAction, HolonCommand, MapCommand, MapResult, SpaceCommand, TransactionAction,
    TransactionCommand, WritableHolonAction,
};
use map_commands_runtime::{ExecutionPolicy, Runtime, RuntimeSession};
use std::sync::Arc;

const ZOME: &str = "holons";
const PROBE_ZOME: &str = "holons_test_probes";

const BOOK_KEY: &str = "Book.CommitConflict.1";
const TITLE_PROPERTY_KEY: &str = "Title.PropertyType";
const REFERENCES_PROPERTY: &str = "ReferencesProperty";
const BOOK_CACHE_1_KEY: &str = "Book.SmartLinkCache.1";
const BOOK_CACHE_2_KEY: &str = "Book.SmartLinkCache.2";
const PERSON_CACHE_1_KEY: &str = "Person.SmartLinkCache.1";
const PERSON_CACHE_2_KEY: &str = "Person.SmartLinkCache.2";
const PERSON_DESCRIPTOR_KEY: &str = "Person.HolonType";
const AUTHORED_BY: &str = "AuthoredBy";
const AUTHOR_OF: &str = "AuthorOf";

/// Canonical key on the planted link. Differs from the book's real key, which is what makes the
/// commit-time write a `Conflict` rather than an idempotent `AlreadyPresent`.
const STALE_KEY: &str = "stale-key-from-an-older-writer";

fn rel(name: &str) -> RelationshipName {
    RelationshipName(MapString(name.to_string()))
}

/// Creates one Book instance, described by `Book.HolonType`, and commits it.
///
/// Returns the saved instance's id. The `DescribedBy` edge is what makes `ReferencesProperty` resolvable on
/// this holon later; without it the graph-only add fails with `DescriptorDeclarationNotFound`
/// before reaching any SmartLink write.
///
/// The descriptor is resolved inside this function's own transaction: `HolonReference`s are
/// transaction-bound, so only ids may cross a transaction boundary.
async fn create_described_book(runtime: &Runtime) -> LocalId {
    create_described_holon(runtime, BOOK_KEY, BOOK_DESCRIPTOR_KEY).await
}

/// Creates and commits one instance with an explicit `DescribedBy` relationship.
///
/// The descriptor reference is resolved in the creation transaction because runtime references
/// are transaction-bound; the returned local id can safely cross into later transactions.
async fn create_described_holon(runtime: &Runtime, key: &str, descriptor_key: &str) -> LocalId {
    let context = begin_transaction(runtime).await;
    let descriptor = saved_reference_by_key(runtime, &context, descriptor_key).await;

    let transient = match runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::NewHolon { key: Some(MapString(key.to_string())) },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("new_holon failed")
    {
        MapResult::Reference(HolonReference::Transient(transient)) => transient,
        other => panic!("expected a transient Reference, got {other:?}"),
    };

    let staged = match runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::StageNewHolon { source: transient },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("stage_new_holon failed")
    {
        MapResult::Reference(reference) => reference,
        other => panic!("expected a staged Reference, got {other:?}"),
    };

    runtime
        .execute_command(
            MapCommand::Holon(HolonCommand {
                context: Arc::clone(&context),
                target: staged,
                action: HolonAction::Write(WritableHolonAction::AddRelatedHolons {
                    name: CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                    holons: vec![descriptor],
                }),
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("describing '{key}' failed: {error:?}"));

    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("committing described '{key}' failed: {error:?}"));

    let context = begin_transaction(runtime).await;
    local_id_of(&saved_reference_by_key(runtime, &context, key).await)
}

/// Builds a runtime over `backend`, keeping the same conductor handle the test uses for raw probe
/// and storage extern calls. Mirrors `init_test_runtime`, minus the fixture-transient import.
async fn runtime_over(backend: Arc<MockConductorConfig>) -> Runtime {
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(ClientHolonService);
    let dance_initiator = Arc::new(TrustChannel::new(backend));

    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        Some(dance_initiator),
        holon_service,
        None,
        ServiceRoutingPolicy::Combined,
    ));

    let session = Arc::new(RuntimeSession::new(Arc::clone(&space_manager), None));

    Runtime::new(session)
}

async fn begin_transaction(runtime: &Runtime) -> Arc<TransactionContext> {
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("failed to begin transaction");

    let tx_id = match result {
        MapResult::TransactionCreated { tx_id } => tx_id,
        other => panic!("expected TransactionCreated, got {other:?}"),
    };

    runtime.session().get_transaction(&tx_id).expect("transaction must exist in session")
}

/// Loads one content set in its own transaction.
///
/// `LoadHolons` commits the transaction it runs in, so every load — and every command after the
/// last one — needs a fresh transaction rather than the one it just closed.
async fn load(runtime: &Runtime, content_set: ContentSet, label: &str) {
    let context = begin_transaction(runtime).await;
    let response = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context,
                action: TransactionAction::LoadHolons { content_set },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("{label} failed: {error:?}"));

    let MapResult::Reference(HolonReference::Transient(response)) = response else {
        panic!("{label} returned an unexpected load response");
    };
    let errors = response
        .property_value(&CorePropertyTypeName::ErrorCount.as_property_name())
        .unwrap_or_else(|error| panic!("{label} response could not read ErrorCount: {error:?}"));
    let error_messages = response
        .related_holons(&CoreRelationshipTypeName::HasLoadError)
        .ok()
        .and_then(|errors| errors.read().ok().map(|errors| errors.get_members().clone()))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|error| {
            error
                .property_value(&CorePropertyTypeName::ErrorMessage.as_property_name())
                .ok()
                .flatten()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        errors,
        Some(PropertyValue::IntegerValue(MapInteger(0))),
        "{label} reported loader errors: {error_messages:?}"
    );
}

/// Resolves a committed holon by key through `GetAllHolons`.
async fn saved_reference_by_key(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    key: &str,
) -> HolonReference {
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(context),
                action: TransactionAction::GetAllHolons,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("get_all_holons failed");

    let collection = match result {
        MapResult::Collection(collection) => collection,
        other => panic!("expected Collection, got {other:?}"),
    };

    collection
        .get_by_key(&MapString(key.to_string()))
        .unwrap_or_else(|error| panic!("lookup of '{key}' failed: {error:?}"))
        .unwrap_or_else(|| panic!("no committed holon with key '{key}'"))
}

fn local_id_of(reference: &HolonReference) -> LocalId {
    reference.holon_id().expect("committed holon must have an id").local_id().clone()
}

/// Reads live SmartLinks straight from storage, bypassing the reference layer's caching.
async fn live_smartlinks(
    backend: &MockConductorConfig,
    source_id: &LocalId,
    relationship_name: &RelationshipName,
) -> Vec<SmartLink> {
    backend
        .conductor
        .call(
            &backend.cell.zome(ZOME),
            "smartlink_expand",
            (source_id.clone(), relationship_name.clone()),
        )
        .await
}

async fn stage_saved_holon(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    holon_id: LocalId,
    label: &str,
) -> HolonReference {
    match runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(context),
                action: TransactionAction::StageNewVersionFromId {
                    holon_id: HolonId::Local(holon_id),
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("staging {label} failed: {error:?}"))
    {
        MapResult::Reference(reference) => reference,
        other => panic!("staging {label}: expected a Reference, got {other:?}"),
    }
}

async fn add_relationship(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    source: HolonReference,
    relationship: &str,
    targets: Vec<HolonReference>,
    label: &str,
) {
    runtime
        .execute_command(
            MapCommand::Holon(HolonCommand {
                context: Arc::clone(context),
                target: source,
                action: HolonAction::Write(WritableHolonAction::AddRelatedHolons {
                    name: rel(relationship),
                    holons: targets,
                }),
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("adding {label} failed: {error:?}"));
}

fn commit_status(result: MapResult) -> String {
    let response = match result {
        MapResult::Reference(HolonReference::Transient(response)) => response,
        other => panic!("expected a transient CommitResponse reference, got {other:?}"),
    };
    match response.property_value(&CorePropertyTypeName::CommitRequestStatus) {
        Ok(Some(PropertyValue::StringValue(MapString(status)))) => status,
        other => panic!("expected a string CommitRequestStatus, got {other:?}"),
    }
}

fn assert_exact_targets(links: &[SmartLink], expected: &[LocalId], label: &str) {
    assert_eq!(links.len(), expected.len(), "{label}: unexpected link count: {links:?}");
    for expected_id in expected {
        assert_eq!(
            links
                .iter()
                .filter(|link| link.target_id == HolonId::Local(expected_id.clone()))
                .count(),
            1,
            "{label}: expected exactly one link to {expected_id:?}: {links:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn smartlink_conflict_downgrades_commit_to_incomplete() {
    let backend = setup_probe_enabled_conductor().await;
    let runtime = runtime_over(Arc::clone(&backend)).await;

    // --- Phase 1: schema, then one described Book instance ---------------------------------
    load(&runtime, build_core_schema_content_set().unwrap(), "core schema load").await;
    load(&runtime, build_book_person_inverse_content_set().unwrap(), "book/person schema load")
        .await;

    let context = begin_transaction(&runtime).await;
    let title_property_id =
        local_id_of(&saved_reference_by_key(&runtime, &context, TITLE_PROPERTY_KEY).await);

    let book_id = create_described_book(&runtime).await;

    // --- Phase 2: stage the graph-only relationship add ------------------------------------
    // `ReferencesProperty` is non-definitional, so this stays a graph-only edit and Pass 2 anchors the
    // link to the book's *existing* id — the only commit shape that can collide with a live link.
    // Nothing is planted yet: see the module doc on why planting first makes the add a no-op.
    let context = begin_transaction(&runtime).await;
    // Re-resolved in this transaction: the reference from Phase 1 belongs to a closed one.
    let title_property = saved_reference_by_key(&runtime, &context, TITLE_PROPERTY_KEY).await;
    let staged = match runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::StageNewVersionFromId {
                    holon_id: HolonId::Local(book_id.clone()),
                },
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("staging the book for update failed")
    {
        MapResult::Reference(reference) => reference,
        other => panic!("expected a staged Reference, got {other:?}"),
    };

    runtime
        .execute_command(
            MapCommand::Holon(HolonCommand {
                context: Arc::clone(&context),
                target: staged,
                action: HolonAction::Write(WritableHolonAction::AddRelatedHolons {
                    name: rel(REFERENCES_PROPERTY),
                    holons: vec![title_property.clone()],
                }),
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("adding the ReferencesProperty relationship failed");

    // --- Phase 3: a foreign writer lands the same edge, with a different key ----------------
    // Between staging and commit, so the staged collection cannot absorb it. The identity matches
    // what Pass 2 is about to write (source + target + relationship, no occurrence); only the
    // canonical key differs, which is exactly the `Conflict` condition. Authored through the probe
    // zome because no supported write path produces a stale-keyed row.
    let stale_tag = encode_smartlink_tag(&SmartLinkTagInput {
        target_id: HolonId::Local(title_property_id.clone()),
        relationship_name: rel(REFERENCES_PROPERTY),
        canonical_key: CanonicalKey::new(STALE_KEY).unwrap(),
        occurrence_id: None,
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    })
    .expect("stale tag must encode");

    let _planted: LocalId = backend
        .conductor
        .call(
            &backend.cell.zome(PROBE_ZOME),
            "smartlink_author_raw_tag_for_test",
            (book_id.clone(), title_property_id.clone(), stale_tag),
        )
        .await;

    let planted = live_smartlinks(&backend, &book_id, &rel(REFERENCES_PROPERTY)).await;
    assert_eq!(planted.len(), 1, "setup: exactly one planted link should be live");
    assert_eq!(
        planted[0].canonical_key.as_str(),
        STALE_KEY,
        "setup: the planted link must carry the stale key"
    );

    let commit_result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("commit itself returns Ok; the conflict surfaces in the response status");

    // --- Phase 4: the assertions ------------------------------------------------------------
    // The one that carries the test. Relaxing `persist_smartlink`'s Conflict arm to `Ok(())`
    // reports "Complete" here while the relationship remains unpersisted.
    let response = match commit_result {
        MapResult::Reference(HolonReference::Transient(response)) => response,
        other => panic!("expected a transient CommitResponse reference, got {other:?}"),
    };
    let status = match response.property_value(&CorePropertyTypeName::CommitRequestStatus) {
        Ok(Some(PropertyValue::StringValue(MapString(status)))) => status,
        other => panic!("expected a string CommitRequestStatus, got {other:?}"),
    };
    let after = live_smartlinks(&backend, &book_id, &rel(REFERENCES_PROPERTY)).await;
    let from_target: Vec<SmartLink> = backend
        .conductor
        .call(&backend.cell.zome(ZOME), "smartlink_expand_all", title_property_id.clone())
        .await;
    assert_eq!(
        status,
        "Incomplete",
        "a SmartLink conflict must downgrade the commit response, not report success.\n\
         book_id                = {:?}\n\
         {REFERENCES_PROPERTY} from book = {:?}\n\
         links from the target  = {:?}",
        book_id,
        after
            .iter()
            .map(|link| (link.canonical_key.as_str().to_string(), link.target_id.clone()))
            .collect::<Vec<_>>(),
        from_target
            .iter()
            .map(|link| (
                link.relationship_name.0 .0.clone(),
                link.canonical_key.as_str().to_string(),
                link.target_id.clone()
            ))
            .collect::<Vec<_>>()
    );

    // Supporting: the requested link was not written and the planted row is untouched.
    assert_eq!(after.len(), 1, "the conflicting link must not have been persisted");
    assert_eq!(
        after[0].canonical_key.as_str(),
        STALE_KEY,
        "the planted link must survive the failed commit unchanged"
    );

    // Pass 2 stops at the first relationship error, so the inverse direction is never reached.
    // Asserted by target identity rather than by inverse relationship name, so the assertion does
    // not depend on how the schema names the reciprocal edge.
    assert!(
        !from_target.iter().any(|link| link.target_id == HolonId::Local(book_id.clone())),
        "the inverse link back to the book must not be written after a forward conflict"
    );
}

/// Relationship pass 2 must share its SmartLink write context across every staged holon in one
/// commit. This ordinary domain scenario covers both repeated declared and repeated inverse
/// buckets without relying on an instance-to-descriptor relationship:
///
/// - `Book.Cache.1 --AuthoredBy--> [Person.Cache.1, Person.Cache.2]` repeats the declared bucket;
/// - `Book.Cache.2 --AuthoredBy--> Person.Cache.1` repeats Person.Cache.1's `AuthorOf` inverse
///   bucket after the first Book's inverse has been materialized.
#[tokio::test(flavor = "multi_thread")]
async fn commit_reuses_smartlink_buckets_for_declared_and_inverse_relationships() {
    let backend = setup_probe_enabled_conductor().await;
    let runtime = runtime_over(Arc::clone(&backend)).await;

    load(&runtime, build_core_schema_content_set().unwrap(), "core schema load").await;
    load(&runtime, build_book_person_inverse_content_set().unwrap(), "book/person schema load")
        .await;

    let book_1_id = create_described_holon(&runtime, BOOK_CACHE_1_KEY, BOOK_DESCRIPTOR_KEY).await;
    let book_2_id = create_described_holon(&runtime, BOOK_CACHE_2_KEY, BOOK_DESCRIPTOR_KEY).await;
    let person_1_id =
        create_described_holon(&runtime, PERSON_CACHE_1_KEY, PERSON_DESCRIPTOR_KEY).await;
    let person_2_id =
        create_described_holon(&runtime, PERSON_CACHE_2_KEY, PERSON_DESCRIPTOR_KEY).await;

    // One transaction covers both repeated declared writes from Book.Cache.1 and repeated inverse
    // writes from Person.Cache.1. The relationship is ordinary instance-to-instance domain data.
    let context = begin_transaction(&runtime).await;
    let book_1 = stage_saved_holon(&runtime, &context, book_1_id, BOOK_CACHE_1_KEY).await;
    let book_2 = stage_saved_holon(&runtime, &context, book_2_id, BOOK_CACHE_2_KEY).await;
    let person_1 = saved_reference_by_key(&runtime, &context, PERSON_CACHE_1_KEY).await;
    let person_2 = saved_reference_by_key(&runtime, &context, PERSON_CACHE_2_KEY).await;

    add_relationship(
        &runtime,
        &context,
        book_1,
        AUTHORED_BY,
        vec![person_1.clone(), person_2.clone()],
        "Book.Cache.1 --AuthoredBy--> Person.Cache.1 and Person.Cache.2",
    )
    .await;
    add_relationship(
        &runtime,
        &context,
        book_2,
        AUTHORED_BY,
        vec![person_1],
        "Book.Cache.2 --AuthoredBy--> Person.Cache.1",
    )
    .await;

    let first_commit = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: Arc::clone(&context),
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("first relationship commit failed");
    assert_eq!(commit_status(first_commit), "Complete");

    // AuthoredBy is definitional, so both Books now resolve to their new committed versions.
    let context = begin_transaction(&runtime).await;
    let committed_book_1_id =
        local_id_of(&saved_reference_by_key(&runtime, &context, BOOK_CACHE_1_KEY).await);
    let committed_book_2_id =
        local_id_of(&saved_reference_by_key(&runtime, &context, BOOK_CACHE_2_KEY).await);

    let book_1_authored = live_smartlinks(&backend, &committed_book_1_id, &rel(AUTHORED_BY)).await;
    let book_2_authored = live_smartlinks(&backend, &committed_book_2_id, &rel(AUTHORED_BY)).await;
    let person_1_authored = live_smartlinks(&backend, &person_1_id, &rel(AUTHOR_OF)).await;
    let person_2_authored = live_smartlinks(&backend, &person_2_id, &rel(AUTHOR_OF)).await;

    assert_exact_targets(
        &book_1_authored,
        &[person_1_id.clone(), person_2_id.clone()],
        "Book.Cache.1 AuthoredBy",
    );
    assert_exact_targets(&book_2_authored, &[person_1_id.clone()], "Book.Cache.2 AuthoredBy");
    assert_exact_targets(
        &person_1_authored,
        &[committed_book_1_id.clone(), committed_book_2_id.clone()],
        "Person.Cache.1 AuthorOf",
    );
    assert_exact_targets(
        &person_2_authored,
        &[committed_book_1_id.clone()],
        "Person.Cache.2 AuthorOf",
    );

    // A no-op transaction must leave the established directional links untouched.
    let no_op_context = begin_transaction(&runtime).await;
    let no_op_commit = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: no_op_context,
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .expect("no-op commit failed");
    assert_eq!(commit_status(no_op_commit), "Complete");

    assert_exact_targets(
        &live_smartlinks(&backend, &committed_book_1_id, &rel(AUTHORED_BY)).await,
        &[person_1_id.clone(), person_2_id.clone()],
        "Book.Cache.1 AuthoredBy after no-op commit",
    );
    assert_exact_targets(
        &live_smartlinks(&backend, &committed_book_2_id, &rel(AUTHORED_BY)).await,
        &[person_1_id.clone()],
        "Book.Cache.2 AuthoredBy after no-op commit",
    );
    assert_exact_targets(
        &live_smartlinks(&backend, &person_1_id, &rel(AUTHOR_OF)).await,
        &[committed_book_1_id.clone(), committed_book_2_id],
        "Person.Cache.1 AuthorOf after no-op commit",
    );
    assert_exact_targets(
        &live_smartlinks(&backend, &person_2_id, &rel(AUTHOR_OF)).await,
        &[committed_book_1_id],
        "Person.Cache.2 AuthorOf after no-op commit",
    );
}
