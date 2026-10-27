//! Explicit construction capture through the production guest Dance and normal transport.
//! Setup and subsequent graph inspection are outside the measured operation.
use holons_boundary::{HolonReferenceWire, SerializableHolonPool, SessionStateWire};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_prelude::prelude::*;
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use serde_json::json;
use std::{sync::Arc, time::Instant};

const PRODUCTS: [&str; 8] = [
    "EffectiveInstanceProperties",
    "EffectiveInstanceRelationships",
    "NavigableRelationships",
    "EffectiveAffordsDance",
    "EffectiveAffordsCommand",
    "EffectiveValidationBindings",
    "EffectiveConstraints",
    "EffectiveInstanceKeyRule",
];

async fn begin(runtime: &Runtime) -> Arc<TransactionContext> {
    let MapResult::TransactionCreated { tx_id } = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("expected transaction")
    };
    runtime.session().get_transaction(&tx_id).unwrap()
}

async fn dispose(runtime: &Runtime, context: Arc<TransactionContext>) {
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context,
                action: TransactionAction::Dispose,
            }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap();
}

fn members(reference: &HolonReference, name: &str) -> Vec<HolonReference> {
    reference.related_holons(name).unwrap().read().unwrap().get_members().clone()
}

/// JSON is a reproducible size proxy for normal session state, not the actual zome encoding.
fn session_json_bytes(context: &Arc<TransactionContext>) -> usize {
    let state = SessionStateWire::new(
        SerializableHolonPool::from(context.export_transient_holons().unwrap()),
        SerializableHolonPool::from(context.export_staged_holons().unwrap()),
        context.get_space_holon().unwrap().as_ref().map(HolonReferenceWire::from),
        Some(context.tx_id()),
    );
    serde_json::to_vec(&state).unwrap().len()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit performance capture; requires MAP_EFFECTIVE_DESCRIPTOR_PERF_OUTPUT"]
async fn measure_effective_descriptor_construction() {
    let output =
        std::env::var("MAP_EFFECTIVE_DESCRIPTOR_PERF_OUTPUT").expect("output path required");
    let samples: usize = std::env::var("MAP_EFFECTIVE_DESCRIPTOR_PERF_SAMPLES")
        .unwrap_or_else(|_| "3".into())
        .parse()
        .unwrap();
    assert!((2..=10).contains(&samples));
    let include_contributions =
        std::env::var("MAP_EFFECTIVE_DESCRIPTOR_PERF_INCLUDE_CONTRIBUTIONS").is_ok();
    let setup_started = Instant::now();
    let (runtime, initial_tx) = init_test_runtime(&mut DancesTestCase::default()).await;
    let initial = runtime.session().get_transaction(&initial_tx).unwrap();
    let mut targets = Vec::new();
    for key in ["HolonType.TypeDescriptor", "HolonSpace.HolonType", "EffectiveDescriptor.HolonType"]
    {
        let reference: HolonReference =
            initial.lookup().get_saved_holon_by_key(&MapString(key.into())).unwrap().into();
        targets.push((key, reference.holon_id().unwrap()));
    }
    dispose(&runtime, initial).await;
    let setup_us = setup_started.elapsed().as_micros() as u64;
    eprintln!(
        "[effective-descriptor-host] setup_us={setup_us}; setup excluded from construction samples"
    );
    let mut rows = Vec::new();
    for (key, id) in targets {
        let retained = begin(&runtime).await;
        let mut previous_retained = None;
        for sample in 0..samples {
            // Alternate order to avoid always granting one mode the warmer storage cache.
            for slot in 0..2 {
                let fresh = (slot + sample) % 2 == 0;
                let context = if fresh { begin(&runtime).await } else { retained.clone() };
                let mode = if fresh { "fresh_transaction" } else { "retained_transaction" };
                let target = HolonReference::smart_from_id(context.space_read_handle(), id.clone());
                let before_count = context.lookup().transient_count().unwrap();
                let before_bytes = session_json_bytes(&context);
                eprintln!(
                    "[effective-descriptor-host] start target={key} mode={mode} sample={sample}"
                );
                let started = Instant::now();
                let result = runtime
                    .execute_command(
                        MapCommand::Transaction(TransactionCommand {
                            context: context.clone(),
                            action: TransactionAction::ConstructEffectiveDescriptor {
                                target,
                                include_contributions,
                            },
                        }),
                        ExecutionPolicy::default(),
                    )
                    .await
                    .unwrap();
                let construction_us = started.elapsed().as_micros() as u64;
                let MapResult::Reference(reference) = result else { panic!("expected reference") };
                assert!(reference.is_transient());
                assert!(reference.key().unwrap().is_none());
                let product_counts = PRODUCTS
                    .into_iter()
                    .map(|name| (name, members(&reference, name).len()))
                    .collect::<std::collections::BTreeMap<_, _>>();
                assert_eq!(product_counts["EffectiveInstanceKeyRule"], 1);
                assert_eq!(members(&reference, "DescribedHolonType").len(), 1);
                let contributions = members(&reference, "EffectiveContributions").len();
                if include_contributions {
                    assert!(contributions > 0);
                } else {
                    assert_eq!(contributions, 0);
                }
                let after_count = context.lookup().transient_count().unwrap();
                let after_bytes = session_json_bytes(&context);
                let graph_nodes = 1 + contributions;
                // Result and records plus ordinary request/invocation/response allocations.
                assert_eq!(after_count - before_count, graph_nodes as i64 + 3);
                if !fresh {
                    if let Some(previous) = previous_retained {
                        assert_ne!(reference.full_reference_id_string(), previous);
                    }
                    previous_retained = Some(reference.full_reference_id_string());
                }
                rows.push(json!({"target": key, "mode": mode, "sample": sample,
                    "host_construction_us": construction_us, "transients_before": before_count,
                    "transients_after": after_count, "graph_nodes": graph_nodes,
                    "contributions": contributions, "product_counts": product_counts,
                    "session_json_bytes_before": before_bytes, "session_json_bytes_after": after_bytes}));
                eprintln!("[effective-descriptor-host] done target={key} mode={mode} sample={sample} host_us={construction_us} graph_nodes={graph_nodes}");
                if fresh {
                    dispose(&runtime, context).await;
                }
            }
        }
        dispose(&runtime, retained).await;
    }
    let capture = json!({"include_contributions": include_contributions, "platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH}, "setup_us": setup_us, "samples_per_target_and_mode": samples,
        "notes": ["Setup and graph inspection excluded from host construction interval.",
            "Native Runtime command path; TypeScript/Tauri IPC is not included.",
            "Fresh transaction is not cold storage: bootstrap and earlier calls warm shared caches.",
            "Guest request-local caches start afresh for each invocation; repeated calls can warm conductor/storage caches.",
            "Retained mode transports prior transient results. No hidden pool cleanup is performed.",
            "Session JSON bytes are a size proxy, not actual encoded zome payload bytes.",
            "The opt-in descriptor-performance-profiling guest build emits core and envelope phase times at WARN level."], "rows": rows});
    std::fs::write(output, serde_json::to_vec_pretty(&capture).unwrap()).unwrap();
}
