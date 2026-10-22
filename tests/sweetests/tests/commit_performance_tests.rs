//! Single-instance Commit measurements through the ordinary command/reference surfaces.

use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use holons_test::{init_commit_performance_runtime, init_test_runtime, DancesTestCase};
#[path = "performance_support/link_index.rs"]
mod link_index;
#[path = "performance_support/new_version_samples.rs"]
mod new_version_samples;
#[path = "performance_support/readiness_cases.rs"]
mod readiness_cases;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::ExecutionPolicy;
use std::time::Instant;
use tracing::info;

#[tokio::test(flavor = "multi_thread")]
async fn measures_first_and_repeated_visualizer_usage_commits() {
    let mut bootstrap = DancesTestCase::default();
    let index_mode = std::env::var("MAP_COMMIT_PERF_INDEX_MODE").unwrap_or_else(|_| "none".into());
    let (runtime, initial_tx_id) = if index_mode == "none" {
        init_test_runtime(&mut bootstrap).await
    } else {
        assert!(index_mode == "control" || index_mode == "base-hash");
        let (runtime, tx_id, backend) = init_commit_performance_runtime(&mut bootstrap).await;
        let path =
            std::env::var("MAP_COMMIT_PERF_INDEX_OUTPUT").expect("index evidence output required");
        link_index::profile(&backend, index_mode == "base-hash", &path).await;
        (runtime, tx_id)
    };
    runtime.session().archive_transaction(&initial_tx_id).unwrap();

    // Each sample creates a new usage with the same four outbound members. Shared inverse
    // buckets grow by one per sample; this is repeated execution, not identical storage state.
    let samples = std::env::var("MAP_COMMIT_PERF_SAMPLES")
        .map(|value| value.parse::<usize>().expect("sample count must be a positive integer"))
        .unwrap_or(6);
    assert!(samples > 0, "sample count must be positive");
    let subjects = std::env::var("MAP_COMMIT_PERF_SUBJECTS")
        .map(|value| value.parse::<usize>().expect("subject count must be a positive integer"))
        .unwrap_or(1);
    assert!(subjects > 0);
    let mode = std::env::var("MAP_COMMIT_PERF_MODE").unwrap_or_else(|_| "create".into());
    if mode == "new-version" {
        assert_eq!(subjects, 1, "new-version samples publish one successor per Commit");
        new_version_samples::run(&runtime, samples).await;
        return;
    }
    assert_eq!(mode, "create");
    for sample in 0..samples {
        let MapResult::TransactionCreated { tx_id } = runtime
            .execute_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap()
        else {
            panic!("BeginTransaction must return a transaction");
        };
        let context = runtime.session().get_transaction(&tx_id).unwrap();
        let lookup = |key: &str| -> HolonReference {
            context.lookup().get_saved_holon_by_key(&MapString::from(key)).unwrap().into()
        };
        let descriptor = lookup("VisualizerUsage.HolonType");
        let visualizer = lookup("DefaultPropertyMapVisualizer.PropertyMapVisualizer");
        let subject = lookup("HolonType.TypeDescriptor");
        let owner = context.get_space_holon().unwrap().unwrap();
        for subject_index in 0..subjects {
            let mut usage = context
                .mutation()
                .new_holon(Some(format!("commit-baseline-usage-{sample}-{subject_index}").into()))
                .unwrap();
            usage.add_related_holons("DescribedBy", vec![descriptor.clone()]).unwrap();
            usage.add_related_holons("UsesVisualizer", vec![visualizer.clone()]).unwrap();
            usage.add_related_holons("ForSubjectType", vec![subject.clone()]).unwrap();
            let staged = context.mutation().stage_new_holon(usage).unwrap();
            assert_eq!(
                HolonReference::from(&staged)
                    .all_related_holons()
                    .unwrap()
                    .iter()
                    .into_iter()
                    .map(|(_, members)| members.read().unwrap().get_members().len())
                    .sum::<usize>(),
                4
            );
        }

        let condition = if sample == 0 { "first" } else { "repeated" };
        info!("[commit-baseline] sample_start={} condition={} candidates={} outbound_members={} prior_usages={}", sample, condition, subjects, subjects * 4, sample * subjects);
        let started = Instant::now();
        let response = runtime
            .execute_command(
                MapCommand::Transaction(TransactionCommand {
                    context: context.clone(),
                    action: TransactionAction::Commit,
                }),
                ExecutionPolicy::default(),
            )
            .await
            .unwrap();
        let command_us = started.elapsed().as_micros();
        let MapResult::Reference(response) = response else {
            panic!("Commit must return a response");
        };
        assert_eq!(
            response.property_value("CommitRequestStatus").unwrap(),
            Some("Complete".to_base_value())
        );
        assert_eq!(
            response.property_value("CommitsAttempted").unwrap(),
            Some((subjects as i64).to_base_value())
        );
        assert_eq!(
            response.property_value("ValidationViolationCount").unwrap(),
            Some(0_i64.to_base_value())
        );
        let saved =
            response.related_holons("SavedHolons").unwrap().read().unwrap().get_members().clone();
        assert_eq!(saved.len(), subjects);
        info!(
            "[commit-baseline] sample_end={} condition={} command_us={}",
            sample, condition, command_us
        );

        for saved_reference in &saved {
            let saved_id = saved_reference.holon_id().unwrap();
            // Fresh persisted forward and inverse checks are outside the measured command interval.
            for (forward, inverse, target) in [
                ("DescribedBy", "Instances", descriptor.clone()),
                ("UsesVisualizer", "UsedByVisualizerUsage", visualizer.clone()),
                ("ForSubjectType", "HasVisualizerUsage", subject.clone()),
                ("OwnedBy", "Owns", owner.clone()),
            ] {
                let members = saved_reference
                    .related_holons_with_hint(forward, RelationshipReadHint::RequireFresh)
                    .unwrap();
                assert_eq!(
                    members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .map(|member| member.holon_id().unwrap())
                        .collect::<Vec<_>>(),
                    vec![target.holon_id().unwrap()]
                );
                let members = target
                    .related_holons_with_hint(inverse, RelationshipReadHint::RequireFresh)
                    .unwrap();
                assert_eq!(
                    members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .filter(|member| member.holon_id().unwrap() == saved_id)
                        .count(),
                    1
                );
            }
        }
        runtime.session().archive_transaction(&tx_id).unwrap();
    }
    if let Ok(path) = std::env::var("MAP_COMMIT_READINESS_CASES_OUTPUT") {
        readiness_cases::run(&runtime, &path).await;
    }
}
