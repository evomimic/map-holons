use std::any::Any;
use std::sync::Arc;

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyName, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{
    Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
};
use holons_core::reference_layer::{HolonServiceApi, StagedReference, TransientReference};

use holons_core::core_shared_objects::transactions::TxId;

use crate::dispatch::{Runtime, RuntimeSession};
use crate::domain::{
    CommandDescriptor, HolonAction, MutationClassification, ReadableHolonAction, SpaceCommand,
    TransactionAction, WritableHolonAction,
};
use crate::wire::*;

// ── Test double ─────────────────────────────────────────────────────

/// Fail-fast test double: holon-service methods are intentionally out of scope
/// for dispatch tests and should never be invoked here.
#[derive(Debug)]
struct TestHolonService;

fn unreachable_in_dispatch_tests<T>() -> Result<T, HolonError> {
    Err(HolonError::NotImplemented(
        "TestHolonService".to_string(),
    ))
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
        unreachable_in_dispatch_tests()
    }

    fn delete_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _local_id: &LocalId,
    ) -> Result<(), HolonError> {
        unreachable_in_dispatch_tests()
    }

    fn fetch_all_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        unreachable_in_dispatch_tests()
    }

    fn fetch_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _id: &HolonId,
    ) -> Result<Holon, HolonError> {
        unreachable_in_dispatch_tests()
    }

    fn fetch_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_dispatch_tests()
    }

    fn get_all_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_dispatch_tests()
    }

    fn load_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        unreachable_in_dispatch_tests()
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

fn test_options() -> RequestOptions {
    RequestOptions { gesture_id: None, gesture_label: None, snapshot_after: false }
}

fn build_test_runtime() -> Runtime {
    let space_manager = build_test_space_manager();
    let session = Arc::new(RuntimeSession::new(space_manager));
    Runtime::new(session)
}

/// Helper: begin a transaction and return the tx_id.
async fn begin_tx(runtime: &Runtime) -> TxId {
    let request = MapIpcRequest {
        request_id: RequestId::new(0),
        command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Ok(MapResultWire::TransactionCreated { tx_id }) => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    }
}

// ── Dispatch tests ──────────────────────────────────────────────────

#[tokio::test]
async fn begin_transaction_returns_valid_tx_id() {
    let runtime = build_test_runtime();

    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
        options: test_options(),
    };

    let response = runtime.dispatch(request).await.expect("dispatch should succeed");

    assert_eq!(response.request_id, RequestId::new(1));
    match response.result {
        Ok(MapResultWire::TransactionCreated { tx_id }) => {
            assert!(tx_id.value() > 0, "tx_id should be positive");
        }
        other => panic!("expected TransactionCreated, got {:?}", other),
    }
}

#[tokio::test]
async fn begin_transaction_ids_are_unique() {
    let runtime = build_test_runtime();

    let mut tx_ids = Vec::new();
    for i in 0..3 {
        let request = MapIpcRequest {
            request_id: RequestId::new(i),
            command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
            options: test_options(),
        };
        let response = runtime.dispatch(request).await.expect("dispatch should succeed");
        match response.result {
            Ok(MapResultWire::TransactionCreated { tx_id }) => tx_ids.push(tx_id),
            other => panic!("expected TransactionCreated, got {:?}", other),
        }
    }

    // All tx_ids should be unique
    for i in 0..tx_ids.len() {
        for j in (i + 1)..tx_ids.len() {
            assert_ne!(tx_ids[i], tx_ids[j], "tx_ids should be unique");
        }
    }
}

#[tokio::test]
async fn invalid_tx_id_returns_error() {
    let runtime = build_test_runtime();

    let bad_tx_id = serde_json::from_value(serde_json::json!(999)).unwrap();

    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id: bad_tx_id,
            action: TransactionActionWire::Commit,
        }),
        options: test_options(),
    };

    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Err(HolonError::InvalidParameter(msg)) => {
            assert!(msg.contains("999"), "error should mention the tx_id");
        }
        other => panic!("expected InvalidParameter error, got {:?}", other),
    }
}

// ── Transaction lookup dispatch ─────────────────────────────────────

#[tokio::test]
async fn staged_count_returns_zero_for_new_tx() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::StagedCount,
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Ok(MapResultWire::Value(BaseValue::IntegerValue(MapInteger(0)))) => {}
        other => panic!("expected Value(IntegerValue(0)), got {:?}", other),
    }
}

