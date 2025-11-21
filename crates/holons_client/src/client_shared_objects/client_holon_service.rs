#![allow(unused_variables)]

use base_types::{MapString};
use core_types::{HolonError, HolonId};
use holons_core::{ReadableHolon};
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::TransientReference;
use holons_core::{
    core_shared_objects::{Holon, HolonCollection},
    reference_layer::{HolonServiceApi, HolonsContextBehavior},
    HolonReference, RelationshipMap, SmartReference, StagedReference,
};
use integrity_core_types::{LocalId, RelationshipName};
use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use futures_executor::block_on;
// use tokio::runtime::Handle;
// use tokio::runtime::Builder;


#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        // 1. Build commit dance request
        let request = holon_dance_builders::build_commit_dance_request()?;

        // 2. Run the dance
        let initiator = context.get_space_manager().get_dance_initiator()?;
        let response = run_future_synchronously(initiator.initiate_dance(context, request));

        // 3. Any non-OK status is an error
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "Commit dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }
        // 4. Extract HolonReference → TransientReference
        match response.body {
            ResponseBody::HolonReference(href) => {
                match href {
                    HolonReference::Transient(tref) => Ok(tref),

                    // Wrong kind of reference ⇒ failure in commit pipeline
                    other => Err(HolonError::CommitFailure(format!(
                        "Expected TransientReference but received {:?}",
                        other
                    ))),
                }
            }

            // Wrong response type entirely ⇒ core bug, not caller error
            body => Err(HolonError::CommitFailure(format!(
                "Commit returned unexpected ResponseBody: {:?}; expected HolonReference",
                body
            ))),
        }
    }


    fn delete_holon_internal(&self, local_id: &LocalId) -> Result<(), HolonError> {
        //let request = holon_dance_builders::build_delete_holon_dance_request(*local_id)?;
        //let initiator = context.get_space_manager().get_dance_initiator()?;
        //let response = run_future_synchronously(initiator.initiate_dance(context, request))?;
        // no context.. not sure what to do here
        todo!()
    }

    fn fetch_all_related_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        //let request = holon_dance_builders::=((*source_id)?)?;
        //let initiator = context.get_space_manager().get_dance_initiator()?;
        //let response = run_future_synchronously(initiator.initiate_dance(context, request))?;
        //not sure how to do this one? 

        todo!()
    }

    fn fetch_holon_internal(&self, _id: &HolonId) -> Result<Holon, HolonError> {
     // no context.. not sure what to do here
        todo!()
    }

    fn fetch_related_holons_internal(
        &self,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        // no context.. not sure what to do here
        todo!()
    }

    fn get_all_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError> {
        let request = holon_dance_builders::build_get_all_holons_dance_request()?;
        let initiator = context.get_space_manager().get_dance_initiator()?;
        let response = run_future_synchronously(initiator.initiate_dance(context, request));
        if response.status_code != ResponseStatusCode::OK {
             return Err(HolonError::Misc(format!(
                 "commit dance failed: {:?} — {}",
                 response.status_code, response.description.0
             )));
         }
        match response.body {
            ResponseBody::HolonCollection(collection) => Ok(collection),
            _ => Err(HolonError::InvalidParameter(
                "GetAllHolons: expected ResponseBody::HolonCollection".into(),
            )),
        }

    }

    fn load_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        // // 1) Build the dance request for the loader.
        let request = holon_dance_builders::build_load_holons_dance_request(bundle)?;
        //
        // // 2) Get the DanceInitiator from the Space Manager.
        let initiator = context.get_space_manager().get_dance_initiator()?; // <- no .read()
        //
        // // 3) Bridge async → sync (keep this because the client service is sync)
         let response = run_future_synchronously(initiator.initiate_dance(context, request));
        //
        // // 4) Check the status
         if response.status_code != ResponseStatusCode::OK {
             return Err(HolonError::Misc(format!(
                 "LoadHolons dance failed: {:?} — {}",
                 response.status_code, response.description.0
             )));
         }
        
        // 5) Extract the returned holon
         match response.body {
             ResponseBody::HolonReference(HolonReference::Transient(t)) => Ok(t),
             ResponseBody::HolonReference(other_ref) => other_ref.clone_holon(context),
             _ => Err(HolonError::InvalidParameter(
                 "LoadHolons: expected ResponseBody::HolonReference".into(),
             )),
         }
        //todo!()
    }
}

/// Run an async computation to completion from a synchronous context.
///
/// This helper deliberately avoids Tokio runtime APIs so that
/// `holons_client` does not depend on Tokio's internal runtime
/// configuration (which Holochain controls).
///
/// It is intended to be called from top-level, synchronous code paths
/// (e.g., UI handlers), not from within an async Tokio task.
fn run_future_synchronously<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    block_on(future)
}

// /// Run an async future to completion from synchronous code (native only).
// ///
// /// Behavior:
// /// - If a Tokio runtime is already running on this thread, the future is executed
// ///   inside that runtime using `block_in_place` to avoid creating a nested runtime.
// /// - If no runtime is running, a lightweight current-thread runtime is created
// ///   just for this call.
// ///
// ///
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
