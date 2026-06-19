use std::any::Any;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId};
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::core_shared_objects::ServiceRoutingPolicy;
use holons_core::dances::{build_dance_v2_invocation, DanceInvocation};
use holons_core::reference_layer::{
    HolonReference, HolonServiceApi, StagedReference, TransientReference, WritableHolon,
};

use client_shared_types::base_receptor::{BaseReceptor, ReceptorType};
use holons_client::LocalRecoveryReceptor;
use recovery_receptor::{RecoveryStore, TransactionRecoveryStore};

use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};

use crate::{ExecutionPolicy, Runtime, RuntimeSession};

// ── Test double ─────────────────────────────────────────────────────

/// Fail-fast test double: holon-service methods are intentionally out of scope
/// for handler tests and should never be invoked here.
#[derive(Debug)]
struct TestHolonService;

fn unreachable_in_handler_tests<T>() -> Result<T, HolonError> {
    Err(HolonError::NotImplemented("TestHolonService".to_string()))
}

impl HolonServiceApi for TestHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        _staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        let mut response =
            context.mutation().new_holon(Some(MapString::from("commit-response")))?;
        response.with_property_value("TypeName", "CommitResponseType")?;
        response.with_property_value("IsAbstractType", false)?;
        response.with_property_value("InstanceTypeKind", "Holon")?;
        response.with_property_value("CommitRequestStatus", "Complete")?;
        Ok(response)
    }

    fn delete_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _local_id: &LocalId,
    ) -> Result<(), HolonError> {
        Ok(())
    }

    fn fetch_all_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
    ) -> Result<holons_core::core_shared_objects::RelationshipMap, HolonError> {
        unreachable_in_handler_tests()
    }

    fn fetch_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _id: &HolonId,
    ) -> Result<holons_core::core_shared_objects::Holon, HolonError> {
        unreachable_in_handler_tests()
    }

    fn fetch_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
        _relationship_name: &core_types::RelationshipName,
    ) -> Result<holons_core::core_shared_objects::HolonCollection, HolonError> {
        unreachable_in_handler_tests()
    }

    fn get_all_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<holons_core::core_shared_objects::HolonCollection, HolonError> {
        unreachable_in_handler_tests()
    }

    fn load_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        unreachable_in_handler_tests()
    }
}

fn build_test_space_manager() -> Arc<HolonSpaceManager> {
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
    Arc::new(HolonSpaceManager::new_with_managers(
        None,
        holon_service,
        None,
        ServiceRoutingPolicy::BlockExternal,
    ))
}

fn build_test_runtime() -> Runtime {
    let space_manager = build_test_space_manager();
    let session = Arc::new(RuntimeSession::new(space_manager, None));
    Runtime::new(session)
}

/// Helper: begin a transaction and return the tx_id.
async fn begin_tx(runtime: &Runtime) -> TxId {
    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");
    match result {
        MapResult::TransactionCreated { tx_id } => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    }
}

/// Helper: build a TransactionCommand for a given tx_id.
fn tx_cmd(runtime: &Runtime, tx_id: &TxId, action: TransactionAction) -> MapCommand {
    let context = runtime.session().get_transaction(tx_id).expect("tx should exist");
    MapCommand::Transaction(TransactionCommand { context, action })
}

// ── Handler tests ───────────────────────────────────────────────────

#[tokio::test]
async fn begin_transaction_returns_valid_tx_id() {
    let runtime = build_test_runtime();

    let result = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");

    match result {
        MapResult::TransactionCreated { tx_id } => {
            assert!(tx_id.value() > 0, "tx_id should be positive");
        }
        other => panic!("expected TransactionCreated, got {:?}", other),
    }
}

#[tokio::test]
async fn begin_transaction_ids_are_unique() {
    let runtime = build_test_runtime();

    let mut tx_ids = Vec::new();
    for _ in 0..3 {
        let result = runtime
            .execute_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                ExecutionPolicy::default(),
            )
            .await
            .expect("execute_command should succeed");
        match result {
            MapResult::TransactionCreated { tx_id } => tx_ids.push(tx_id),
            other => panic!("expected TransactionCreated, got {:?}", other),
        }
    }

    for i in 0..tx_ids.len() {
        for j in (i + 1)..tx_ids.len() {
            assert_ne!(tx_ids[i], tx_ids[j], "tx_ids should be unique");
        }
    }
}

