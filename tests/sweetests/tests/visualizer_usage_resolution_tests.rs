//! Committed affordance and Usage resolution across the ordinary client/guest boundary.
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceInvocation, DanceResponseReference, VisualizerUsageResolution};
use holons_core::{HolonReference, ReadableHolon, WritableHolon};
use holons_test::{init_test_runtime, DancesTestCase};
use map_commands_contract::{MapCommand, MapResult, TransactionAction, TransactionCommand};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use std::sync::Arc;

fn saved(context: &Arc<TransactionContext>, key: &str) -> HolonReference {
    context.lookup().get_saved_holon_by_key(&key.into()).unwrap().into()
}
fn members(source: &HolonReference, name: &str) -> Vec<HolonReference> {
    source.related_holons(name).unwrap().read().unwrap().get_members().clone()
}
async fn new_context(runtime: &Runtime) -> Arc<TransactionContext> {
    let id = runtime.session().begin_transaction().await.unwrap();
    runtime.session().get_transaction(&id).unwrap()
}
async fn command(
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    action: TransactionAction,
) -> MapResult {
    runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand { context: context.clone(), action }),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn guest_usage_resolution_preserves_affordance_inheritance_scope_and_read_only_misses() {
    let (runtime, _) = init_test_runtime(&mut DancesTestCase::default()).await;
    let context = new_context(&runtime).await;
    let visualizer = saved(&context, "DefaultPropertyMapVisualizer.PropertyMapVisualizer");
    let actual = saved(&context, "HolonSpace.HolonType");
    let default_type = saved(&context, "HolonType.TypeDescriptor");
    let slot = saved(&context, "DefaultPropertyMapVisualizer.PropertySlot");
    let dance = saved(&context, "ResolveVisualizerUsage.DanceType");
    let visualizer_type = saved(&context, "Visualizer.HolonType");
    assert!(members(&visualizer_type, "AffordsDance").contains(&dance));
    assert_eq!(members(&dance, "DanceAffordedBy"), vec![visualizer_type]);

    let missing = context.resolve_visualizer_usage(&visualizer, &actual, &slot).unwrap();
    assert!(missing.usage.is_none());
    assert_eq!(missing.default_type, default_type);
    assert_eq!(missing.search_path.first(), Some(&actual));
    assert_eq!(context.lookup().staged_count().unwrap(), 0);

    // Commit a default configuration and resolve it from a fresh transaction.
    let mut usage = context.mutation().new_holon(Some("integration-shared-usage".into())).unwrap();
    usage.with_descriptor(saved(&context, "VisualizerUsage.HolonType")).unwrap();
    usage.add_related_holons("UsesVisualizer", vec![visualizer.clone()]).unwrap();
    usage.add_related_holons("ForSubjectType", vec![default_type]).unwrap();
    let staged = context.mutation().stage_new_holon(usage).unwrap();
    command(&runtime, &context, TransactionAction::Commit).await;
    assert!(!context.is_open());
    let expected = staged.holon_id().unwrap();

    let context = new_context(&runtime).await;
    let visualizer = saved(&context, "DefaultPropertyMapVisualizer.PropertyMapVisualizer");
    let actual = saved(&context, "HolonSpace.HolonType");
    let slot = saved(&context, "DefaultPropertyMapVisualizer.PropertySlot");
    let resolved = context.resolve_visualizer_usage(&visualizer, &actual, &slot).unwrap();
    assert_eq!(resolved.usage.unwrap().holon_id().unwrap(), expected);
    assert_eq!(resolved.usage_type, Some(saved(&context, "HolonType.TypeDescriptor")));
    assert_eq!(context.lookup().staged_count().unwrap(), 0);

    // Canonical invocation reaches the same guest-only service capability.
    let mut request = context.mutation().new_holon(Some("resolve-usage-request".into())).unwrap();
    request.with_descriptor(saved(&context, "ResolveVisualizerUsageRequest.HolonType")).unwrap();
    request.add_related_holons("UsageSubjectType", vec![actual]).unwrap();
    request.add_related_holons("UsageSlot", vec![slot]).unwrap();
    let mut invocation =
        context.mutation().new_holon(Some("resolve-usage-invocation".into())).unwrap();
    invocation.with_descriptor(saved(&context, "DanceInvocation.HolonType")).unwrap();
    invocation.with_property_value("DanceName", "ResolveVisualizerUsage").unwrap();
    invocation.add_related_holons("Request", vec![request.into()]).unwrap();
    invocation.add_related_holons("AffordingHolon", vec![visualizer]).unwrap();
    let MapResult::Reference(response) = command(
        &runtime,
        &context,
        TransactionAction::DanceV2 { invocation: DanceInvocation::new(invocation.into()).unwrap() },
    )
    .await
    else {
        panic!("Dance response")
    };
    let body = DanceResponseReference::new(response).unwrap().require_response_body().unwrap();
    let result = VisualizerUsageResolution::from_projection(&body).unwrap();
    assert_eq!(result.usage.unwrap().holon_id().unwrap(), expected);
    assert_eq!(context.lookup().staged_count().unwrap(), 0);
}
