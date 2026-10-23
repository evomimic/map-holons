//! Isolated probe-backed Commit retries through the fixture expectation model.

// This target runs initialized cases; suite bootstrap entrypoints are unused.
#[allow(dead_code)]
mod execution_steps;
mod fixture_cases {
    pub mod commit_incomplete_fixture;
    pub mod described_instance_helpers;
}

use core_types::{encode_smartlink_tag, CanonicalKey, DeleteSmartLinkOutcome, SmartLinkTagInput};
use execution_steps::dance_test_runner::run_dance_test_case;
use fixture_cases::commit_incomplete_fixture::commit_incomplete_fixture;
use holons_core::core_shared_objects::holon::StagedState;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::init_probe_test_runtime;
use holons_test::{
    assert_exact_relationship_ids, DanceTestStep, DancesTestCase, ResolveBy, TestExecutionState,
    TestReference,
};
use integrity_core_types::LocalId;
use pretty_assertions::assert_eq;

const STALE_KEY: &str = "stale.title.property";

/// Executes a contiguous part of the finalized plan, allowing injection only between steps.
async fn run_segment(
    case: &DancesTestCase,
    steps: Vec<DanceTestStep>,
    state: &mut TestExecutionState,
    schema_loaded: &mut bool,
    query_schema_loaded: &mut bool,
) {
    let mut segment = case.clone();
    segment.steps = steps;
    run_dance_test_case(segment, state, schema_loaded, query_schema_loaded).await;
}

fn saved_id(state: &TestExecutionState, token: &TestReference) -> LocalId {
    let reference = state
        .resolve_execution_reference(&state.context(), ResolveBy::Expected, token)
        .expect("saved token remains usable");
    assert!(matches!(reference, HolonReference::Smart(_)), "token must stay saved");
    reference.holon_id().unwrap().local_id().clone()
}

