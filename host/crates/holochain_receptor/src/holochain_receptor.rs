use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use crate::holochain_conductor_client::HolochainConductorClient;
use base_types::MapString;
use core_types::HolonError;
use holons_client::{
    client_context::ClientSession, dances_client::ClientDanceBuilder, init_client_context, shared_types::{
        base_receptor::{BaseReceptor, ReceptorBehavior},
        holon_space::{HolonSpace, SpaceInfo},
        map_request::{MapRequest, MapRequestBody},
        map_response::MapResponse,
    }
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceInitiator, DanceResponse, ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::HolonReference;
use holons_loader_client::load_holons_from_files;
use holons_trust_channel::TrustChannel;

/// POC-safe Holochain Receptor.
/// Enough to satisfy Conductora runtime configuration.
/// Does NOT implement full space loading / root holon discovery yet.
pub struct HolochainReceptor {
    receptor_id: Option<String>,
    receptor_type: String,
    properties: HashMap<String, String>,
    session: ClientSession,
    client_handler: Arc<HolochainConductorClient>,
    _home_space_holon: HolonSpace,
}

impl HolochainReceptor {
    fn is_commit_dance_request(request_name: &str) -> bool {
        matches!(request_name, "commit")
    }

    fn is_read_only_request(request_name: &str) -> bool {
        matches!(request_name, "get_all_holons" | "get_holon_by_id" | "query_relationships")
    }

    pub fn new(base: BaseReceptor) -> Self {
        // Downcast the stored client into our concrete conductor client
        let client_any =
            base.client_handler.as_ref().expect("Client is required for HolochainReceptor").clone();

        let client_handler = client_any
            .downcast::<HolochainConductorClient>()
            .expect("Failed to downcast client to HolochainConductorClient");

        // Trust channel wraps the conductor client
        let trust_channel = TrustChannel::new(client_handler.clone());
        let initiator: Arc<dyn DanceInitiator + Send + Sync> = Arc::new(trust_channel);

        // Build client context with dance initiator and recovery store if provided
        let session = if let Some(recovery_store) = base.snapshot_store.as_ref() {
            init_client_context(Some(initiator), Some(Arc::clone(recovery_store)))
        } else {
            init_client_context(Some(initiator), None)
        };

        // Default until we fully implement space discovery
        let _home_space_holon = HolonSpace::default();

        Self {
            receptor_id: base.receptor_id.clone(),
            receptor_type: base.receptor_type.clone(),
            properties: base.properties.clone(),
            session,
            client_handler,
            _home_space_holon,
        }
    }
}

#[async_trait]
impl ReceptorBehavior for HolochainReceptor {
    fn transaction_context(&self) -> Arc<TransactionContext> {
        Arc::clone(&self.session.context)
    }

    /// Core request → client dance pipeline
    async fn handle_map_request(&self, request: MapRequest) -> Result<MapResponse, HolonError> {
        // Temporary Phase 1.4/1.5 bridge: commit-like requests serialize host
        // ingress here. In Phase 2 this moves to CommandDispatcher.
        if Self::is_commit_dance_request(request.name.as_str()) {
            let _commit_guard = self.session.context.begin_host_commit_ingress_guard()?;
            // Preserve request-shape validation before routing to context-owned commit execution.
            let _validated_request = ClientDanceBuilder::validate_and_execute(&self.session.context, &request)?;
            let response_reference = self.session.context.commit()?;
            let dance_response = DanceResponse::new(
                ResponseStatusCode::OK,
                MapString("Commit executed via TransactionContext".to_string()),
                ResponseBody::HolonReference(HolonReference::Transient(response_reference)),
                None,
            );

            return Ok(MapResponse::new_from_dance_response(request.space.id, dance_response));
        }

        // Temporary Phase 1.4/1.5 bridge: load+commit is commit-like and
        // should serialize at host ingress until CommandDispatcher owns this.
        if request.name == "load_holons" {
            let _commit_guard = self.session.context.begin_host_commit_ingress_guard()?;

            let content_set = match request.body {
                MapRequestBody::LoadHolons(content_set) => content_set,
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Expected LoadHolons body for load_holons request".into(),
                    ))
                }
            };

            let response_reference = load_holons_from_files(self.session.context.clone(), content_set).await?;
            tracing::info!(
                "HolochainReceptor: loaded holons with reference: {:?}",
                response_reference
            );

            let dance_response = DanceResponse::new(
                ResponseStatusCode::OK,
                MapString("LoadHolons executed via TransactionContext".to_string()),
                ResponseBody::HolonReference(HolonReference::Transient(response_reference)),
                None,
            );

            return Ok(MapResponse::new_from_dance_response(request.space.id, dance_response));
        }

        // Read/query requests remain available during host commit ingress and after
        // lifecycle reaches Committed so clients can inspect commit/load results.
        // External write/mutation requests (including transient creation) require
        // an open transaction and must be blocked during host commit ingress.
        let is_read_only = Self::is_read_only_request(request.name.as_str());
        if !is_read_only {
            self.session.context.ensure_host_mutation_entry_allowed()?;
        }

        let dance_request = ClientDanceBuilder::validate_and_execute(&self.session.context, &request)?;
        let dance_response = self
            .session.context
            .initiate_ingress_dance(dance_request, is_read_only)
            .await?;

        Ok(MapResponse::new_from_dance_response(request.space.id, dance_response))
    }

    /// POC stub for system info
    async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        // Call stubbed conductor client
        self.client_handler.get_all_spaces().await
    }
}

