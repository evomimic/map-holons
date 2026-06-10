use std::sync::RwLock;

use core_types::HolonError;
use map_commands_contract::MapCommand;
use map_commands_runtime::{ExecutionPolicy, Runtime};
use map_commands_wire::{MapCommandWire, MapIpcRequest, MapIpcResponse, MapResultWire};
use tauri::{command, State};

/// Tauri-managed state wrapper for the MAP Commands runtime.
///
/// Initially `None` until Holochain setup completes and the Runtime
/// is constructed in `run_complete_setup`.
pub type RuntimeState = RwLock<Option<Runtime>>;

#[command]
pub async fn dispatch_map_command(
    request: MapIpcRequest,
    runtime_state: State<'_, RuntimeState>,
) -> Result<MapIpcResponse, ()> {
    tracing::debug!("[TAURI COMMAND] 'dispatch_map_command' invoked");

    let request_id = request.request_id;

    let result =
        dispatch_inner(&request_id, request.command, request.options, &runtime_state).await;

    Ok(wrap_response(request_id, result))
}

/// Inner dispatch that returns `Result` so early errors are captured in the
/// response envelope rather than escaping as a bare Tauri error.
async fn dispatch_inner(
    request_id: &map_commands_wire::RequestId,
    command: MapCommandWire,
    options: map_commands_wire::RequestOptions,
    runtime_state: &RuntimeState,
) -> Result<map_commands_contract::MapResult, HolonError> {
    let runtime = load_runtime(runtime_state)?;

    let runtime = runtime.ok_or_else(|| {
        HolonError::ServiceNotAvailable("MAP Commands Runtime not initialized".to_string())
    })?;

    log_marker_context(request_id, &options);

    // Bind wire → domain
    let command = bind_command(&runtime, command)?;

    // Execute via runtime (policy enforcement + handler routing)
    runtime.execute_command(command, translate_request_options(options)).await
}

fn load_runtime(runtime_state: &RuntimeState) -> Result<Option<Runtime>, HolonError> {
    runtime_state
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(format!("RuntimeState lock poisoned: {}", e)))
        .map(|guard| guard.clone())
}

fn translate_request_options(options: map_commands_wire::RequestOptions) -> ExecutionPolicy {
    ExecutionPolicy {
        snapshot_after: options.snapshot_after,
        disable_undo: options.disable_undo,
        marker_id: options.marker_id.map(|m| m.0 .0),
        label: options.marker_label,
    }
}

fn wrap_response(
    request_id: map_commands_wire::RequestId,
    result: Result<map_commands_contract::MapResult, HolonError>,
) -> MapIpcResponse {
    let wire_result = result.map(MapResultWire::from);

    // Always Ok — all domain errors are inside the envelope.
    MapIpcResponse { request_id, result: wire_result }
}

fn log_marker_context(
    request_id: &map_commands_wire::RequestId,
    options: &map_commands_wire::RequestOptions,
) {
    if let Some(ref marker_id) = options.marker_id {
        let label = options.marker_label.as_deref().unwrap_or("<no label>");
        tracing::info!(
            "dispatch_map_command request_id={} marker_id={:?} label={}",
            request_id.value(),
            marker_id.0,
            label
        );
    }
}