/// Skipped in CI for now (`ci-rest` filter in `.config/nextest.toml`): it timed out at 600s and
/// then at 1200s on a hosted runner, beyond comparable probe-conductor tests. Runs locally.
#[tokio::test(flavor = "multi_thread")]
async fn incomplete_commit_repeats_error_then_completes_relationship_retry() {
    let scenario = commit_incomplete_fixture().unwrap();
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id, control) = init_probe_test_runtime(&mut bootstrap).await;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        scenario.case.test_session_state.get_transient_holons().clone(),
        scenario.case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    let mut schema_loaded = false;
    let mut query_schema_loaded = false;
    let case = &scenario.case;
    let mut next_step = 0;
    let mut expected_inverse_sources = Vec::new();
    for sequence in &scenario.attempts {
        run_segment(
            case,
            case.steps[next_step..sequence.first_attempt].to_vec(),
            &mut state,
            &mut schema_loaded,
            &mut query_schema_loaded,
        )
        .await;
        let context = state.context();
        let book_id = saved_id(&state, &sequence.book);
        let title_id = saved_id(&state, &sequence.title);
        let name_id = saved_id(&state, &sequence.name);
        let retained_update = context
            .staged_references()
            .unwrap()
            .into_iter()
            .find(|reference| reference.versioned_source_id().unwrap() == Some(book_id.clone()))
            .expect("retain the graph-only update across all attempts");
        assert!(retained_update.commit_errors().unwrap().is_empty());
        let retained_unchanged = sequence.unchanged.as_ref().map(|unchanged| {
            let source_id = saved_id(&state, &unchanged.source);
            let reference = state
                .resolve_execution_reference(&context, ResolveBy::Expected, &unchanged.staged)
                .unwrap();
            let HolonReference::Staged(reference) = reference else {
                panic!("unchanged candidate must start staged");
            };
            (unchanged, reference, source_id)
        });
        let encoded_tag = encode_smartlink_tag(&SmartLinkTagInput {
            target_id: HolonId::Local(title_id.clone()),
            relationship_name: "ReferencesProperty".to_relationship_name(),
            canonical_key: CanonicalKey::new(STALE_KEY).unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        })
        .unwrap();
        // Plant only after staging: an earlier row would be absorbed as an untouched no-op.
        let planted_id =
            control.plant_stale_link(book_id.clone(), title_id.clone(), encoded_tag).await;
        let planted =
            control.live_links(book_id.clone(), "ReferencesProperty".to_relationship_name()).await;
        assert_eq!(planted.len(), 1);
        assert_eq!(planted[0].canonical_key.as_str(), STALE_KEY);

        for attempt in 0..2 {
            run_segment(
                case,
                vec![case.steps[sequence.first_attempt + attempt].clone()],
                &mut state,
                &mut schema_loaded,
                &mut query_schema_loaded,
            )
            .await;
            assert!(context.is_open(), "Incomplete leaves the same transaction open");
            assert_eq!(state.context().tx_id(), context.tx_id());
            assert_eq!(
                saved_id(&state, &sequence.saved_update),
                book_id,
                "saved mapping survives retries"
            );
            assert_eq!(retained_update.commit_errors().unwrap().len(), attempt + 1);
            let response = state.last_commit_response().unwrap();
            assert_eq!(
                response.property_value(CorePropertyTypeName::CommitsAttempted).unwrap(),
                Some(
                    MapInteger(if attempt == 0 {
                        2 + i64::from(retained_unchanged.is_some())
                    } else {
                        i64::from(retained_unchanged.is_some())
                    })
                    .to_base_value()
                )
            );
            assert_eq!(
                response
                    .related_holons(CoreRelationshipTypeName::SavedHolons)
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_count(),
                MapInteger(if attempt == 0 { 2 } else { 0 })
            );
            if let Some((unchanged, retained, source_id)) = &retained_unchanged {
                let reference = state
                    .resolve_execution_reference(&context, ResolveBy::Expected, &unchanged.staged)
                    .unwrap();
                let HolonReference::Staged(reference) = reference else {
                    panic!("Incomplete must leave the unchanged candidate token staged");
                };
                assert_eq!(reference.temporary_id(), retained.temporary_id());
                assert_eq!(reference.staged_state().unwrap(), StagedState::ForUpdate);
                assert!(reference.is_live_validation_candidate().unwrap());
                assert!(reference.commit_errors().unwrap().is_empty());
                assert_eq!(
                    saved_id(&state, &unchanged.source),
                    *source_id,
                    "persisted reads still use the unchanged candidate's source token"
                );
            }
            let links = control
                .live_links(book_id.clone(), "ReferencesProperty".to_relationship_name())
                .await;
            assert_eq!(
                links.len(),
                2,
                "one successful Name occurrence plus the stale Title conflict"
            );
            assert_eq!(
                links
                    .iter()
                    .filter(|link| link.target_id == HolonId::Local(name_id.clone()))
                    .count(),
                1
            );
            assert_eq!(
                links
                    .iter()
                    .find(|link| link.target_id == HolonId::Local(title_id.clone()))
                    .unwrap()
                    .canonical_key
                    .as_str(),
                STALE_KEY
            );
            let inverse = control
                .live_links(title_id.clone(), "ReferencedByBook".to_relationship_name())
                .await;
            assert!(
                inverse.iter().all(|link| link.target_id != HolonId::Local(book_id.clone())),
                "the Title inverse must remain absent while its forward persistence fails"
            );
        }
        assert_eq!(control.delete_link(planted_id).await, DeleteSmartLinkOutcome::Deleted);
        run_segment(
            case,
            case.steps[sequence.first_attempt + 2..sequence.end_step].to_vec(),
            &mut state,
            &mut schema_loaded,
            &mut query_schema_loaded,
        )
        .await;
        assert!(!context.is_open(), "Complete closes the transaction");
        assert_eq!(saved_id(&state, &sequence.saved_update), book_id);
        let response = state.last_commit_response().unwrap();
        assert_eq!(
            response.property_value(CorePropertyTypeName::CommitsAttempted).unwrap(),
            Some(MapInteger(i64::from(retained_unchanged.is_some())).to_base_value())
        );
        assert_eq!(
            response
                .related_holons(CoreRelationshipTypeName::SavedHolons)
                .unwrap()
                .read()
                .unwrap()
                .get_count(),
            MapInteger(0)
        );
        if let Some((unchanged, _, source_id)) = &retained_unchanged {
            assert_eq!(
                saved_id(&state, &unchanged.saved),
                *source_id,
                "Complete binds the unchanged candidate's head to its saved source"
            );
        }
        assert_eq!(
            retained_update.commit_errors().unwrap().len(),
            2,
            "successful retry appends no errors"
        );
        let links =
            control.live_links(book_id.clone(), "ReferencesProperty".to_relationship_name()).await;
        assert_eq!(links.len(), 2, "only the two requested forward identities remain");
        assert!(links.iter().all(|link| link.canonical_key.as_str() != STALE_KEY));
        expected_inverse_sources.push(book_id);
        for target in [name_id, title_id] {
            let inverse =
                control.live_links(target, "ReferencedByBook".to_relationship_name()).await;
            assert_exact_relationship_ids(
                "known inverse sources after successful replay",
                inverse.iter().map(|link| link.target_id.local_id().clone()).collect(),
                expected_inverse_sources.clone(),
            );
        }
        next_step = sequence.end_step;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn outstanding_graph_replay_rejects_removal_then_composes_live_addition() {
    let scenario = fixture_cases::commit_incomplete_fixture::graph_overlap_retry_fixture().unwrap();
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id, control) = init_probe_test_runtime(&mut bootstrap).await;
    let case = &scenario.case;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        case.test_session_state.get_transient_holons().clone(),
        case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    let mut loaded = false;
    let mut query_loaded = false;
    run_segment(
        case,
        case.steps[..scenario.first_attempt].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    let source = saved_id(&state, &scenario.source);
    let target = saved_id(&state, &scenario.added);
    let encoded = encode_smartlink_tag(&SmartLinkTagInput {
        target_id: HolonId::Local(target.clone()),
        relationship_name: "ReferencesProperty".to_relationship_name(),
        canonical_key: CanonicalKey::new(STALE_KEY).unwrap(),
        occurrence_id: None,
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    })
    .unwrap();
    let planted = control.plant_stale_link(source.clone(), target, encoded).await;
    run_segment(
        case,
        vec![case.steps[scenario.first_attempt].clone()],
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    assert_eq!(control.delete_link(planted).await, DeleteSmartLinkOutcome::Deleted);
    let before =
        control.live_links(source.clone(), "ReferencesProperty".to_relationship_name()).await;
    run_segment(
        case,
        case.steps[scenario.first_attempt + 1..scenario.after_rejection].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    let after = control.live_links(source, "ReferencesProperty".to_relationship_name()).await;
    assert_eq!(before, after, "rejected retry must perform no relationship writes");
    let context = state.context();
    let live = context
        .staged_references()
        .unwrap()
        .into_iter()
        .filter(|c| c.is_live_validation_candidate().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(live.len(), 1);
    let findings = live[0].validation_findings().unwrap();
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("outstanding replay from an incomplete Commit"));
    run_segment(
        case,
        case.steps[scenario.after_rejection..].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn published_rekey_is_reserved_before_key_index_retry_repairs_owns() {
    use core_types::OccurrenceId;
    use sha2::{Digest, Sha256};
    let scenario = fixture_cases::commit_incomplete_fixture::key_index_retry_fixture().unwrap();
    let mut bootstrap = DancesTestCase::default();
    let (runtime, tx_id, control) = init_probe_test_runtime(&mut bootstrap).await;
    let case = &scenario.case;
    let mut state = TestExecutionState::new(
        runtime,
        tx_id.clone(),
        case.test_session_state.get_transient_holons().clone(),
        case.test_session_state.fixture_head_index().clone(),
    );
    state.activate_transaction(tx_id).unwrap();
    let mut loaded = false;
    let mut query_loaded = false;
    run_segment(
        case,
        case.steps[..scenario.first_attempt].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    let root = saved_id(&state, &scenario.root);
    let space =
        state.context().get_space_holon().unwrap().unwrap().holon_id().unwrap().local_id().clone();
    // The historical-key index uses a stable occurrence derived from the complete key.
    // Occupy precisely that identity with a stale key so publication succeeds but indexing fails.
    let mut hasher = Sha256::new();
    hasher.update(b"map-holons:owns-key-index:v1\0");
    hasher.update(scenario.key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    let encoded = encode_smartlink_tag(&SmartLinkTagInput {
        target_id: HolonId::Local(root.clone()),
        relationship_name: CoreRelationshipTypeName::Owns.as_relationship_name(),
        canonical_key: CanonicalKey::new("stale.key-index-retry").unwrap(),
        occurrence_id: Some(OccurrenceId(bytes)),
        relationship_property_values: PropertyMap::new(),
        target_property_cache_candidates: Vec::new(),
    })
    .unwrap();
    let planted = control.plant_stale_link(space.clone(), root.clone(), encoded).await;
    run_segment(
        case,
        vec![case.steps[scenario.first_attempt].clone()],
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    assert!(!control
        .live_links(space.clone(), CoreRelationshipTypeName::Owns.as_relationship_name())
        .await
        .iter()
        .any(|link| link.canonical_key.as_str() == scenario.key));
    assert_eq!(control.delete_link(planted).await, DeleteSmartLinkOutcome::Deleted);
    let mut before = control
        .live_links(space.clone(), CoreRelationshipTypeName::Owns.as_relationship_name())
        .await;
    run_segment(
        case,
        case.steps[scenario.first_attempt + 1..scenario.after_rejection].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    before.sort_by_key(|row| row.smartlink_id.0 .0.clone());
    let mut after = control
        .live_links(space.clone(), CoreRelationshipTypeName::Owns.as_relationship_name())
        .await;
    after.sort_by_key(|row| row.smartlink_id.0 .0.clone());
    assert_eq!(before, after, "rejected collision cannot repair or overwrite the index");
    run_segment(
        case,
        case.steps[scenario.after_rejection..].to_vec(),
        &mut state,
        &mut loaded,
        &mut query_loaded,
    )
    .await;
    let rows =
        control.live_links(space, CoreRelationshipTypeName::Owns.as_relationship_name()).await;
    for key in ["Book.KeyIndexRetry.Old", scenario.key] {
        let rows: Vec<_> = rows.iter().filter(|row| row.canonical_key.as_str() == key).collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].target_id, HolonId::Local(root.clone()));
    }
}