#[tokio::test]
async fn invalid_tx_id_returns_error() {
    let runtime = build_test_runtime();

    let bad_tx_id: TxId = serde_json::from_value(serde_json::json!(999)).unwrap();
    let result = runtime.session().get_transaction(&bad_tx_id);

    match result {
        Err(HolonError::InvalidParameter(msg)) => {
            assert!(msg.contains("999"), "error should mention the tx_id");
        }
        other => panic!("expected InvalidParameter error, got {:?}", other),
    }
}

// ── Transaction lookup handler ─────────────────────────────────────

#[tokio::test]
async fn staged_count_returns_zero_for_new_tx() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::GetStagedCount),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");

    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(0))) => {}
        other => panic!("expected Value(IntegerValue(0)), got {:?}", other),
    }
}

#[tokio::test]
async fn transient_count_returns_zero_for_new_tx() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::GetTransientCount),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");

    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(0))) => {}
        other => panic!("expected Value(IntegerValue(0)), got {:?}", other),
    }
}

// ── Transaction mutation handler ───────────────────────────────────

#[tokio::test]
async fn new_holon_then_transient_count() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    // NewHolon creates a transient
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("test-key")) },
    );
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("execute_command should succeed");
    match &result {
        MapResult::Reference(HolonReference::Transient(_)) => {}
        other => panic!("expected Transient reference, got {:?}", other),
    }

    // Transient count should be 1
    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::GetTransientCount),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");
    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(1))) => {}
        other => panic!("expected Value(IntegerValue(1)), got {:?}", other),
    }
}

#[tokio::test]
async fn new_holon_stage_then_staged_count() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    // NewHolon → get transient ref
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("stage-test")) },
    );
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("execute_command should succeed");
    let transient_ref = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("expected Transient reference, got {:?}", other),
    };

    // StageNewHolon using the transient ref directly
    let cmd = tx_cmd(&runtime, &tx_id, TransactionAction::StageNewHolon { source: transient_ref });
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("execute_command should succeed");
    match &result {
        MapResult::Reference(HolonReference::Staged(_)) => {}
        other => panic!("expected Staged reference, got {:?}", other),
    }

    // GetStagedCount should be 1
    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::GetStagedCount),
            ExecutionPolicy::default(),
        )
        .await
        .expect("execute_command should succeed");
    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(1))) => {}
        other => panic!("expected Value(IntegerValue(1)), got {:?}", other),
    }
}

// ── Recovery-backed runtime helpers ────────────────────────────────

fn build_test_recovery_receptor() -> Arc<LocalRecoveryReceptor> {
    let store = Arc::new(
        TransactionRecoveryStore::new(Path::new(":memory:"))
            .expect("in-memory recovery store should init"),
    );
    let base = BaseReceptor {
        receptor_id: "test-recovery".to_string(),
        receptor_type: ReceptorType::LocalRecovery,
        //client_handler: Some(store as Arc<dyn Any + Send + Sync>),
        properties: HashMap::new(),
    };
    Arc::new(LocalRecoveryReceptor::from_base(base, store)) //.expect("receptor should create")
}

fn build_test_runtime_with_recovery() -> Runtime {
    let space_manager = build_test_space_manager();
    let recovery = build_test_recovery_receptor();
    let session = Arc::new(RuntimeSession::new(space_manager, Some(recovery)));
    Runtime::new(session)
}

/// Create a transient holon, stage it, and close an ExperienceUnit (`snapshot_after=true`).
async fn stage_and_close(runtime: &Runtime, tx_id: &TxId, key: &str) {
    let cmd =
        tx_cmd(runtime, tx_id, TransactionAction::NewHolon { key: Some(MapString::from(key)) });
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("NewHolon should succeed");
    let transient_ref = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("stage_and_close: expected Transient reference, got {:?}", other),
    };

    let cmd = tx_cmd(runtime, tx_id, TransactionAction::StageNewHolon { source: transient_ref });
    runtime
        .execute_command(cmd, ExecutionPolicy { snapshot_after: true, ..Default::default() })
        .await
        .expect("StageNewHolon should succeed");
}

async fn staged_count(runtime: &Runtime, tx_id: &TxId) -> i64 {
    let result = runtime
        .execute_command(
            tx_cmd(runtime, tx_id, TransactionAction::GetStagedCount),
            ExecutionPolicy::default(),
        )
        .await
        .expect("GetStagedCount should succeed");
    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(n))) => n,
        other => panic!("expected IntegerValue, got {:?}", other),
    }
}