impl Debug for HolochainReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HolochainReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::HolochainReceptor;
    use base_types::{BaseValue, MapString};
    use core_types::{PropertyMap, PropertyName};
    use holons_client::{
        dances_client::ClientDanceBuilder,
        init_client_context,
        shared_types::{
            holon_space::HolonSpace,
            map_request::{MapRequest, MapRequestBody},
        },
    };
    use holons_core::dances::DanceType;
    use holons_recovery::TransactionRecoveryStore;
    use holons_recovery::RecoveryStore;
    use std::{path::Path, sync::Arc};


    /// Helper: open an in-memory recovery store (`:memory:` is a rusqlite built-in).
    fn in_memory_store() -> Arc<TransactionRecoveryStore> {
        Arc::new(TransactionRecoveryStore::new(Path::new(":memory:")).expect("in-memory store"))
    }
    /// Exercises the full session recovery API:
    ///   persist (undoable) → undo → redo → list_undo_history → recover_last_snapshot → cleanup
    /// Also verifies that `disable_undo = true` does NOT add an entry to the undo stack.
    #[tokio::test]
    async fn session_recovery_api_full_flow() {
        let store = in_memory_store();
        let session = init_client_context(None, Some(store));

        // ── 1. Persist a normal, undoable command ──────────────────────────
        session.persist("cmd-1", false).await;

        let history = session.list_undo_history().await;
        assert_eq!(history.len(), 1, "one undoable entry after first persist");

        let last = session.recover_last_snapshot();
        assert!(last.is_some(), "recover_last_snapshot should return Some after persist");

        // ── 2. Undo brings stack to empty ─────────────────────────────────
        let undone = session.undo().await;
        assert!(
            undone.is_none(),
            "undo should return None when no prior checkpoint exists"
        );

        let history_after_undo = session.list_undo_history().await;
        assert_eq!(history_after_undo.len(), 0, "undo stack should be empty after undoing the only entry");

        // ── 3. Redo restores the entry ────────────────────────────────────
        let redone = session.redo().await;
        assert!(redone.is_some(), "redo should return the snapshot that was redone");

        let history_after_redo = session.list_undo_history().await;
        assert_eq!(history_after_redo.len(), 1, "undo stack should be restored after redo");

        // ── 4. persist with disable_undo = true ───────────────────────────
        // A bulk/loader-style command: stored for crash recovery but NOT added
        // to the undo stack, so the stack length should not change.
        session.persist("bulk-op (no-undo)", true).await;

        let history_after_no_undo = session.list_undo_history().await;
        assert_eq!(
            history_after_no_undo.len(), 1,
            "disable_undo persist must not grow the undo stack"
        );

        // Undo returns None when popping the last undoable entry (baseline).
        let undone2 = session.undo().await;
        assert!(
            undone2.is_none(),
            "undo should return None when no prior checkpoint exists"
        );

        // After popping that entry the stack is empty again.
        let undo_empty = session.undo().await;
        assert!(undo_empty.is_none(), "undo on an empty stack should return None");

        // ── 5. cleanup ────────────────────────────────────────────────────
        session.cleanup().await;
        // After cleanup the store's session is gone — recover_last_snapshot returns None.
        let after_cleanup = session.recover_last_snapshot();
        assert!(after_cleanup.is_none(), "recover_last_snapshot should return None after cleanup");
    }


    #[test]
    fn commit_route_classification_is_exact() {
        assert!(HolochainReceptor::is_commit_dance_request("commit"));
        assert!(!HolochainReceptor::is_commit_dance_request("get_all_holons"));
        assert!(!HolochainReceptor::is_commit_dance_request("load_holons"));
    }

    #[test]
    fn read_only_route_classification_includes_supported_reads() {
        assert!(HolochainReceptor::is_read_only_request("get_all_holons"));
        assert!(HolochainReceptor::is_read_only_request("get_holon_by_id"));
        assert!(HolochainReceptor::is_read_only_request("query_relationships"));
    }

    #[test]
    fn read_only_route_classification_excludes_mutations() {
        assert!(!HolochainReceptor::is_read_only_request("commit"));
        assert!(!HolochainReceptor::is_read_only_request("create_new_holon"));
        assert!(!HolochainReceptor::is_read_only_request("stage_new_holon"));
        assert!(!HolochainReceptor::is_read_only_request("load_holons"));
    }

    #[test]
    fn host_mutation_precheck_blocks_create_new_holon_before_builder_side_effects() {
        let session = init_client_context(None,None);
        let _guard = session.context.begin_host_commit_ingress_guard().expect("guard should acquire");

        let before = session.context.lookup().transient_count().expect("count should succeed");

        let mut props = PropertyMap::new();
        props.insert(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(MapString("PRECHECK_BLOCK".to_string())),
        );

        let request = MapRequest {
            name: "create_new_holon".to_string(),
            req_type: DanceType::Standalone,
            body: MapRequestBody::ParameterValues(props),
            space: HolonSpace::default(),
        };

        let is_read_only = HolochainReceptor::is_read_only_request(request.name.as_str());
        assert!(!is_read_only);

        // New receptor ordering: precheck before request build.
        let err = session.context
            .ensure_host_mutation_entry_allowed()
            .expect_err("host mutation precheck should reject during commit ingress");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("TransactionCommitInProgress"),
            "expected TransactionCommitInProgress, got {msg}"
        );

        // Ensure request builder was not run and no transient was created as a side effect.
        let after = session.context.lookup().transient_count().expect("count should succeed");
        assert_eq!(before, after, "transient pool must remain unchanged");

        // Sanity: builder remains side-effecting for create_new_holon if called directly.
        let _ = ClientDanceBuilder::validate_and_execute(&session.context, &request);
        let after_builder = session.context.lookup().transient_count().expect("count should succeed");
        assert!(
            after_builder > after,
            "direct builder call should still create transient side effect"
        );
    }
}
