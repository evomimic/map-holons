//! ClientHolonService
//! ------------------
//! This module provides the *client-side* implementation of the `HolonServiceApi`.
//!
//! The guest-side (`holons_guest`) implementation performs persistence directly on the
//! Holochain DHT. The client-side version cannot do so; instead, *every operation that
//! interacts with saved holons must be executed as an asynchronous Dance* via the
//! DanceInitiator.
//!
//! Because `HolonServiceApi` is intentionally **synchronous** (shared by both guest and
//! client builds), this file implements the **sync → async → sync bridge** for all
//! dance-based operations:
//!
//!   1. A synchronous API function (e.g., `commit_internal`, `load_holons_internal`)
//!      constructs a dance request.
//!   2. It invokes the DanceInitiator's async `initiate_dance(...)` method.
//!   3. The returned `Future` is executed to completion using
//!      `run_future_synchronously`, ensuring that callers never need to be async.
//!   4. The resulting `DanceResponse` is interpreted and converted back into a
//!      `HolonReference`, `HolonCollection`, or a `HolonError`.
//!
//! This makes the client holon service a **pure request/response layer**: it never touches
//! persistence, never owns a runtime, and remains compatible with synchronous application
//! environments (Tauri, desktop/native, CLI tools, etc.).
//!
//! Key architectural notes:
//!   • This module must *not* assume the presence of a Tokio runtime.
//!   • The async runtime used to execute dances is entirely owned by the
//!     DanceInitiator / hosting layer (e.g., Conductora).
//!   • All concurrency concerns are isolated inside the dance initiation logic.
//!
//! As such, `ClientHolonService` is the canonical boundary between synchronous client
//! code and the asynchronous, choreography-driven MAP backend.

#![allow(unused_variables)]

use core_types::{HolonError, HolonId};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::reference_layer::TransientReference;
use holons_core::{
    core_shared_objects::{Holon, HolonCollection},
    reference_layer::{HolonServiceApi, HolonsContextBehavior},
    HolonReference, RelationshipMap,
};
use integrity_core_types::{LocalId, RelationshipName};
use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use tokio::runtime::Handle;