/// Binds a wire command to its domain equivalent using the runtime session.
fn bind_command(runtime: &Runtime, command: MapCommandWire) -> Result<MapCommand, HolonError> {
    match command {
        MapCommandWire::Space(wire) => Ok(MapCommand::Space(wire.bind())),
        MapCommandWire::Transaction(wire) => {
            let context = runtime.session().get_transaction(&wire.tx_id)?;
            Ok(MapCommand::Transaction(wire.bind(context)?))
        }
        MapCommandWire::Holon(wire) => {
            let context = runtime.session().get_transaction(&wire.tx_id)?;
            Ok(MapCommand::Holon(wire.bind(&context)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use base_types::{BaseValue, MapString};
    use core_types::{HolonError, HolonId, LocalId, RelationshipName};
    use holons_boundary::HolonReferenceWire;
    use holons_core::core_shared_objects::space_manager::HolonSpaceManager;
    use holons_core::core_shared_objects::transactions::TransactionContext;
    use holons_core::core_shared_objects::{
        Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy,
    };
    use holons_core::reference_layer::{HolonServiceApi, StagedReference, TransientReference};

    use super::*;

    #[derive(Debug)]
    struct TestHolonService;

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
        let session = Arc::new(map_commands_runtime::RuntimeSession::new(space_manager, None));
        Runtime::new(session)
    }

    fn runtime_state(runtime: Option<Runtime>) -> RuntimeState {
        RwLock::new(runtime)
    }

    fn tx_id(value: i64) -> holons_core::core_shared_objects::transactions::TxId {
        serde_json::from_value(serde_json::json!(value)).unwrap()
    }

    fn transaction_command(tx: i64) -> MapCommandWire {
        MapCommandWire::Transaction(map_commands_wire::TransactionCommandWire {
            tx_id: tx_id(tx),
            action: map_commands_wire::TransactionActionWire::GetTransientCount,
        })
    }

    fn default_request_options() -> map_commands_wire::RequestOptions {
        map_commands_wire::RequestOptions {
            marker_id: None,
            marker_label: None,
            snapshot_after: false,
            disable_undo: false,
        }
    }

    impl HolonServiceApi for TestHolonService {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn commit_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _staged_references: &[StagedReference],
        ) -> Result<TransientReference, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn delete_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _local_id: &LocalId,
        ) -> Result<(), HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn fetch_all_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
        ) -> Result<RelationshipMap, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn fetch_holon_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _id: &HolonId,
        ) -> Result<Holon, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn fetch_related_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _source_id: &HolonId,
            _relationship_name: &RelationshipName,
        ) -> Result<HolonCollection, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn get_all_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
        ) -> Result<HolonCollection, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }

        fn load_holons_internal(
            &self,
            _context: &Arc<TransactionContext>,
            _bundle: TransientReference,
        ) -> Result<TransientReference, HolonError> {
            Err(HolonError::NotImplemented("TestHolonService".to_string()))
        }
    }

    #[tokio::test]
    async fn load_runtime_returns_none_when_uninitialized() {
        let state = runtime_state(None);
        assert!(load_runtime(&state).unwrap().is_none());
    }

    #[tokio::test]
    async fn load_runtime_reports_poisoned_lock() {
        let state = Arc::new(runtime_state(None));
        let state_for_thread = Arc::clone(&state);

        let _ = std::thread::spawn(move || {
            let _guard = state_for_thread.write().unwrap();
            panic!("poison runtime state");
        })
        .join();

        match load_runtime(&*state) {
            Err(HolonError::FailedToAcquireLock(msg)) => {
                assert!(msg.contains("RuntimeState"));
            }
            other => panic!("expected FailedToAcquireLock, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn translate_request_options_sets_disable_undo() {
        let options = map_commands_wire::RequestOptions {
            marker_id: None,
            marker_label: Some("label".to_string()),
            snapshot_after: true,
            disable_undo: true,
        };

        let policy = translate_request_options(options);
        assert!(policy.snapshot_after);
        assert!(policy.disable_undo);
        assert_eq!(policy.label.as_deref(), Some("label"));
    }

    #[tokio::test]
    async fn wrap_response_keeps_success_inside_envelope() {
        let response = wrap_response(
            map_commands_wire::RequestId::new(17),
            Ok(map_commands_contract::MapResult::None),
        );

        assert_eq!(response.request_id.value(), 17);
        assert!(matches!(response.result, Ok(MapResultWire::None)));
    }

    #[tokio::test]
    async fn wrap_response_keeps_error_inside_envelope() {
        let response = wrap_response(
            map_commands_wire::RequestId::new(17),
            Err(HolonError::ServiceNotAvailable("missing".to_string())),
        );

        assert_eq!(response.request_id.value(), 17);
        assert!(matches!(response.result, Err(HolonError::ServiceNotAvailable(_))));
    }

    #[tokio::test]
    async fn bind_command_rejects_unknown_tx_id() {
        let runtime = build_test_runtime();
        let result = bind_command(&runtime, transaction_command(999));

        match result {
            Err(HolonError::InvalidParameter(msg)) => {
                assert!(msg.contains("tx_id=999"));
            }
            other => panic!("expected InvalidParameter, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn dispatch_inner_wraps_runtime_missing_in_error_payload() {
        let state = runtime_state(None);
        let result = dispatch_inner(
            &map_commands_wire::RequestId::new(1),
            MapCommandWire::Space(map_commands_wire::SpaceCommandWire::BeginTransaction),
            default_request_options(),
            &state,
        )
        .await;

        match result {
            Err(HolonError::ServiceNotAvailable(msg)) => {
                assert!(msg.contains("not initialized"));
            }
            other => panic!("expected ServiceNotAvailable, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn dispatch_inner_returns_successful_domain_result() {
        let state = runtime_state(Some(build_test_runtime()));
        let result = dispatch_inner(
            &map_commands_wire::RequestId::new(1),
            MapCommandWire::Space(map_commands_wire::SpaceCommandWire::BeginTransaction),
            default_request_options(),
            &state,
        )
        .await
        .expect("dispatch should succeed");

        assert!(matches!(result, map_commands_contract::MapResult::TransactionCreated { .. }));
    }

    #[tokio::test]
    async fn dispatch_inner_binds_and_executes_transaction_command() {
        let state = runtime_state(Some(build_test_runtime()));
        let tx_id = match dispatch_inner(
            &map_commands_wire::RequestId::new(1),
            MapCommandWire::Space(map_commands_wire::SpaceCommandWire::BeginTransaction),
            default_request_options(),
            &state,
        )
        .await
        .expect("begin transaction should succeed")
        {
            map_commands_contract::MapResult::TransactionCreated { tx_id } => tx_id,
            other => panic!("expected TransactionCreated, got {:?}", other),
        };

        let result = dispatch_inner(
            &map_commands_wire::RequestId::new(2),
            MapCommandWire::Transaction(map_commands_wire::TransactionCommandWire {
                tx_id,
                action: map_commands_wire::TransactionActionWire::GetTransientCount,
            }),
            default_request_options(),
            &state,
        )
        .await
        .expect("transaction dispatch should succeed");

        assert!(matches!(
            result,
            map_commands_contract::MapResult::Value(BaseValue::IntegerValue(_))
        ));
    }

    #[tokio::test]
    async fn dispatch_inner_binds_and_executes_holon_command() {
        let state = runtime_state(Some(build_test_runtime()));
        let tx_id = match dispatch_inner(
            &map_commands_wire::RequestId::new(1),
            MapCommandWire::Space(map_commands_wire::SpaceCommandWire::BeginTransaction),
            default_request_options(),
            &state,
        )
        .await
        .expect("begin transaction should succeed")
        {
            map_commands_contract::MapResult::TransactionCreated { tx_id } => tx_id,
            other => panic!("expected TransactionCreated, got {:?}", other),
        };

        let transient = match dispatch_inner(
            &map_commands_wire::RequestId::new(2),
            MapCommandWire::Transaction(map_commands_wire::TransactionCommandWire {
                tx_id,
                action: map_commands_wire::TransactionActionWire::NewHolon {
                    key: Some(MapString::from("alpha")),
                },
            }),
            default_request_options(),
            &state,
        )
        .await
        .expect("new holon should succeed")
        {
            map_commands_contract::MapResult::Reference(
                holons_core::reference_layer::HolonReference::Transient(transient),
            ) => transient,
            other => panic!("expected transient reference, got {:?}", other),
        };

        let result = dispatch_inner(
            &map_commands_wire::RequestId::new(3),
            MapCommandWire::Holon(map_commands_wire::HolonCommandWire {
                tx_id,
                target: HolonReferenceWire::from(
                    holons_core::reference_layer::HolonReference::Transient(transient),
                ),
                action: map_commands_wire::HolonActionWire::Read(
                    map_commands_wire::ReadableHolonActionWire::GetKey,
                ),
            }),
            default_request_options(),
            &state,
        )
        .await
        .expect("holon dispatch should succeed");

        assert!(matches!(
            result,
            map_commands_contract::MapResult::Value(BaseValue::StringValue(value))
                if value == MapString::from("alpha")
        ));
    }
}
