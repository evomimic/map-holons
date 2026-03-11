use std::sync::Arc;

use core_types::HolonError;

use crate::domain::{MapCommand, MapResult};
use crate::wire::{MapCommandWire, MapIpcRequest, MapIpcResponse, MapResultWire};

use super::runtime_session::RuntimeSession;
use super::{holon_dispatch, space_dispatch, transaction_dispatch};

/// The MAP Commands execution boundary.
///
/// All MAP command execution flows through `Runtime::dispatch`. It implements
/// the sandwich model: wire → bind → domain dispatch → wire.
///
/// Runtime is app-scoped and owns a `RuntimeSession` for transaction lifecycle.
#[derive(Debug)]
pub struct Runtime {
    session: Arc<RuntimeSession>,
}

impl Runtime {
    pub fn new(session: Arc<RuntimeSession>) -> Self {
        Self { session }
    }

    /// Single IPC dispatch entrypoint (the full sandwich).
    ///
    /// 1. Bind wire command → domain command
    /// 2. Dispatch domain command
    /// 3. Convert domain result → wire result
    pub async fn dispatch(&self, request: MapIpcRequest) -> Result<MapIpcResponse, HolonError> {
        let request_id = request.request_id;

        let result = self.dispatch_inner(request.command).await;

        // Convert domain result to wire, preserving errors
        let wire_result = match result {
            Ok(domain_result) => Ok(MapResultWire::from(domain_result)),
            Err(error) => Err(error),
        };

        Ok(MapIpcResponse { request_id, result: wire_result })
    }

    /// Bind + dispatch (separated for cleaner error handling).
    async fn dispatch_inner(&self, command_wire: MapCommandWire) -> Result<MapResult, HolonError> {
        let command = self.bind(command_wire)?;
        self.dispatch_command(command).await
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

    /// Dispatches a bound domain command to scope-specific handlers.
    async fn dispatch_command(&self, command: MapCommand) -> Result<MapResult, HolonError> {
        match command {
            MapCommand::Space(cmd) => space_dispatch::dispatch_space(&self.session, cmd),
            MapCommand::Transaction(cmd) => {
                transaction_dispatch::dispatch_transaction(&self.session, cmd).await
            }
            MapCommand::Holon(cmd) => holon_dispatch::dispatch_holon(cmd).await,
        }
    }
}