async fn build_dance_v2_command(runtime: &Runtime, tx_id: &TxId) -> MapCommand {
    let context = runtime.session().get_transaction(tx_id).expect("tx should exist");

    let mut invocation_descriptor = context
        .mutation()
        .new_holon(Some(MapString::from("invocation-descriptor")))
        .expect("invocation descriptor");
    invocation_descriptor
        .with_property_value("TypeName", "DanceInvocation")
        .expect("invocation type");
    invocation_descriptor
        .with_property_value("IsAbstractType", false)
        .expect("invocation abstract");
    invocation_descriptor
        .with_property_value("InstanceTypeKind", "Holon")
        .expect("invocation kind");

    let mut request_type =
        context.mutation().new_holon(Some(MapString::from("request-type"))).expect("request type");
    request_type.with_property_value("TypeName", "SummarizeRequest").expect("request type name");
    request_type.with_property_value("IsAbstractType", false).expect("request abstract");
    request_type.with_property_value("InstanceTypeKind", "Holon").expect("request kind");

    let mut response_type = context
        .mutation()
        .new_holon(Some(MapString::from("response-type")))
        .expect("response type");
    response_type.with_property_value("TypeName", "DanceResponseType").expect("response type name");
    response_type.with_property_value("IsAbstractType", false).expect("response abstract");
    response_type.with_property_value("InstanceTypeKind", "Holon").expect("response kind");

    let mut implementation = context
        .mutation()
        .new_holon(Some(MapString::from("implementation")))
        .expect("implementation");
    implementation.with_property_value("TypeName", "HostSummarizeV1").expect("implementation type");
    implementation.with_property_value("IsAbstractType", false).expect("implementation abstract");
    implementation.with_property_value("InstanceTypeKind", "Holon").expect("implementation kind");

    let mut dance_descriptor =
        context.mutation().new_holon(Some(MapString::from("dance"))).expect("dance descriptor");
    dance_descriptor.with_property_value("TypeName", "Summarize").expect("dance type");
    dance_descriptor.with_property_value("IsAbstractType", false).expect("dance abstract");
    dance_descriptor.with_property_value("InstanceTypeKind", "Holon").expect("dance kind");
    dance_descriptor
        .add_related_holons("InputParameters", vec![request_type.clone().into()])
        .expect("input parameters edge");
    dance_descriptor
        .add_related_holons("Response", vec![response_type.into()])
        .expect("response edge");
    dance_descriptor
        .add_related_holons("ForDance", vec![implementation.into()])
        .expect("implementation edge");

    let mut request =
        context.mutation().new_holon(Some(MapString::from("request"))).expect("request holon");
    request.with_property_value("TypeName", "SummarizeRequest").expect("request type");
    request.with_property_value("IsAbstractType", false).expect("request abstract");
    request.with_property_value("InstanceTypeKind", "Holon").expect("request kind");
    request.with_descriptor(request_type.into()).expect("request described_by");
    let mut invocation = context
        .mutation()
        .new_holon(Some(MapString::from("invocation")))
        .expect("invocation holon");
    invocation.with_property_value("TypeName", "DanceInvocation").expect("invocation type");
    invocation.with_property_value("IsAbstractType", false).expect("invocation abstract");
    invocation.with_property_value("InstanceTypeKind", "Holon").expect("invocation kind");
    invocation.with_descriptor(invocation_descriptor.into()).expect("invocation described_by");
    invocation
        .add_related_holons("InvokesDance", vec![dance_descriptor.into()])
        .expect("invokes dance");
    invocation.add_related_holons("Request", vec![request.into()]).expect("request edge");

    MapCommand::Transaction(TransactionCommand {
        context,
        action: TransactionAction::DanceV2 {
            invocation: build_dance_v2_invocation(invocation.into()).expect("typed invocation"),
        },
    })
}

async fn build_delete_holon_dance_v2_command(runtime: &Runtime, tx_id: &TxId) -> MapCommand {
    let context = runtime.session().get_transaction(tx_id).expect("tx should exist");
    let local_id = LocalId(vec![9, 8, 7]);
    let invocation = DanceInvocation::build_delete_holon(&context, HolonId::Local(local_id))
        .expect("delete holon invocation");

    MapCommand::Transaction(TransactionCommand {
        context,
        action: TransactionAction::DanceV2 { invocation },
    })
}

