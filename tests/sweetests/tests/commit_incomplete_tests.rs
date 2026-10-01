//! Isolated probe-backed Commit retries through the fixture expectation model.

// This target runs initialized cases; suite bootstrap entrypoints are unused.
#[allow(dead_code)]
mod execution_steps;
mod fixture_cases {
    pub mod commit_incomplete_fixture;
    pub mod described_instances;
}

use core_types::{encode_smartlink_tag, CanonicalKey, DeleteSmartLinkOutcome, SmartLinkTagInput};
use execution_steps::dance_test_runner::run_dance_test_case;
use fixture_cases::commit_incomplete_fixture::commit_incomplete_fixture;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::init_probe_test_runtime;
use holons_test::{DanceTestStep, DancesTestCase, ResolveBy, TestExecutionState, TestReference};
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
    run_segment(
        &scenario.case,
        scenario.case.steps[..scenario.first_attempt].to_vec(),
        &mut state,
        &mut schema_loaded,
        &mut query_schema_loaded,
    )
    .await;
    let context = state.context();
    let book_id = saved_id(&state, &scenario.book);
    let title_id = saved_id(&state, &scenario.title);
    let name_id = saved_id(&state, &scenario.name);
    let retained_update = context
        .staged_references()
        .unwrap()
        .into_iter()
        .find(|reference| reference.versioned_source_id().unwrap() == Some(book_id.clone()))
        .expect("retain the graph-only update across all attempts");
    assert!(retained_update.commit_errors().unwrap().is_empty());
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
    let planted_id = control.plant_stale_link(book_id.clone(), title_id.clone(), encoded_tag).await;
    let planted =
        control.live_links(book_id.clone(), "ReferencesProperty".to_relationship_name()).await;
    assert_eq!(planted.len(), 1);
    assert_eq!(planted[0].canonical_key.as_str(), STALE_KEY);

    for attempt in 0..2 {
        run_segment(
            &scenario.case,
            vec![scenario.case.steps[scenario.first_attempt + attempt].clone()],
            &mut state,
            &mut schema_loaded,
            &mut query_schema_loaded,
        )
        .await;
        assert!(context.is_open(), "Incomplete leaves the same transaction open");
        assert_eq!(state.context().tx_id(), context.tx_id());
        assert_eq!(
            saved_id(&state, &scenario.saved_update),
            book_id,
            "saved mapping survives retries"
        );
        assert_eq!(retained_update.commit_errors().unwrap().len(), attempt + 1);
        let links =
            control.live_links(book_id.clone(), "ReferencesProperty".to_relationship_name()).await;
        assert_eq!(links.len(), 2, "one successful Name occurrence plus the stale Title conflict");
        assert_eq!(
            links.iter().filter(|link| link.target_id == HolonId::Local(name_id.clone())).count(),
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
        let inverse =
            control.live_links(title_id.clone(), "ReferencedByBook".to_relationship_name()).await;
        assert!(
            inverse.iter().all(|link| link.target_id != HolonId::Local(book_id.clone())),
            "the Title inverse must remain absent while its forward persistence fails"
        );
    }
    assert_eq!(control.delete_link(planted_id).await, DeleteSmartLinkOutcome::Deleted);
    run_segment(
        &scenario.case,
        scenario.case.steps[scenario.first_attempt + 2..].to_vec(),
        &mut state,
        &mut schema_loaded,
        &mut query_schema_loaded,
    )
    .await;
    assert_eq!(saved_id(&state, &scenario.saved_update), book_id);
    assert_eq!(
        retained_update.commit_errors().unwrap().len(),
        2,
        "successful retry appends no errors"
    );
    let links =
        control.live_links(book_id.clone(), "ReferencesProperty".to_relationship_name()).await;
    assert_eq!(links.len(), 2, "only the two requested forward identities remain");
    assert!(links.iter().all(|link| link.canonical_key.as_str() != STALE_KEY));
    for target in [name_id, title_id] {
        let inverse = control.live_links(target, "ReferencedByBook".to_relationship_name()).await;
        assert_eq!(inverse.len(), 1, "exactly one inverse row after successful replay");
        assert_eq!(inverse[0].target_id, HolonId::Local(book_id.clone()));
    }
}
