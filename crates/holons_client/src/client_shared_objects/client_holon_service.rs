#![allow(unused_variables)]

use base_types::MapString;
use core_types::{HolonError, HolonId};
use holon_dance_builders;
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
// use tokio::runtime::{Builder, Handle};

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
    ) -> Result<TransientReference, HolonError> {
        // // 1) Build the dance request for the loader.
        // let request = holon_dance_builders::build_load_holons_dance_request(bundle)?;
        //
        // // 2) Get the DanceInitiator from the Space Manager.
        // let initiator = context.get_space_manager().get_dance_initiator()?; // <- no .read()
        //
        // // 3) Bridge async → sync (keep this because the client service is sync)
        // let response = run_future_synchronously(initiator.initiate_dance(context, request))?;
        //
        // // 4) Check the status
        // if response.status_code != ResponseStatusCode::OK {
        //     return Err(HolonError::Misc(format!(
        //         "LoadHolons dance failed: {:?} — {}",
        //         response.status_code, response.description.0
        //     )));
        // }
        //
        // // 5) Extract the returned holon
        // match response.body {
        //     ResponseBody::HolonReference(HolonReference::Transient(t)) => Ok(t),
        //     ResponseBody::HolonReference(other_ref) => other_ref.clone_holon(context),
        //     _ => Err(HolonError::InvalidParameter(
        //         "LoadHolons: expected ResponseBody::HolonReference".into(),
        //     )),
        // }
        todo!()
    }

    fn new_holon_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        key: Option<MapString>,
    ) -> Result<TransientReference, HolonError> {
        let transient_service = context.get_space_manager().get_transient_behavior_service();
        let borrowed_service = transient_service
            .write()
            .map_err(|_| HolonError::FailedToBorrow("Transient service write".into()))?;

        // Create empty holon with or without key
        match key {
            Some(key_string) => borrowed_service.create_empty(key_string),
            None => borrowed_service.create_empty_without_key(),
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

// /// Run an async future to completion from synchronous code (native only).
// ///
// /// Behavior:
// /// - If a Tokio runtime is already running on this thread, the future is executed
// ///   inside that runtime using `block_in_place` to avoid creating a nested runtime.
// /// - If no runtime is running, a lightweight current-thread runtime is created
// ///   just for this call.
// ///
// /// This lets synchronous service methods (e.g. client-side HolonServiceApi)
// /// call async dance initiation code without panicking.
// pub fn run_future_synchronously<FutureType, OutputType>(
//     future_to_run: FutureType,
// ) -> Result<OutputType, HolonError>
// where
//     FutureType: core::future::Future<Output = OutputType>,
// {
//     // Reuse an existing Tokio runtime if we are already inside one.
//     if Handle::try_current().is_ok() {
//         let output_value =
//             tokio::task::block_in_place(|| Handle::current().block_on(future_to_run));
//         return Ok(output_value);
//     }
//
//     // Otherwise, create a small current-thread runtime for this one call.
//     let runtime = Builder::new_current_thread()
//         .enable_all()
//         .build()
//         .map_err(|error| {
//             HolonError::Misc(format!(
//                 "run_future_synchronously: failed to build Tokio runtime: {error}"
//             ))
//         })?;
//
//     Ok(runtime.block_on(future_to_run))
// }
