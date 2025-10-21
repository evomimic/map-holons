#![allow(unused_variables)]

use crate::{dances_client, ConductorDanceCaller, DanceCallService};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use holon_dance_builders;
use holon_dance_builders::{build_load_holons_dance_request, build_new_holon_dance_request};
use holons_core::dances::DanceCallServiceApi;
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
use tokio::runtime::Builder;

#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
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
        dance: Option<&dyn DanceCallServiceApi>, // temporary parameter
    ) -> Result<TransientReference, HolonError> {
        // Build request
        let request = build_load_holons_dance_request(bundle)?;

        // Temporary: Require a dance caller on the client
        let dance = dance.ok_or_else(|| HolonError::Misc("DanceCallService missing".into()))?;

        // Bridge async → sync with a small current-thread runtime
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed: {e}")))?;

        let response = runtime.block_on(dance.dance_call(context, request));

        // Handle non-OK statuses
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

        // Extract the reference and return a TransientReference
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

    fn new_holon_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        key: Option<MapString>,
        dance: Option<&dyn DanceCallServiceApi>, // temporary parameter
    ) -> Result<TransientReference, HolonError> {
        // 1) Build request (None => RequestBody::None; Some(key) => RequestBody::ParameterValues)
        let request = build_new_holon_dance_request(key);

        // Temporary: Require a dance caller on the client
        let dance = dance.ok_or_else(|| HolonError::Misc("DanceCallService missing".into()))?;

        // 2) Simple current-thread runtime (compatible with #[async_trait(?Send)])
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed: {e}")))?;

        let response = runtime.block_on(dance.dance_call(context, request));

        // 3) Handle non-OK statuses (accept OK/Accepted for flexibility)
        match response.status_code {
            ResponseStatusCode::OK | ResponseStatusCode::Accepted => {}
            other => {
                return Err(HolonError::Misc(format!(
                    "Dance call failed: {:?} — {}",
                    other, response.description.0
                )));
            }
        }

        // 4) Extract the holon reference and return a TransientReference
        match response.body {
            ResponseBody::HolonReference(href) => href.clone_holon(context),
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
