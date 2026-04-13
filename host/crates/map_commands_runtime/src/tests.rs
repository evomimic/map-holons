use std::any::Any;
use std::sync::Arc;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::core_shared_objects::{
    Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
};
use holons_core::reference_layer::{
    HolonReference, HolonServiceApi, StagedReference, TransientReference,
};

use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};

use crate::{Runtime, RuntimeSession};

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
        _context: &Arc<TransactionContext>,
        _staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        unreachable_in_handler_tests()
    }

    fn delete_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _local_id: &LocalId,
    ) -> Result<(), HolonError> {
        unreachable_in_handler_tests()
    }

    fn fetch_all_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        unreachable_in_handler_tests()
    }

    fn fetch_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _id: &HolonId,
    ) -> Result<Holon, HolonError> {
        unreachable_in_handler_tests()
    }

    fn fetch_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_handler_tests()
    }

    fn get_all_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
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
    let session = Arc::new(RuntimeSession::new(space_manager,None));
    Runtime::new(session)
}

/// Helper: begin a transaction and return the tx_id.
async fn begin_tx(runtime: &Runtime) -> TxId {
    let result = runtime
        .execute_command(MapCommand::Space(SpaceCommand::BeginTransaction))
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
        .execute_command(MapCommand::Space(SpaceCommand::BeginTransaction))
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
            .execute_command(MapCommand::Space(SpaceCommand::BeginTransaction))
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
        .execute_command(tx_cmd(&runtime, &tx_id, TransactionAction::StagedCount))
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
        .execute_command(tx_cmd(&runtime, &tx_id, TransactionAction::TransientCount))
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
    let result = runtime.execute_command(cmd).await.expect("execute_command should succeed");
    match &result {
        MapResult::Reference(HolonReference::Transient(_)) => {}
        other => panic!("expected Transient reference, got {:?}", other),
    }

    // Transient count should be 1
    let result = runtime
        .execute_command(tx_cmd(&runtime, &tx_id, TransactionAction::TransientCount))
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
    let result = runtime.execute_command(cmd).await.expect("execute_command should succeed");
    let transient_ref = match result {
        MapResult::Reference(HolonReference::Transient(t)) => t,
        other => panic!("expected Transient reference, got {:?}", other),
    };

    // StageNewHolon using the transient ref directly
    let cmd = tx_cmd(&runtime, &tx_id, TransactionAction::StageNewHolon { source: transient_ref });
    let result = runtime.execute_command(cmd).await.expect("execute_command should succeed");
    match &result {
        MapResult::Reference(HolonReference::Staged(_)) => {}
        other => panic!("expected Staged reference, got {:?}", other),
    }

    // StagedCount should be 1
    let result = runtime
        .execute_command(tx_cmd(&runtime, &tx_id, TransactionAction::StagedCount))
        .await
        .expect("execute_command should succeed");
    match result {
        MapResult::Value(BaseValue::IntegerValue(MapInteger(1))) => {}
        other => panic!("expected Value(IntegerValue(1)), got {:?}", other),
    }
}