async fn build_commit_dance_v2_command(runtime: &Runtime, tx_id: &TxId) -> MapCommand {
    let context = runtime.session().get_transaction(tx_id).expect("tx should exist");
    let invocation = DanceInvocation::build_commit(&context).expect("commit invocation");

    MapCommand::Transaction(TransactionCommand {
        context,
        action: TransactionAction::DanceV2 { invocation },
    })
}

#[tokio::test]
async fn dance_v2_returns_not_implemented_for_host_binding_without_runtime_adapter() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;
    let command = build_dance_v2_command(&runtime, &tx_id).await;

    let error = runtime
        .execute_command(command, ExecutionPolicy::default())
        .await
        .expect_err("execute_command should surface missing host implementation adapter");

    assert!(
        matches!(error, HolonError::NotImplemented(message) if message.contains("HostSummarizeV1"))
    );
}

#[tokio::test]
async fn dance_v2_delete_holon_returns_reference_result() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;
    let command = build_delete_holon_dance_v2_command(&runtime, &tx_id).await;

    let result = runtime
        .execute_command(command, ExecutionPolicy::default())
        .await
        .expect("delete holon v2 should succeed");

    assert!(matches!(result, MapResult::Reference(_)));
}

#[tokio::test]
async fn built_delete_holon_invocation_uses_request_and_not_target() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;
    let command = build_delete_holon_dance_v2_command(&runtime, &tx_id).await;

    let invocation = match command {
        MapCommand::Transaction(TransactionCommand {
            action: TransactionAction::DanceV2 { invocation },
            ..
        }) => invocation,
        other => panic!("expected DanceV2 transaction command, got {:?}", other),
    };

    let bound = invocation.bind().expect("bind delete holon invocation");
    assert!(bound.request().is_some(), "delete holon should carry request parameters");
    assert!(
        bound.affording_holon().is_none(),
        "delete holon should not resolve a target holon at ingress"
    );
}

#[tokio::test]
async fn dance_v2_commit_returns_reference_result() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;
    let command = build_commit_dance_v2_command(&runtime, &tx_id).await;

    let result = runtime
        .execute_command(command, ExecutionPolicy::default())
        .await
        .expect("commit v2 should succeed");

    assert!(matches!(result, MapResult::Reference(_)));
}

#[tokio::test]
async fn delete_holon_command_returns_none_after_internal_dance_execution() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let result = runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::DeleteHolon { local_id: LocalId(vec![9, 8, 7]) },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("delete holon command should succeed through internal dance execution");

    assert!(matches!(result, MapResult::None));
}

#[tokio::test]
async fn commit_command_returns_reference_after_internal_dance_execution() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::Commit),
            ExecutionPolicy::default(),
        )
        .await
        .expect("commit command should succeed through internal dance execution");

    assert!(matches!(result, MapResult::Reference(HolonReference::Transient(_))));
}

// ── Undo/redo handler tests ─────────────────────────────────────────

#[tokio::test]
async fn snapshot_after_creates_undo_unit() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // Two EUs so the first undo has a prior snapshot to restore
    stage_and_close(&runtime, &tx_id, "holon-a").await;
    stage_and_close(&runtime, &tx_id, "holon-b").await;

    assert_eq!(staged_count(&runtime, &tx_id).await, 2);

    // UndoLast succeeds — proves snapshot_after=true created an EU
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed after snapshot_after=true");
}

#[tokio::test]
async fn intermediate_command_no_undo_unit() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // NewHolon without snapshot_after — no EU is closed
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("no-unit")) },
    );
    runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("NewHolon should succeed");

    // UndoLast must fail — undo stack is empty
    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(result.is_err(), "UndoLast should fail when no ExperienceUnit has been closed");
}

#[tokio::test]
async fn undo_restores_prior_state() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "holon-a").await; // EU_1 — count=1
    stage_and_close(&runtime, &tx_id, "holon-b").await; // EU_2 — count=2

    assert_eq!(staged_count(&runtime, &tx_id).await, 2);

    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1, "undo should restore EU_1 state");
}

#[tokio::test]
async fn redo_after_undo() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "holon-a").await; // EU_1 — count=1
    stage_and_close(&runtime, &tx_id, "holon-b").await; // EU_2 — count=2

    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::RedoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("RedoLast should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 2, "redo should restore EU_2 state");
}

