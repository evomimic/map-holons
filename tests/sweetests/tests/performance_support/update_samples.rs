//! Mixed and saved-source workloads. Setup and persisted verification are outside timing.
use holons_core::RelationshipReadHint;
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::{sync::Arc, time::Instant};
use tracing::info;

async fn begin(runtime: &Runtime) -> Arc<TransactionContext> {
    let MapResult::TransactionCreated { tx_id } = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("transaction expected")
    };
    runtime.session().get_transaction(&tx_id).unwrap()
}
async fn commit(runtime: &Runtime, context: &Arc<TransactionContext>) -> HolonReference {
    let MapResult::Reference(response) = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::Commit,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("reference expected")
    };
    assert_eq!(
        response.property_value("CommitRequestStatus").unwrap(),
        Some("Complete".to_base_value())
    );
    assert_eq!(
        response.property_value("ValidationViolationCount").unwrap(),
        Some(0_i64.to_base_value())
    );
    response
}
fn lookup(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
    context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
}
fn usage(context: &Arc<TransactionContext>, key: &str) {
    let mut usage = context.mutation().new_holon(Some(key.into())).unwrap();
    for (edge, key) in [
        ("DescribedBy", "VisualizerUsage.HolonType"),
        ("UsesVisualizer", "DefaultPropertyMapVisualizer.PropertyMapVisualizer"),
        ("ForSubjectType", "HolonType.TypeDescriptor"),
    ] {
        usage.add_related_holons(edge, vec![lookup(context, key)]).unwrap();
    }
    context.mutation().stage_new_holon(usage).unwrap();
}
pub async fn run(runtime: &Runtime, samples: usize, mode: &str) {
    for sample in 0..samples {
        let context = begin(runtime).await;
        let source = if mode.starts_with("graph-only") {
            let setup = begin(runtime).await;
            usage(&setup, &format!("graph-source-{sample}"));
            let response = commit(runtime, &setup).await;
            let saved = response
                .related_holons("SavedHolons")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            assert_eq!(saved.len(), 1);
            let id = saved[0].holon_id().unwrap();
            runtime.session().archive_transaction(&setup.tx_id()).unwrap();
            Some(id)
        } else {
            None
        };
        let candidates = if mode == "mixed" {
            super::creation_samples::stage_descriptor(
                &context,
                &format!("mixed-definition-{sample}"),
            );
            super::creation_samples::stage_visualizer_slot(
                &context,
                &format!("mixed-slot-{sample}"),
            );
            2
        } else if mode == "no-op" {
            let id =
                lookup(&context, "DefaultPropertyMapVisualizer.PropertySlot").holon_id().unwrap();
            context.mutation().stage_new_version_from_id(id).unwrap();
            1
        } else {
            let count = if mode == "graph-only-overlap" { 2 } else { 1 };
            for _ in 0..count {
                let mut staged =
                    context.mutation().stage_new_version_from_id(source.clone().unwrap()).unwrap();
                staged
                    .add_related_holons(
                        "SelectedForSlot",
                        vec![lookup(&context, "DefaultPropertyMapVisualizer.PropertySlot")],
                    )
                    .unwrap();
                assert_eq!(
                    staged.staged_state().unwrap(),
                    holons_core::core_shared_objects::holon::StagedState::ForUpdateGraphOnly,
                    "workload must exercise graph-only persistence"
                );
            }
            count
        };
        let condition = if sample == 0 { "first" } else { "repeated" };
        info!(
            "[commit-baseline] sample_start={} condition={} mode={} candidates={}",
            sample, condition, mode, candidates
        );
        let started = Instant::now();
        let response = commit(runtime, &context).await;
        let command_us = started.elapsed().as_micros();
        assert_eq!(
            response.property_value("CommitsAttempted").unwrap(),
            Some((candidates as i64).to_base_value())
        );
        let saved =
            response.related_holons("SavedHolons").unwrap().read().unwrap().get_members().clone();
        assert_eq!(
            saved.len(),
            match mode {
                "mixed" => 2,
                "no-op" => 0,
                "graph-only" | "graph-only-overlap" => candidates,
                _ => unreachable!("unsupported update workload"),
            },
            "graph-only candidates return saved-source aliases; unchanged updates return no result"
        );
        if let Some(source) = &source {
            for reference in &saved {
                assert_eq!(
                    reference.holon_id().unwrap(),
                    *source,
                    "graph-only updates must return the exact saved source, not a new version"
                );
            }
        }
        info!(
            "[commit-baseline] sample_end={} condition={} command_us={}",
            sample, condition, command_us
        );
        if let Some(source) = source {
            let saved = HolonReference::smart_from_id(context.space_read_handle(), source);
            let members = saved
                .related_holons_with_hint("SelectedForSlot", RelationshipReadHint::RequireFresh)
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .clone();
            assert_eq!(members.len(), 1, "overlapping identical puts persist exactly once");
            assert_eq!(
                members[0].holon_id().unwrap(),
                lookup(&context, "DefaultPropertyMapVisualizer.PropertySlot").holon_id().unwrap()
            );
        }
        runtime.session().archive_transaction(&context.tx_id()).unwrap();
    }
}
