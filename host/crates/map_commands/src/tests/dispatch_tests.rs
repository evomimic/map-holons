use std::any::Any;
use std::sync::Arc;

use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
use holons_core::core_shared_objects::{
    Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
};
use holons_core::reference_layer::{HolonServiceApi, StagedReference, TransientReference};

use crate::dispatch::{Runtime, RuntimeSession};
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

fn build_test_runtime() -> Runtime {
    let space_manager = build_test_space_manager();
    let session = Arc::new(RuntimeSession::new(space_manager));
    Runtime::new(session)
}

// ── Dispatch tests ──────────────────────────────────────────────────

#[tokio::test]
async fn begin_transaction_returns_valid_tx_id() {
    let runtime = build_test_runtime();

    let request = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
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
async fn unimplemented_command_returns_not_implemented() {
    let runtime = build_test_runtime();

    // First open a transaction so we have a valid tx_id
    let begin_req = MapIpcRequest {
        request_id: RequestId::new(1),
        command: MapCommandWire::Space(SpaceCommandWire::BeginTransaction),
    };
    let begin_resp = runtime.dispatch(begin_req).await.expect("dispatch should succeed");
    let tx_id = match begin_resp.result {
        Ok(MapResultWire::TransactionCreated { tx_id }) => tx_id,
        other => panic!("expected TransactionCreated, got {:?}", other),
    };

    // Try an unimplemented transaction command
    let request = MapIpcRequest {
        request_id: RequestId::new(2),
        command: MapCommandWire::Transaction(TransactionCommandWire {
            tx_id,
            action: TransactionActionWire::Lookup(LookupQueryWire::GetAllHolons),
        }),
    };

    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    assert_eq!(response.request_id, RequestId::new(2));
    match response.result {
        Err(HolonError::NotImplemented(_)) => {} // expected
        other => panic!("expected NotImplemented error, got {:?}", other),
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
    };

    let response = runtime.dispatch(request).await.expect("dispatch should succeed");
    match response.result {
        Err(HolonError::InvalidParameter(msg)) => {
            assert!(msg.contains("999"), "error should mention the tx_id");
        }
        other => panic!("expected InvalidParameter error, got {:?}", other),
    }
}