#[tokio::test]
async fn new_command_invalidates_redo() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "holon-a").await; // EU_1
    stage_and_close(&runtime, &tx_id, "holon-b").await; // EU_2

    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    // New EU after undo clears the redo stack
    stage_and_close(&runtime, &tx_id, "holon-c").await; // EU_3 — redo cleared

    let redo_result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::RedoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(redo_result.is_err(), "RedoLast should fail after new EU was created post-undo");
}

#[tokio::test]
async fn disable_undo_prevents_future_units() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // Stage a holon and close the EU with disable_undo=true — permanently disables checkpointing
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("disable-key")) },
    );
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("NewHolon should succeed");
    let transient_ref = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("expected Transient reference, got {:?}", other),
    };
    let cmd = tx_cmd(&runtime, &tx_id, TransactionAction::StageNewHolon { source: transient_ref });
    runtime
        .execute_command(
            cmd,
            ExecutionPolicy { snapshot_after: true, disable_undo: true, ..Default::default() },
        )
        .await
        .expect("StageNewHolon with disable_undo should succeed");

    // Even with snapshot_after=true, no EU should be created now
    stage_and_close(&runtime, &tx_id, "after-disable").await;

    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(
        result.is_err(),
        "UndoLast should fail after disable_undo permanently disabled checkpointing"
    );
}

// ── Marker navigation tests ─────────────────────────────────────────

/// Stage a holon and close an EU with an explicit marker_id.
async fn stage_and_close_marked(runtime: &Runtime, tx_id: &TxId, key: &str, marker: &str) {
    let cmd =
        tx_cmd(runtime, tx_id, TransactionAction::NewHolon { key: Some(MapString::from(key)) });
    let result = runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("NewHolon should succeed");
    let transient_ref = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("stage_and_close_marked: expected Transient, got {:?}", other),
    };

    let cmd = tx_cmd(runtime, tx_id, TransactionAction::StageNewHolon { source: transient_ref });
    runtime
        .execute_command(
            cmd,
            ExecutionPolicy {
                snapshot_after: true,
                marker_id: Some(marker.to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("StageNewHolon with marker should succeed");
}

#[tokio::test]
async fn undo_to_marker_jumps_to_marked_unit() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // EU_0 (unmarked, count=1) — establishes a snapshot before the marker
    stage_and_close(&runtime, &tx_id, "holon-0").await;
    // EU_1 marked "step-1" (count=2)
    stage_and_close_marked(&runtime, &tx_id, "holon-1", "step-1").await;
    // EU_2 (count=3) and EU_3 (count=4) above the marker
    stage_and_close(&runtime, &tx_id, "holon-2").await;
    stage_and_close(&runtime, &tx_id, "holon-3").await;

    assert_eq!(staged_count(&runtime, &tx_id).await, 4);

    // UndoToMarker should pop EU_3, EU_2, EU_1 and restore to EU_0 state
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "step-1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should succeed");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        1,
        "UndoToMarker should restore state to just before the marked EU"
    );
}

#[tokio::test]
async fn redo_to_marker_jumps_forward() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "holon-0").await; // EU_0, count=1
    stage_and_close_marked(&runtime, &tx_id, "holon-1", "step-1").await; // EU_1, count=2
    stage_and_close(&runtime, &tx_id, "holon-2").await; // EU_2, count=3
    stage_and_close(&runtime, &tx_id, "holon-3").await; // EU_3, count=4

    // Undo to the marker — now count=1, redo stack has [EU_1, EU_2, EU_3]
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "step-1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    // RedoToMarker("step-1") should restore EU_1's state — count=2
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::RedoToMarker { marker_id: "step-1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("RedoToMarker should succeed");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        2,
        "RedoToMarker should restore state to after the marked EU was applied"
    );
}

#[tokio::test]
async fn undo_to_marker_fails_if_marker_not_reachable() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "holon-a").await;
    stage_and_close(&runtime, &tx_id, "holon-b").await;

    // "ghost" is not on the undo stack — should return InvalidParameter
    let result = runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "ghost".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(result.is_err(), "UndoToMarker should fail for a nonexistent marker");

    // Move EU to redo, then try to UndoToMarker it — no longer on undo stack
    stage_and_close_marked(&runtime, &tx_id, "holon-c", "now-in-redo").await;
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    let result2 = runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "now-in-redo".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(
        result2.is_err(),
        "UndoToMarker should fail when the marker is in the redo stack, not undo"
    );
}

