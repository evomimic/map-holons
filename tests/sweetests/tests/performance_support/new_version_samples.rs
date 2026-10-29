//! Successive NewVersion Commits of one bootstrap-loaded instance whose key never changes.
//! Selected by `measure-commit-performance.py --mode new-version` to compare version-producing
//! Commits across captures. The unchanged key keeps key-uniqueness evidence constant across
//! samples; current assessment performs no anchor-lineage inventory scan.
use holons_core::{core_shared_objects::transactions::TxId, RelationshipReadHint};
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::{sync::Arc, time::Instant};
use tracing::info;

const SUBJECT_KEY: &str = "DefaultPropertyMapVisualizer.PropertySlot";
const CHANGED_PROPERTY: &str = "VisualizerSlotDescription";

/// Setup publishes the first successor outside the measured interval, so every
/// measured update starts from a non-root saved source.
pub async fn run(runtime: &Runtime, samples: usize) {
    let (tx_id, context) = begin(runtime).await;
    let root = context.lookup().get_saved_holon_by_key(&MapString::from(SUBJECT_KEY)).unwrap();
    let root_id = root.holon_id();
    let mut head = publish_version(runtime, &context, root_id.clone(), "setup", None).await;
    runtime.session().archive_transaction(&tx_id).unwrap();
    verify(runtime, &head, &root_id, "setup").await;

    for sample in 0..samples {
        let condition = if sample == 0 { "first" } else { "repeated" };
        let description = format!("sample {sample}");
        let (tx_id, context) = begin(runtime).await;
        let saved = publish_version(
            runtime,
            &context,
            head.clone(),
            &description,
            Some((sample, condition)),
        )
        .await;
        runtime.session().archive_transaction(&tx_id).unwrap();
        // Fresh persisted checks are outside the measured command interval.
        verify(runtime, &saved, &head, &description).await;
        head = saved;
    }
}

async fn begin(runtime: &Runtime) -> (TxId, Arc<TransactionContext>) {
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
    (tx_id, context)
}

/// Stages a property-only successor of `source`, commits it, and returns the new version.
/// Only sampled commits emit baseline markers; setup commits are ignored by the summarizer.
async fn publish_version(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    source: HolonId,
    description: &str,
    sample: Option<(usize, &str)>,
) -> HolonId {
    let mut staged = context.mutation().stage_new_version_from_id(source.clone()).unwrap();
    staged.with_property_value(CHANGED_PROPERTY, description).unwrap();
    if let Some((sample, condition)) = sample {
        info!(
            "[commit-baseline] sample_start={} condition={} candidates=1 mode=new-version",
            sample, condition
        );
    }
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
    assert_eq!(response.property_value("CommitsAttempted").unwrap(), Some(1_i64.to_base_value()));
    assert_eq!(
        response.property_value("ValidationViolationCount").unwrap(),
        Some(0_i64.to_base_value())
    );
    let saved =
        response.related_holons("SavedHolons").unwrap().read().unwrap().get_members().clone();
    assert_eq!(saved.len(), 1);
    if let Some((sample, condition)) = sample {
        info!(
            "[commit-baseline] sample_end={} condition={} command_us={}",
            sample, condition, command_us
        );
    }
    let saved_id = saved[0].holon_id().unwrap();
    assert_ne!(saved_id, source, "a property change must publish a new version");
    saved_id
}

async fn verify(runtime: &Runtime, saved_id: &HolonId, source: &HolonId, description: &str) {
    let (tx_id, context) = begin(runtime).await;
    let saved: HolonReference =
        SmartReference::new_from_id(context.space_read_handle(), saved_id.clone()).into();
    assert_eq!(saved.property_value(CHANGED_PROPERTY).unwrap(), Some(description.to_base_value()));
    let predecessors = saved
        .related_holons_with_hint("Predecessor", RelationshipReadHint::RequireFresh)
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .iter()
        .map(|member| member.holon_id().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(predecessors, vec![source.clone()], "the exact saved source is the predecessor");
    runtime.session().archive_transaction(&tx_id).unwrap();
}
