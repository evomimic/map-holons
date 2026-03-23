use std::sync::Arc;

use core_types::HolonError;
use tracing::info;

use holons_core::core_shared_objects::transactions::TransactionLifecycleState;

use crate::domain::{MapCommand, MapResult, MutationClassification};
use crate::wire::{MapCommandWire, MapIpcRequest, MapIpcResponse, MapResultWire};

use super::runtime_session::RuntimeSession;
use super::{holon_handler, space_handler, transaction_handler};

/// The MAP Commands execution boundary.
///
/// All MAP command execution flows through `Runtime::handle_ipc`. It implements
/// the sandwich model: wire → bind → enforce policy → route to handler → wire.
///
/// Runtime is app-scoped and owns a `RuntimeSession` for transaction lifecycle.
#[derive(Debug, Clone)]
pub struct Runtime {
    session: Arc<RuntimeSession>,
}

impl Runtime {
    pub fn new(session: Arc<RuntimeSession>) -> Self {
        Self { session }
    }

    /// Single IPC entrypoint (the full sandwich).
    ///
    /// 1. Bind wire command → domain command
    /// 2. Enforce lifecycle via CommandDescriptor
    /// 3. Route to scope-specific handler
    /// 4. Convert domain result → wire result
    pub async fn handle_ipc(&self, request: MapIpcRequest) -> Result<MapIpcResponse, HolonError> {
        let request_id = request.request_id;

        // Log gesture context if present
        if let Some(ref gesture_id) = request.options.gesture_id {
            let label = request.options.gesture_label.as_deref().unwrap_or("<no label>");
            info!(
                "handle_ipc request_id={} gesture_id={:?} label={}",
                request_id.value(),
                gesture_id.0,
                label
            );
        }

        let result = self.execute_bound_command(request.command).await;

        // Convert domain result to wire, preserving errors
        let wire_result = match result {
            Ok(domain_result) => Ok(MapResultWire::from(domain_result)),
            Err(error) => Err(error),
        };

        Ok(MapIpcResponse { request_id, result: wire_result })
    }

    /// Bind + lifecycle enforcement + route to handler.
    async fn execute_bound_command(
        &self,
        command_wire: MapCommandWire,
    ) -> Result<MapResult, HolonError> {
        let command = self.bind(command_wire)?;

        let descriptor = command.descriptor();

        // Extract context for lifecycle checks (Transaction and Holon commands have one)
        let context = match &command {
            MapCommand::Transaction(cmd) => Some(Arc::clone(&cmd.context)),
            MapCommand::Holon(cmd) => Some(Arc::clone(&cmd.context)),
            MapCommand::Space(_) => None,
        };

        // Open-transaction check: reject commands that require an open transaction
        if descriptor.requires_open_tx {
            if let Some(ref ctx) = context {
                if !ctx.is_open() {
                    let tx_id = ctx.tx_id().value();
                    return match ctx.lifecycle_state() {
                        TransactionLifecycleState::Committed => {
                            Err(HolonError::TransactionAlreadyCommitted { tx_id })
                        }
                        other => Err(HolonError::TransactionNotOpen {
                            tx_id,
                            state: format!("{:?}", other),
                        }),
                    };
                }
            }
        }

        // Commit guard: hold across handler execution for commit-guarded commands
        let _commit_guard = if descriptor.requires_commit_guard {
            if let Some(ref ctx) = context {
                Some(ctx.begin_host_commit_ingress_guard()?)
            } else {
                None
            }
        } else {
            None
        };

        // Mutation entry check: for non-commit-guarded mutating commands
        if !descriptor.requires_commit_guard
            && descriptor.mutation == MutationClassification::Mutating
        {
            if let Some(ref ctx) = context {
                ctx.ensure_host_mutation_entry_allowed()?;
            }
        }

        self.route_command(command).await
    }

    /// Binds a wire command to its domain equivalent.
    fn bind(&self, command: MapCommandWire) -> Result<MapCommand, HolonError> {
        match command {
            MapCommandWire::Space(wire) => Ok(MapCommand::Space(wire.bind())),
            MapCommandWire::Transaction(wire) => {
                let context = self.session.get_transaction(&wire.tx_id)?;
                Ok(MapCommand::Transaction(wire.bind(context)?))
            }
            MapCommandWire::Holon(wire) => {
                let context = self.session.get_transaction(&wire.tx_id)?;
                Ok(MapCommand::Holon(wire.bind(&context)?))
            }
        }
    }

    /// Routes a bound domain command to its scope-specific handler.
    async fn route_command(&self, command: MapCommand) -> Result<MapResult, HolonError> {
        match command {
            MapCommand::Space(cmd) => space_handler::handle_space(&self.session, cmd),
            MapCommand::Transaction(cmd) => {
                transaction_handler::handle_transaction(&self.session, cmd).await
            }
            MapCommand::Holon(cmd) => holon_handler::handle_holon(cmd).await,
        }
    }
}
