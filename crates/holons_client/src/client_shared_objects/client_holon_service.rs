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
use tokio::runtime::{Builder, Handle};

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
        dance_service: Option<&dyn DanceCallServiceApi>, // temporary parameter
    ) -> Result<TransientReference, HolonError> {
        // 1) Build the dance request for the loader.
        let request = build_load_holons_dance_request(bundle)?;

        // 2) Temporary: Require a dance caller on the client (loader executes on guest).
        let dance =
            dance_service.ok_or_else(|| HolonError::Misc("DanceCallService missing".into()))?;

        // ─────────────────────────────────────────────────────────────────────
        // 3) BRIDGE ASYNC → SYNC WITHOUT NESTED RUNTIMES
        //
        // If we're already *in* a Tokio runtime (e.g., #[tokio::test]), the helper
        // reuses it safely (no nested runtime). If no runtime exists, it creates a
        // small current-thread runtime just for this call. Works on native & WASM.
        // ─────────────────────────────────────────────────────────────────────
        let response = run_future_synchronously(dance.dance_call(context, request))?;

        // 4) Handle non-OK statuses (map to HolonError; refine mapping later if needed).
        match response.status_code {
            ResponseStatusCode::OK => { /* proceed */ }
            other => {
                return Err(HolonError::Misc(format!(
                    "Dance call failed: {:?} — {}",
                    other, response.description.0
                )));
            }
        }

        // 5) Extract the holon reference and return a TransientReference.
        match response.body {
            // Directly pass through the transient reference that has been restored from session state.
            ResponseBody::HolonReference(HolonReference::Transient(transient_reference)) => {
                Ok(transient_reference)
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
        dance_service: Option<&dyn DanceCallServiceApi>, // temporary parameter(unused in local-only implementation)
    ) -> Result<TransientReference, HolonError> {
        // // 1) Build request (None => RequestBody::None; Some(key) => RequestBody::ParameterValues)
        // let request = build_new_holon_dance_request(key);
        //
        // // Temporary: Require a dance caller on the client
        // let dance = dance_service.ok_or_else(|| HolonError::Misc("DanceCallService missing".into()))?;
        //
        // // 2) Simple current-thread runtime (compatible with #[async_trait(?Send)])
        // let runtime = Builder::new_current_thread()
        //     .enable_all()
        //     .build()
        //     .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed: {e}")))?;
        //
        // let response = runtime.block_on(dance.dance_call(context, request));
        //
        // // 3) Handle non-OK statuses (accept OK/Accepted for flexibility)
        // match response.status_code {
        //     ResponseStatusCode::OK | ResponseStatusCode::Accepted => {}
        //     other => {
        //         return Err(HolonError::Misc(format!(
        //             "Dance call failed: {:?} — {}",
        //             other, response.description.0
        //         )));
        //     }
        // }
        //
        // // 4) Extract the holon reference and return a TransientReference
        // match response.body {
        //     ResponseBody::HolonReference(href) => href.clone_holon(context),
        //     _ => Err(HolonError::InvalidParameter(
        //         "Unexpected ResponseBody: expected HolonReference".into(),
        //     )),
        // }

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

/// Run an async future to completion from synchronous code,
/// working in both "no runtime yet" and "already inside a Tokio runtime" cases,
/// and compiling cleanly for native and WASM targets.
///
/// Behavior:
/// - If a Tokio runtime is already running (native): reuse it and **do not** spin up
///   a nested runtime (avoids the “Cannot start a runtime from within a runtime” panic).
/// - If no runtime is present (native): create a small current-thread runtime and block.
/// - On WASM: always create a small current-thread runtime and block (Tokio’s multi-thread
///   & `block_in_place` are not available on wasm32).
///
/// Errors while building the temporary runtime are mapped to `HolonError`.
///
/// Requirements (native):
/// - Tokio features must include at least `rt` (you already have this).
/// - If your toolchain says `tokio::task::block_in_place` is missing, ensure the dependency for
///   non-WASM targets enables the multi-thread scheduler (see Cargo.toml note below).

pub fn run_future_synchronously<F, R>(future_to_run: F) -> Result<R, HolonError>
where
    F: core::future::Future<Output = R>,
{
    // ------- Native (non-wasm32) path -------
    #[cfg(not(target_arch = "wasm32"))]
    {
        use tokio::runtime::{Builder, Handle};

        // If we are already inside a Tokio runtime, block the current thread in-place
        // and drive the provided future to completion on the existing runtime.
        if Handle::try_current().is_ok() {
            let result = tokio::task::block_in_place(|| Handle::current().block_on(future_to_run));
            return Ok(result);
        }

        // Otherwise, create a tiny current-thread runtime just for this call.
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed: {e}")))?;
        Ok(rt.block_on(future_to_run))
    }

    // ------- WASM path -------
    #[cfg(target_arch = "wasm32")]
    {
        use tokio::runtime::Builder;

        // On WASM there is no multi-thread scheduler or block_in_place; just build a
        // small current-thread runtime and block on the future.
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| HolonError::Misc(format!("Tokio runtime build failed (wasm): {e}")))?;
        Ok(rt.block_on(future_to_run))
    }
}
