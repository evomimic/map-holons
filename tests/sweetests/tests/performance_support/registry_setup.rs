//! Controlled Session products, prepared outside measured commands.
use holons_core::descriptors::Descriptor;
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime, RuntimeSession};
use std::sync::Arc;
use tracing::info;

pub async fn configure(runtime: Runtime) -> Runtime {
    let Ok(value) = std::env::var("MAP_COMMIT_PERF_REGISTRY") else { return runtime };
    let population: usize = value.parse().expect("registry population must be nonnegative");
    let reuse = std::env::var_os("MAP_COMMIT_PERF_GOVERNOR_REUSE").is_some();
    assert!(!reuse || population > 0);
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
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let mut targets = Vec::new();
    if reuse {
        targets.push(
            context
                .lookup()
                .get_saved_holon_by_key(&"VisualizerUsage.HolonType".into())
                .unwrap()
                .holon_id(),
        );
    }
    for index in targets.len()..population {
        super::creation_samples::stage_descriptor(&context, &format!("registry-filler-{index}"));
    }
    if population > usize::from(reuse) {
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
            panic!("commit expected")
        };
        assert_eq!(
            response.property_value("CommitRequestStatus").unwrap(),
            Some("Complete".to_base_value())
        );
        targets.extend(
            response
                .related_holons("SavedHolons")
                .unwrap()
                .read()
                .unwrap()
                .get_members()
                .iter()
                .map(|reference| reference.holon_id().unwrap()),
        );
    }
    runtime.session().archive_transaction(&tx_id).unwrap();
    // A new Session starts empty while preserving the already bootstrapped saved graph.
    drop(context);
    let runtime = fresh_session(runtime);
    let registry = runtime.session().descriptor_registry();
    let owner = registry.context().unwrap();
    for id in &targets {
        let target = HolonReference::smart_from_id(owner.space_read_handle(), id.clone());
        let descriptor = owner.construct_effective_holon_descriptor(target).unwrap();
        assert_eq!(descriptor.described_holon_type().unwrap().holon().holon_id().unwrap(), *id);
        registry.publish(id.clone(), &descriptor).unwrap();
        assert!(registry.get(id).unwrap().is_some());
    }
    info!(
        "[commit-workload-registry] requested={} published={} governor_reuse={}",
        population,
        targets.len(),
        reuse
    );
    assert_eq!(targets.len(), population);
    runtime
}

/// Release the previous Session before attaching its replacement to the retained space proxy.
fn fresh_session(runtime: Runtime) -> Runtime {
    let space = runtime.session().space_manager().clone();
    drop(runtime);
    Runtime::new(Arc::new(RuntimeSession::new(space, None)))
}

#[test]
fn registry_setup_replaces_the_session_without_two_active_owners() {
    use holons_core::core_shared_objects::{
        space_manager::HolonSpaceManager, ServiceRoutingPolicy,
    };
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(holons_client::ClientHolonService::development_default()),
        None,
        ServiceRoutingPolicy::Combined,
    ));
    let runtime = Runtime::new(Arc::new(RuntimeSession::new(space.clone(), None)));
    let previous = Arc::downgrade(runtime.session().descriptor_registry());
    let replacement = fresh_session(runtime);
    assert!(previous.upgrade().is_none(), "the previous registry owner must be released");
    assert!(Arc::ptr_eq(replacement.session().space_manager(), &space));
    assert!(replacement.session().descriptor_registry().is_empty().unwrap());
    assert!(Arc::ptr_eq(
        &space.descriptor_registry().unwrap(),
        replacement.session().descriptor_registry()
    ));
}
