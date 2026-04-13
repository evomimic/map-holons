use std::sync::Arc;

use core_types::HolonError;

use holons_core::core_shared_objects::transactions::TransactionLifecycleState;

use map_commands_contract::{
    HolonAction, MapCommand, MapResult, MutationClassification, ReadableHolonAction,
    SpaceCommand, TransactionAction,
};

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

#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionPolicy {
    pub snapshot_after: bool,
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
    pub async fn execute_command(&self, command: MapCommand) -> Result<MapResult, HolonError> {
        self.execute_command_with_policy(command, ExecutionPolicy::default())
            .await
    }

    /// Enforce lifecycle policy, apply execution policy, and route a bound
    /// domain command to its handler.
    pub async fn execute_command_with_policy(
        &self,
        command: MapCommand,
        policy: ExecutionPolicy,
    ) -> Result<MapResult, HolonError> {
        let descriptor = command.descriptor();
        let command_label = command_label(&command);

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

        let tx_id_for_snapshot = context.as_ref().map(|ctx| ctx.tx_id());
        let result = self.route_command(command).await?;

        if policy.snapshot_after && descriptor.mutation != MutationClassification::ReadOnly {
            if let Some(tx_id) = tx_id_for_snapshot {
                self.session
                    .persist_success(&tx_id, command_label, false)
                    .await?;
            }
        }

        Ok(result)
    }

    /// Routes a bound domain command to its scope-specific handler.
    async fn route_command(&self, command: MapCommand) -> Result<MapResult, HolonError> {
        match command {
            MapCommand::Space(cmd) => space_handler::handle_space(&self.session, cmd).await,
            MapCommand::Transaction(cmd) => {
                transaction_handler::handle_transaction(&self.session, cmd).await
            }
            MapCommand::Holon(cmd) => holon_handler::handle_holon(cmd).await,
        }
    }
}

fn command_label(command: &MapCommand) -> &'static str {
    match command {
        MapCommand::Space(SpaceCommand::BeginTransaction) => "begin_transaction",
        MapCommand::Transaction(cmd) => match &cmd.action {
            TransactionAction::Commit => "commit",
            TransactionAction::LoadHolons { .. } => "load_holons",
            TransactionAction::Dance(_) => "dance",
            TransactionAction::Query(_) => "query",
            TransactionAction::GetAllHolons => "get_all_holons",
            TransactionAction::GetStagedHolonByBaseKey { .. } => "get_staged_holon_by_base_key",
            TransactionAction::GetStagedHolonsByBaseKey { .. } => {
                "get_staged_holons_by_base_key"
            }
            TransactionAction::GetStagedHolonByVersionedKey { .. } => {
                "get_staged_holon_by_versioned_key"
            }
            TransactionAction::GetTransientHolonByBaseKey { .. } => {
                "get_transient_holon_by_base_key"
            }
            TransactionAction::GetTransientHolonByVersionedKey { .. } => {
                "get_transient_holon_by_versioned_key"
            }
            TransactionAction::StagedCount => "staged_count",
            TransactionAction::TransientCount => "transient_count",
            TransactionAction::NewHolon { .. } => "new_holon",
            TransactionAction::StageNewHolon { .. } => "stage_new_holon",
            TransactionAction::StageNewFromClone { .. } => "stage_new_from_clone",
            TransactionAction::StageNewVersion { .. } => "stage_new_version",
            TransactionAction::StageNewVersionFromId { .. } => "stage_new_version_from_id",
            TransactionAction::DeleteHolon { .. } => "delete_holon",
        },
        MapCommand::Holon(cmd) => match &cmd.action {
            HolonAction::Read(action) => match action {
                ReadableHolonAction::CloneHolon => "clone_holon",
                ReadableHolonAction::EssentialContent => "essential_content",
                ReadableHolonAction::Summarize => "summarize",
                ReadableHolonAction::HolonId => "holon_id",
                ReadableHolonAction::Predecessor => "predecessor",
                ReadableHolonAction::Key => "key",
                ReadableHolonAction::VersionedKey => "versioned_key",
                ReadableHolonAction::PropertyValue { .. } => "property_value",
                ReadableHolonAction::RelatedHolons { .. } => "related_holons",
            },
            HolonAction::Write(_) => "holon_write",
        },
    }
}
