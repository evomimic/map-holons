use base_types::{BaseValue, MapInteger};
use core_types::HolonError;
use holons_core::dances::execute_dance_v2;
use holons_core::{HolonReference, ReadableHolon};
use map_commands_contract::{MapResult, TransactionAction, TransactionCommand};

use super::runtime_session::RuntimeSession;

/// Handles transaction-scoped commands.
pub async fn handle_transaction(
    session: &RuntimeSession,
    command: TransactionCommand,
) -> Result<MapResult, HolonError> {
    let context = &command.context;

    match command.action {
        TransactionAction::Dispose => {
            session.dispose_transaction(&context.tx_id()).await?;
            Ok(MapResult::None)
        }
        TransactionAction::CheckLoadTarget { space } => {
            let expected = context
                .get_space_holon()?
                .ok_or_else(|| HolonError::InvalidState("No persisted HolonSpace".into()))?;
            if space.holon_id()? != expected.holon_id()? {
                return Err(HolonError::InvalidParameter(
                    "Captured HolonSpace does not match the loader transaction".into(),
                ));
            }
            Ok(MapResult::None)
        }
        TransactionAction::Commit => {
            let response = context.commit()?;
            Ok(MapResult::Reference(HolonReference::Transient(response)))
        }
        TransactionAction::UndoLast => {
            session.undo_last(&command.context.tx_id()).await?;
            Ok(MapResult::UndoComplete)
        }
        TransactionAction::RedoLast => {
            session.redo_last(&command.context.tx_id()).await?;
            Ok(MapResult::RedoComplete)
        }
        TransactionAction::UndoToMarker { marker_id } => {
            session.undo_to_marker(&command.context.tx_id(), &marker_id).await?;
            Ok(MapResult::UndoToMarkerComplete)
        }
        TransactionAction::RedoToMarker { marker_id } => {
            session.redo_to_marker(&command.context.tx_id(), &marker_id).await?;
            Ok(MapResult::RedoToMarkerComplete)
        }
        TransactionAction::Dance(request) => {
            let response = context.initiate_ingress_dance(request, false).await?;
            // Deliberate transitional exception: dance execution still returns
            // a `DanceResponse` instead of projecting onto the canonical
            // command result family.
            Ok(MapResult::DanceResponse(response))
        }
        TransactionAction::DanceV2 { invocation } => {
            let is_load = invocation.dance_name()?.to_string() == "LoadHolons";
            let mut completion = if is_load {
                super::load_request_completion::LoadRequestCompletion::from_invocation(
                    context,
                    &invocation,
                )?
            } else {
                None
            };
            let result = async {
                if is_load {
                    let resolved =
                        holons_core::dances::resolve_dance_v2_invocation(invocation.clone())?;
                    let request = resolved.bound_invocation().request().ok_or_else(|| {
                        HolonError::InvalidParameter(
                            "LoadHolons requires a prepared request".into(),
                        )
                    })?;
                    session.admission(context.tx_id())?.submit(request)?;
                }
                execute_dance_v2(context, invocation).await
            }
            .await;
            match result {
                Ok(response) => {
                    let response = HolonReference::from(response);
                    if let Some(completion) = completion.as_mut() {
                        completion.responded(response.clone())?;
                    }
                    Ok(MapResult::Reference(response))
                }
                Err(error) => {
                    if let Some(completion) = completion.as_mut() {
                        completion.failed(context, &error)?;
                    }
                    Err(error)
                }
            }
        }
        TransactionAction::SelectCollectionVisualizer { collection, parent_visualizer, slot } => {
            Ok(MapResult::VisualizerSelection(dahn_selection::select_collection_visualizer(
                context,
                collection,
                parent_visualizer,
                slot,
            )?))
        }
        TransactionAction::DiscoverVisualizers { request, current_selection, retain_evidence } => {
            let mut discovery =
                dahn_selection::discover_visualizers(context, request.clone(), current_selection)?;
            if retain_evidence {
                discovery.snapshot =
                    Some(session.discoveries.retain(context.tx_id(), request, &discovery)?);
            }
            Ok(MapResult::VisualizerDiscovery(discovery))
        }
        TransactionAction::ProjectVisualizerDiscovery { snapshot } => {
            Ok(MapResult::Reference(session.discoveries.project(context, &snapshot)?))
        }
        TransactionAction::ReleaseVisualizerDiscovery { snapshot } => {
            session.discoveries.release(&context.tx_id(), &snapshot)?;
            Ok(MapResult::None)
        }
        TransactionAction::ChooseVisualizer { request, candidate } => {
            Ok(MapResult::VisualizerSelection(dahn_selection::choose_visualizer(
                context, request, candidate,
            )?))
        }
        TransactionAction::SelectVisualizerUsage { request, selected } => {
            Ok(MapResult::VisualizerUsageSelection(session.usage_transactions.select(
                session.space_manager(),
                context,
                request,
                selected,
            )?))
        }
        TransactionAction::RecordVisualizerUse { request, selected, usage, origin, report } => {
            session.usage_transactions.record(
                session.space_manager(),
                context,
                request,
                selected,
                usage,
                origin,
                report,
            )?;
            Ok(MapResult::None)
        }
        TransactionAction::SelectVisualizer { request } => {
            Ok(MapResult::VisualizerSelection(dahn_selection::select_visualizer(context, request)?))
        }
        TransactionAction::FetchArtifact { handle } => {
            Ok(MapResult::Value(BaseValue::BytesValue(context.fetch_artifact(&handle)?)))
        }
        TransactionAction::PrepareHolons { content_set } => {
            session.admission(context.tx_id())?.begin_preparation()?;
            let request =
                holons_loader_client::prepare_holons_from_files(context.clone(), content_set)?;
            super::prepared_load_description::describe_prepared_load(
                context,
                request.clone().into(),
            )?;
            session
                .admission(context.tx_id())?
                .prepared(HolonReference::Transient(request.clone()))?;
            Ok(MapResult::Reference(HolonReference::Transient(request)))
        }
        TransactionAction::LoadHolons { content_set } => {
            let response =
                holons_loader_client::load_holons_from_files(context.clone(), content_set).await?;
            Ok(MapResult::Reference(HolonReference::Transient(response)))
        }
        TransactionAction::GetCommittedHolons => {
            let mut members = Vec::new();
            for staged in context.staged_references()? {
                // State inspection distinguishes expected unsaved candidates from read failures.
                if staged.is_committed()? {
                    members.push(HolonReference::smart_from_id(
                        context.space_read_handle(),
                        staged.holon_id()?,
                    ));
                }
            }
            // Membership is known; keys must be fetched in the separate review context.
            Ok(MapResult::Collection(holons_core::HolonCollection::from_parts(
                holons_core::CollectionState::Fetched,
                members,
                Default::default(),
            )))
        }
        TransactionAction::GetAllHolons => {
            let collection = context.lookup().get_all_holons()?;
            Ok(MapResult::Collection(collection))
        }
        TransactionAction::GetSavedHolonByBaseKey { key } => {
            let saved = context.lookup().get_saved_holon_by_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Smart(saved)))
        }
        TransactionAction::GetStagedHolonByBaseKey { key } => {
            let staged = context.lookup().get_staged_holon_by_base_key(&key)?;
            Ok(MapResult::Reference(HolonReference::Staged(staged)))
        }
        TransactionAction::GetStagedHolonsByBaseKey { key } => {
            let staged_refs = context.lookup().get_staged_holons_by_base_key(&key)?;
            // Deliberate exception: duplicate base-key staging lookup stays
            // reference-shaped rather than returning `HolonCollection`.
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
        TransactionAction::GetStagedCount => {
            let count = context.lookup().staged_count()?;
            Ok(MapResult::Value(BaseValue::IntegerValue(MapInteger(count))))
        }
        TransactionAction::GetTransientCount => {
            let count = context.lookup().transient_count()?;
            Ok(MapResult::Value(BaseValue::IntegerValue(MapInteger(count))))
        }
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
