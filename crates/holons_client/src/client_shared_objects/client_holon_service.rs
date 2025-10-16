#![allow(unused_variables)]

use crate::{dances_client, ConductorDanceCaller, DanceCallService};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use holon_dance_builders;
use holon_dance_builders::build_load_holons_dance_request;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::TransientReference;
use holons_core::{
    core_shared_objects::{CommitResponse, Holon, HolonCollection},
    reference_layer::{HolonServiceApi, HolonsContextBehavior},
    HolonReference, ReadableHolon, RelationshipMap, SmartReference, StagedReference,
};
use integrity_core_types::{LocalId, RelationshipName};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::runtime::Builder;

#[derive(Debug, Clone)]
pub struct ClientHolonService<C: ConductorDanceCaller + Debug + 'static> {
    /// Temporary: injected directly until DanceCallService is managed by SpaceManager
    /// and available via `context.get_space_manager().get_dance_call_service()`.
    dance_call_service: Arc<DanceCallService<C>>,
}

impl<C: ConductorDanceCaller + Debug + 'static> ClientHolonService<C> {
    pub fn new(dance_call_service: Arc<DanceCallService<C>>) -> Self {
        Self { dance_call_service }
    }
}

impl<C: ConductorDanceCaller + Debug> HolonServiceApi for ClientHolonService<C>
where
    // temporary fix for injecting DanceCallService and making `as_any()` happy
    C: ConductorDanceCaller + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        _context: &dyn HolonsContextBehavior,
    ) -> Result<CommitResponse, HolonError> {
        //let request = build_commit_dance_request(&SessionState::default())?;
        // let response: DanceResponse = conductor.call(&cell.zome("dances"), "dance", valid_request).await;
        // _context.get_space_manager()
        todo!()
    }

    fn delete_holon_internal(&self, local_id: &LocalId) -> Result<(), HolonError> {
        todo!()
    }

    fn fetch_all_related_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        todo!()
    }

    fn fetch_holon_internal(&self, _id: &HolonId) -> Result<Holon, HolonError> {
        todo!()
    }

    fn fetch_related_holons_internal(
        &self,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        todo!()
    }

    fn get_all_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError> {
        todo!()
    }

    fn load_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        // 1) Build request
        let request = build_load_holons_dance_request(bundle)?;

        // 2) Keep it simple: create a small current-thread runtime and block
        //    (compatible with #[async_trait(?Send)]).
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed: {e}")))?;

        let response = runtime.block_on(self.dance_call_service.dance_call(context, request));

        // 3) Handle non-OK statuses
        match response.status_code {
            ResponseStatusCode::OK => { /* proceed */ }
            other => {
                // Map to a HolonError; refine if you later add a reverse mapping helper
                return Err(HolonError::Misc(format!(
                    "Dance call failed: {:?} — {}",
                    other, response.description.0
                )));
            }
        }

        // 4) Extract the reference and return a TransientReference
        match response.body {
            ResponseBody::HolonReference(holon_reference) => {
                // Can't verify type if the loader had an error before setting the DescribedBy
                // relationship for the response holon so we just clone to transient and return it.
                holon_reference.clone_holon(context)
            }
            _ => Err(HolonError::InvalidParameter(
                "Unexpected ResponseBody: expected HolonReference".into(),
            )),
        }
    }

    fn stage_new_from_clone_internal(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: HolonReference,
        _new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        todo!()
    }

    fn stage_new_version_internal(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        todo!()
    }
}