#[tokio::test]
async fn marker_binding_at_close() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // EU_0 establishes a prior snapshot so UndoToMarker has something to restore
    stage_and_close(&runtime, &tx_id, "base").await; // count=1
                                                     // EU_1 bound to marker "m1"
    stage_and_close_marked(&runtime, &tx_id, "marked", "m1").await; // count=2
                                                                    // EU_2 above the marker
    stage_and_close(&runtime, &tx_id, "above").await; // count=3

    // If the marker was stored, UndoToMarker("m1") finds EU_1 and succeeds
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "m1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should find the marker — proving it was bound at EU close");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1, "restored to the EU before the marker");
}

// ── disable_undo hardening tests ────────────────────────────────────

#[tokio::test]
async fn disable_undo_midstream_prior_eus_remain_undoable() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // Two EUs created normally before disable fires.
    stage_and_close(&runtime, &tx_id, "before-0").await; // EU_0, count=1
    stage_and_close(&runtime, &tx_id, "before-1").await; // EU_1, count=2
    assert_eq!(staged_count(&runtime, &tx_id).await, 2);

    // Intermediate command with disable_undo=true — permanently disables checkpointing
    // but does not touch the existing undo stack.
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("after-disable")) },
    );
    runtime
        .execute_command(cmd, ExecutionPolicy { disable_undo: true, ..Default::default() })
        .await
        .expect("NewHolon with disable_undo should succeed");

    // EU_1 is still on the undo stack — UndoLast must succeed.
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should still work on EUs created before disable_undo");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        1,
        "EU_1 should have been undone; count drops to 1"
    );
}

#[tokio::test]
async fn disable_undo_without_snapshot_after_still_sets_flag() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // Fire any command with disable_undo=true and snapshot_after=false — the flag
    // is written to the DB regardless of snapshot_after.
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("flag-setter")) },
    );
    runtime
        .execute_command(
            cmd,
            ExecutionPolicy { disable_undo: true, snapshot_after: false, ..Default::default() },
        )
        .await
        .expect("NewHolon with disable_undo=true should succeed");

    // Close an EU — snapshot_after=true — but the flag is already set so no EU is created.
    stage_and_close(&runtime, &tx_id, "after-flag").await;

    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(
        result.is_err(),
        "UndoLast should fail: disable_undo set the flag before any EU was created"
    );
}

#[tokio::test]
async fn disable_undo_after_markers_marker_still_navigable() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    // EU_0 (no marker, count=1) provides a snapshot to restore to — same pattern
    // used by every other marker test (undo_to_marker restores to the EU *below* the marker).
    stage_and_close(&runtime, &tx_id, "base").await;
    // EU_1 with marker "m1" (count=2), EU_2 above (count=3).
    stage_and_close_marked(&runtime, &tx_id, "marked", "m1").await;
    stage_and_close(&runtime, &tx_id, "above").await;

    // Disable checkpointing — existing EUs stay on the undo stack.
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("disabler")) },
    );
    runtime
        .execute_command(cmd, ExecutionPolicy { disable_undo: true, ..Default::default() })
        .await
        .expect("NewHolon with disable_undo should succeed");

    // undo_to_marker("m1") pops EU_2 and EU_1; restores to EU_0's snapshot (count=1).
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "m1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should still navigate to markers created before disable_undo");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        1,
        "restored to EU_0's state — count drops from 3 to 1"
    );
}

// ── Redo invalidation by forward mutations ──────────────────────────

#[tokio::test]
async fn intermediate_mutation_after_undo_invalidates_redo() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "eu-0").await; // EU_0, count=1
    stage_and_close(&runtime, &tx_id, "eu-1").await; // EU_1, count=2

    // Undo EU_1 — it moves to the redo stack.
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    // Intermediate forward mutation (no snapshot_after) — must invalidate redo.
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("forward")) },
    );
    runtime
        .execute_command(cmd, ExecutionPolicy::default())
        .await
        .expect("intermediate NewHolon should succeed");

    // RedoLast must now fail — the redo timeline was cleared.
    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::RedoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(result.is_err(), "RedoLast should fail: intermediate mutation cleared the redo stack");
}

