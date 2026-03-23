use std::sync::Arc;

use core_types::HolonError;

use holons_core::core_shared_objects::transactions::TransactionLifecycleState;

use map_commands_contract::{MapCommand, MapResult, MutationClassification};

use super::runtime_session::RuntimeSession;
use super::{holon_handler, space_handler, transaction_handler};

/// The MAP Commands execution boundary.
///
/// All MAP command execution flows through `Runtime::execute_command`. It
/// enforces lifecycle policy via `CommandDescriptor` and routes to
/// scope-specific handlers.
///
/// Wire binding (IPC envelope → domain command) is handled by the caller
/// (Conductora) before reaching this layer.
#[derive(Debug, Clone)]
pub struct Runtime {
    session: Arc<RuntimeSession>,
}

impl Runtime {
    pub fn new(session: Arc<RuntimeSession>) -> Self {
        Self { session }
    }

    /// Returns a reference to the session for transaction lookups during binding.
    pub fn session(&self) -> &Arc<RuntimeSession> {
        &self.session
    }

    /// Enforce lifecycle policy and route a bound domain command to its handler.
    pub async fn execute_command(
        &self,
        command: MapCommand,
    ) -> Result<MapResult, HolonError> {
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