#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &TransactionContext,
    ) -> Result<TransientReference, HolonError> {
        // 1. Build commit dance request
        let request = holon_dance_builders::build_commit_dance_request()?;

        // 2. Reacquire Arc<TransactionContext> for initiator
        let context_arc = reacquire_transaction_context_arc(context)?;

        // 3. Run the dance (sync → async → sync)
        let initiator = context.get_dance_initiator()?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(Arc::clone(&context_arc), request).await
        });

        // 4. Any non-OK status is an error
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "Commit dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        // 5. Extract HolonReference → TransientReference
        match response.body {
            ResponseBody::HolonReference(HolonReference::Transient(tref)) => Ok(tref),
            ResponseBody::HolonReference(other) => Err(HolonError::CommitFailure(format!(
                "Expected TransientReference but received {:?}",
                other
            ))),
            body => Err(HolonError::CommitFailure(format!(
                "Commit returned unexpected ResponseBody: {:?}; expected HolonReference",
                body
            ))),
        }
    }

    fn delete_holon_internal(&self, local_id: &LocalId) -> Result<(), HolonError> {
        //let request = holon_dance_builders::build_delete_holon_dance_request(*local_id)?;
        //let initiator = context.get_space_manager().get_dance_initiator()?;
        // let ctx: &(dyn HolonsContextBehavior + Send + Sync) = context;
        // let response = run_future_synchronously(initiator.initiate_dance(ctx, request));
        // no context.. not sure what to do here
        todo!()
    }

    fn fetch_all_related_holons_internal(
        &self,
        context: &TransactionContext,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        //let request = holon_dance_builders::=((*source_id)?)?;
        //let initiator = context.get_space_manager().get_dance_initiator()?;
        // let ctx: &(dyn HolonsContextBehavior + Send + Sync) = context;
        // let response = run_future_synchronously(initiator.initiate_dance(ctx, request));
        //not sure how to do this one?

        todo!()
    }

    fn fetch_holon_internal(&self, _id: &HolonId) -> Result<Holon, HolonError> {
        // no context.. not sure what to do here
        todo!()
    }

    fn fetch_related_holons_internal(
        &self,
        context: &TransactionContext,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        // no context.. not sure what to do here
        todo!()
    }

    fn get_all_holons_internal(
        &self,
        context: &TransactionContext,
    ) -> Result<HolonCollection, HolonError> {
        let request = holon_dance_builders::build_get_all_holons_dance_request()?;

        let context_arc = reacquire_transaction_context_arc(context)?;
        let initiator = context.get_dance_initiator()?;

        let response = run_future_synchronously(async move {
            initiator.initiate_dance(Arc::clone(&context_arc), request).await
        });

        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "GetAllHolons dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        match response.body {
            ResponseBody::HolonCollection(collection) => Ok(collection),
            other => Err(HolonError::InvalidParameter(format!(
                "GetAllHolons: expected ResponseBody::HolonCollection, got {:?}",
                other
            ))),
        }
    }

    fn load_holons_internal(
        &self,
        context: &TransactionContext,
        set: TransientReference, // HolonLoadSet type
    ) -> Result<TransientReference, HolonError> {
        // 1) Build the dance request for the loader.
        let request = holon_dance_builders::build_load_holons_dance_request(set)?;

        // 2) Get the DanceInitiator from the Space Manager.
        let initiator = context.get_dance_initiator()?;

        // 3) Bridge async → sync (ClientHolonService is synchronous)
        //
        // We must pass an `Arc<TransactionContext>` into the initiator so it can
        // apply envelope logic and initiate the Holochain IPC request.
        // Since this API is still `&TransactionContext` (transitional), reacquire the Arc.
        let context_arc = reacquire_transaction_context_arc(context)?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(Arc::clone(&context_arc), request).await
        });

        // 4) Check the status
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "LoadHolons dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        // 5) Extract the returned holon
        match response.body {
            ResponseBody::HolonReference(HolonReference::Transient(tref)) => Ok(tref),
            ResponseBody::HolonReference(other) => Err(HolonError::InvalidParameter(format!(
                "LoadHolons: expected TransientReference, got {:?}",
                other
            ))),
            other => Err(HolonError::InvalidParameter(format!(
                "LoadHolons: expected ResponseBody::HolonReference, got {:?}",
                other
            ))),
        }
    }
}

/// Run an async future to completion from synchronous code (native only).
/// Drive an async future to completion from synchronous host/client code, aware of Tokio.
///
/// Behavior:
/// - If a Tokio runtime is already running on this thread, the future is executed
///   inside that runtime using `block_in_place` to avoid creating a nested runtime.
/// - If no runtime is running, a lightweight current-thread runtime is created
///   just for this call.
///
/// This helper intentionally returns `T` (and will panic if runtime setup fails or the
/// future panics) to keep the surrounding sync APIs unchanged.
// pub fn run_future_synchronously<F, T>(future: F) -> T
// where
//     F: Future<Output = T>,
// {
//     // Choice: return T to avoid widening the sync API surface; if we want to propagate
//     // runtime setup errors instead of panicking, we can change the signature to
//     // -> Result<T, HolonError> later.
//
//     // If already inside a Tokio runtime, drive the future there without requiring 'static.
//     if Handle::try_current().is_ok() {
//         return block_in_place(|| block_on(future));
//     }
//
//     // Otherwise, create a small current-thread runtime for this call (futures_executor).
//     block_on(future)
// }
pub fn run_future_synchronously<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    // If we are already inside a Tokio runtime, use it.
    if let Ok(handle) = Handle::try_current() {
        return handle.block_on(future);
    }

    // Otherwise, create a small runtime for this call.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime for synchronous bridge")
        .block_on(future)
}

// Temporary helper
fn reacquire_transaction_context_arc(
    context: &TransactionContext,
) -> Result<Arc<TransactionContext>, HolonError> {
    context
        .space_manager()
        .get_transaction_manager()
        .get_transaction(&context.tx_id())?
        .ok_or_else(|| HolonError::ServiceNotAvailable("TransactionContext".into()))
}