#[tokio::test]
async fn disable_undo_mutation_after_undo_invalidates_redo() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "eu-0").await; // EU_0, count=1
    stage_and_close(&runtime, &tx_id, "eu-1").await; // EU_1, count=2

    // Undo EU_1 — it moves to the redo stack.
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    // disable_undo mutation — must also invalidate redo.
    let cmd = tx_cmd(
        &runtime,
        &tx_id,
        TransactionAction::NewHolon { key: Some(MapString::from("forward")) },
    );
    runtime
        .execute_command(cmd, ExecutionPolicy { disable_undo: true, ..Default::default() })
        .await
        .expect("disable_undo NewHolon should succeed");

    // RedoLast must now fail — the redo timeline was cleared.
    let result = runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::RedoLast),
            ExecutionPolicy::default(),
        )
        .await;
    assert!(result.is_err(), "RedoLast should fail: disable_undo mutation cleared the redo stack");
}

#[tokio::test]
async fn undo_first_eu_restores_to_baseline() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "eu-0").await; // EU_0, count=1
    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    // Undo the only EU — store returns None (no prior snapshot), must restore to baseline.
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoLast should succeed even when undoing the first EU");

    assert_eq!(staged_count(&runtime, &tx_id).await, 0, "baseline should have no staged holons");

    // EU_0 is now on the redo stack — RedoLast must restore it.
    runtime
        .execute_command(
            tx_cmd(&runtime, &tx_id, TransactionAction::RedoLast),
            ExecutionPolicy::default(),
        )
        .await
        .expect("RedoLast should restore EU_0");

    assert_eq!(staged_count(&runtime, &tx_id).await, 1, "redo should restore EU_0 state");
}

#[tokio::test]
async fn undo_to_marker_at_first_eu_restores_to_baseline() {
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close_marked(&runtime, &tx_id, "eu-0", "m0").await; // EU_0 with marker, count=1
    assert_eq!(staged_count(&runtime, &tx_id).await, 1);

    // UndoToMarker targeting the first (only) EU — store returns None (no prior snapshot).
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "m0".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should succeed when marker is the first EU");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        0,
        "UndoToMarker at first EU should restore to baseline"
    );
}

#[tokio::test]
async fn undo_to_marker_after_redo_to_marker_uses_correct_stack_order() {
    // Regression for redo_to_marker stack_pos inversion bug.
    // After redo_to_marker moves multiple EUs back to undo, the marker must be
    // at the top of the undo stack so undo_to_marker("m1") pops only that one EU.
    let runtime = build_test_runtime_with_recovery();
    let tx_id = begin_tx(&runtime).await;

    stage_and_close(&runtime, &tx_id, "eu-0").await; // EU_0, count=1
    stage_and_close_marked(&runtime, &tx_id, "eu-1", "m1").await; // EU_1 (marked), count=2
    stage_and_close(&runtime, &tx_id, "eu-2").await; // EU_2, count=3

    // Undo all three — redo stack now holds [EU_2, EU_1, EU_0] (EU_2 most recently undone)
    for _ in 0..3 {
        runtime
            .execute_command(
                tx_cmd(&runtime, &tx_id, TransactionAction::UndoLast),
                ExecutionPolicy::default(),
            )
            .await
            .expect("UndoLast should succeed");
    }
    assert_eq!(staged_count(&runtime, &tx_id).await, 0);

    // redo_to_marker("m1") moves EU_2 (newest on redo) and EU_1 (marker) back to undo.
    // After this: undo=[EU_0, EU_1], redo=[EU_2], count=2.
    // EU_1 must be at the TOP of the undo stack (highest stack_pos).
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::RedoToMarker { marker_id: "m1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("RedoToMarker should succeed");

    assert_eq!(staged_count(&runtime, &tx_id).await, 2);

    // undo_to_marker("m1") must pop only EU_1 (the top of the undo stack) → count=1.
    // With the stack_pos inversion bug, load_eu_stack would put EU_0 at the top
    // and drain both EU_0 and EU_1, giving count=0 instead.
    runtime
        .execute_command(
            tx_cmd(
                &runtime,
                &tx_id,
                TransactionAction::UndoToMarker { marker_id: "m1".to_string() },
            ),
            ExecutionPolicy::default(),
        )
        .await
        .expect("UndoToMarker should find m1 at the top of the undo stack");

    assert_eq!(
        staged_count(&runtime, &tx_id).await,
        1,
        "undo_to_marker after redo_to_marker must pop only the marker EU, not the ones below it"
    );
}
