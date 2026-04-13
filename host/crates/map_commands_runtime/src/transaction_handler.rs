use base_types::{BaseValue, MapInteger};
use core_types::HolonError;
use holons_core::reference_layer::HolonReference;

use map_commands_contract::{MapResult, TransactionAction, TransactionCommand};

use super::runtime_session::RuntimeSession;

/// Handles transaction-scoped commands.
pub async fn handle_transaction(
    session: &RuntimeSession,
    command: TransactionCommand,
) -> Result<MapResult, HolonError> {
    let context = &command.context;

    match command.action {
        // ── Commit ───────────────────────────────────────────────────
        TransactionAction::Commit => {
            let transient_ref = session.commit_transaction(&command.context.tx_id()).await?;
            Ok(MapResult::Reference(HolonReference::Transient(transient_ref)))
        }

        // ── Dance / Query / LoadHolons ───────────────────────────────
        TransactionAction::Dance(request) => {
            let response = context.initiate_ingress_dance(request, false).await?;
            Ok(MapResult::DanceResponse(response))
        }
        TransactionAction::Query(_) => {
            Err(HolonError::NotImplemented("TransactionAction::Query".to_string()))
        }
        TransactionAction::LoadHolons { bundle } => {
            // LoadHolons requires a TransientReference; extract from the bound HolonReference
            match bundle {
                HolonReference::Transient(transient_ref) => {
                    let result = context.load_holons_and_commit(transient_ref)?;
                    Ok(MapResult::Reference(HolonReference::Transient(result)))
                }
                other => Err(HolonError::InvalidParameter(format!(
                    "LoadHolons requires a TransientReference, got {:?}",
                    other
                ))),
            }
        }

        // ── Lookup actions (LookupFacade) ────────────────────────────
        TransactionAction::GetAllHolons => {
            let collection = context.lookup().get_all_holons()?;
            Ok(MapResult::Collection(collection))
        }
        TransactionAction::GetStagedHolonByBaseKey { key } => {
            let staged = context.lookup().get_staged_holon_by_base_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::GetStagedHolonsByBaseKey { key } => {
            let staged_refs = context.lookup().get_staged_holons_by_base_key(&key)?;
            Ok(MapResult::References(staged_refs.into_iter().map(HolonReference::Staged).collect()))
        }
        TransactionAction::GetStagedHolonByVersionedKey { key } => {
            let staged = context.lookup().get_staged_holon_by_versioned_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::GetTransientHolonByBaseKey { key } => {
            let transient = context.lookup().get_transient_holon_by_base_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Transient(transient)))
        }
        TransactionAction::GetTransientHolonByVersionedKey { key } => {
            let transient = context.lookup().get_transient_holon_by_versioned_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Transient(transient)))
        }
        TransactionAction::StagedCount => {
            let count = context.lookup().staged_count()?;
            Ok(MapResult::Value(BaseValue::IntegerValue(MapInteger(count))))
        }
        TransactionAction::TransientCount => {
            let count = context.lookup().transient_count()?;
            Ok(MapResult::Value(BaseValue::IntegerValue(MapInteger(count))))
        }

        // ── Mutation actions (MutationFacade) ────────────────────────
        TransactionAction::NewHolon { key } => {
            let transient = context.mutation().new_holon(key)?;
            Ok(MapResult::Reference(HolonReference::Transient(transient)))
        }
        TransactionAction::StageNewHolon { source } => {
            let staged = context.mutation().stage_new_holon(source)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::StageNewFromClone { original, new_key } => {
            let staged = context.mutation().stage_new_from_clone(original, new_key)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::StageNewVersion { current_version } => {
            let staged = context.mutation().stage_new_version(current_version)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::StageNewVersionFromId { holon_id } => {
            let staged = context.mutation().stage_new_version_from_id(holon_id)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::DeleteHolon { local_id } => {
            context.mutation().delete_holon(local_id)?;
            Ok(MapResult::None)
        }
    }
}