#[tokio::test]
async fn transient_count_returns_zero_for_new_tx() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::TransientCount,
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Ok(MapResultWire::Value(BaseValue::IntegerValue(MapInteger(0)))) => {}
        other => panic!("expected Value(IntegerValue(0)), got {:?}", other),
    }
}

// ── Transaction mutation dispatch ───────────────────────────────────

#[tokio::test]
async fn new_holon_then_staged_count() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    // NewHolon creates a transient
    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::NewHolon {
                key: Some(MapString::from("test-key")),
            },
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match &response.result {
        Ok(MapResultWire::Reference(_)) => {}
        other => panic!("expected Reference, got {:?}", other),
    }

    // Transient count should be 1
    let request = MapIpcRequest {
        request_id: RequestId::new(2),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::TransientCount,
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Ok(MapResultWire::Value(BaseValue::IntegerValue(MapInteger(1)))) => {}
        other => panic!("expected Value(IntegerValue(1)), got {:?}", other),
    }
}

#[tokio::test]
async fn new_holon_stage_then_staged_count() {
    let runtime = build_test_runtime();
    let tx_id = begin_tx(&runtime).await;

    // NewHolon → get transient ref wire
    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::NewHolon {
                key: Some(MapString::from("stage-test")),
            },
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    let transient_wire = match response.result {
        Ok(MapResultWire::Reference(r)) => r,
        other => panic!("expected Reference, got {:?}", other),
    };

    // StageNewHolon needs a TransientReferenceWire
    let transient_ref_wire = match transient_wire {
        holons_boundary::HolonReferenceWire::Transient(t) => t,
        other => panic!("expected Transient wire ref, got {:?}", other),
    };

    let request = MapIpcRequest {
        request_id: RequestId::new(2),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::StageNewHolon {
                source: transient_ref_wire,
            },
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match &response.result {
        Ok(MapResultWire::Reference(_)) => {}
        other => panic!("expected Reference (staged), got {:?}", other),
    }

    // StagedCount should be 1
    let request = MapIpcRequest {
        request_id: RequestId::new(3),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::StagedCount,
        }),
        options: test_options(),
    };
    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Ok(MapResultWire::Value(BaseValue::IntegerValue(MapInteger(1)))) => {}
        other => panic!("expected Value(IntegerValue(1)), got {:?}", other),
    }
}

// ── CommandDescriptor classification tests ──────────────────────────

#[test]
fn space_begin_transaction_descriptor() {
    let desc = SpaceCommand::BeginTransaction.descriptor();
    assert_eq!(desc.mutation, MutationClassification::Mutating);
    assert!(!desc.requires_open_tx);
    assert!(!desc.requires_commit_guard);
}

#[test]
fn transaction_action_descriptors() {
    assert_eq!(TransactionAction::Commit.descriptor(), CommandDescriptor::mutating_with_guard());
    assert_eq!(TransactionAction::StagedCount.descriptor(), CommandDescriptor::read_only());
    assert_eq!(TransactionAction::TransientCount.descriptor(), CommandDescriptor::read_only());
    assert_eq!(TransactionAction::GetAllHolons.descriptor(), CommandDescriptor::read_only());
    assert_eq!(
        TransactionAction::NewHolon { key: None }.descriptor(),
        CommandDescriptor::mutating()
    );
    assert_eq!(
        TransactionAction::DeleteHolon {
            local_id: LocalId(vec![]),
        }
        .descriptor(),
        CommandDescriptor::mutating()
    );
}

#[test]
fn holon_action_descriptors() {
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::Key).descriptor(),
        CommandDescriptor::read_only()
    );
    assert_eq!(
        HolonAction::Read(ReadableHolonAction::CloneHolon).descriptor(),
        CommandDescriptor::mutating(),
        "CloneHolon creates a transient — mutating despite being a ReadableHolonAction"
    );
    assert_eq!(
        HolonAction::Write(WritableHolonAction::WithPropertyValue {
            name: PropertyName(MapString::from("x")),
            value: BaseValue::StringValue(MapString::from("v")),
        })
        .descriptor(),
        CommandDescriptor::mutating()
    );
}
