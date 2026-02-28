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
use futures_executor::block_on;
use holons_core::core_shared_objects::transactions::{TransactionContext, TransactionContextHandle};
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::query_layer::{Node, NodeCollection, QueryExpression};
use holons_core::reference_layer::TransientReference;
use holons_core::{
    core_shared_objects::{Holon, HolonCollection},
    reference_layer::{HolonServiceApi, HolonsContextBehavior, SmartReference},
    HolonCollectionApi, HolonReference, RelationshipMap,
};
use integrity_core_types::{LocalId, RelationshipName};
use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::{Arc, RwLock};
use tokio::runtime::Handle;
use tokio::task::block_in_place;

#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<TransientReference, HolonError> {
        // Build commit dance request
        let request = holon_dance_builders::build_commit_dance_request()?;

        // Run the dance (sync → async → sync)
        let initiator = context.get_dance_initiator()?;

        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
        });

        // Any non-OK status is an error
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "Commit dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        // Extract HolonReference → TransientReference
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

    fn delete_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        local_id: &LocalId,
    ) -> Result<(), HolonError> {
        let request = holon_dance_builders::build_delete_holon_dance_request(local_id.clone())?;
        let initiator = context.get_dance_initiator()?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
        });

        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "DeleteHolon dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        match response.body {
            ResponseBody::None => Ok(()),
            other => Err(HolonError::InvalidParameter(format!(
                "DeleteHolon: expected ResponseBody::None, got {:?}",
                other
            ))),
        }
    }

    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let context_handle = TransactionContextHandle::new(Arc::clone(context));
        let source_reference =
            HolonReference::Smart(SmartReference::new_from_id(context_handle, source_id.clone()));
        let node_collection = NodeCollection {
            members: vec![Node::new(source_reference, None)],
            query_spec: None,
        };

        let request =
            holon_dance_builders::build_fetch_all_related_holons_dance_request(node_collection)?;
        let initiator = context.get_dance_initiator()?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
        });

        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "FetchAllRelatedHolons dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        match response.body {
            ResponseBody::NodeCollection(node_collection) => {
                let mut result = RelationshipMap::new_empty();

                for node in node_collection.members {
                    if let Some(relationships) = node.relationships {
                        for (relationship_name, related_nodes) in relationships.0 {
                            let references = related_nodes
                                .members
                                .iter()
                                .map(|related| related.source_holon.clone())
                                .collect::<Vec<_>>();

                            if let Some(existing_collection_arc) =
                                result.get_collection_for_relationship(&relationship_name)
                            {
                                let mut existing_collection =
                                    existing_collection_arc.write().map_err(|e| {
                                        HolonError::FailedToAcquireLock(format!(
                                            "Failed to acquire write lock on relationship collection: {}",
                                            e
                                        ))
                                    })?;
                                existing_collection.add_references(references)?;
                            } else {
                                let mut collection = HolonCollection::new_existing();
                                collection.add_references(references)?;
                                result.insert(
                                    relationship_name,
                                    Arc::new(RwLock::new(collection)),
                                );
                            }
                        }
                    }
                }

                Ok(result)
            }
            other => Err(HolonError::InvalidParameter(format!(
                "FetchAllRelatedHolons: expected ResponseBody::NodeCollection, got {:?}",
                other
            ))),
        }
    }

    fn fetch_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        let request = holon_dance_builders::build_get_holon_by_id_dance_request(id.clone())?;
        let initiator = context.get_dance_initiator()?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
        });

        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "GetHolonById dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        match response.body {
            ResponseBody::Holon(holon) => Ok(holon),
            other => Err(HolonError::InvalidParameter(format!(
                "GetHolonById: expected ResponseBody::Holon, got {:?}",
                other
            ))),
        }
    }

    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let context_handle = TransactionContextHandle::new(Arc::clone(context));
        let source_reference =
            HolonReference::Smart(SmartReference::new_from_id(context_handle, source_id.clone()));
        let node_collection = NodeCollection {
            members: vec![Node::new(source_reference, None)],
            query_spec: None,
        };
        let query = QueryExpression::new(relationship_name.clone());
        let request = holon_dance_builders::build_query_relationships_dance_request(
            node_collection,
            query,
        )?;
        let initiator = context.get_dance_initiator()?;
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
        });

        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "QueryRelationships dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        match response.body {
            ResponseBody::NodeCollection(node_collection) => {
                let mut result = HolonCollection::new_existing();
                for node in node_collection.members {
                    if let Some(relationships) = node.relationships {
                        if let Some(related_nodes) = relationships.0.get(relationship_name) {
                            let references = related_nodes
                                .members
                                .iter()
                                .map(|related| related.source_holon.clone())
                                .collect();
                            result.add_references(references)?;
                        }
                    }
                }
                Ok(result)
            }
            other => Err(HolonError::InvalidParameter(format!(
                "QueryRelationships: expected ResponseBody::NodeCollection, got {:?}",
                other
            ))),
        }
    }

    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        let request = holon_dance_builders::build_get_all_holons_dance_request()?;

        let initiator = context.get_dance_initiator()?;

        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
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
        context: &Arc<TransactionContext>,
        set: TransientReference, // HolonLoadSet type
    ) -> Result<TransientReference, HolonError> {
        // 1) Build the dance request for the loader.
        let request = holon_dance_builders::build_load_holons_dance_request(set)?;

        // 2) Get the DanceInitiator from the Space Manager.
        let initiator = context.get_dance_initiator()?;

        // 3) Bridge async → sync (ClientHolonService is synchronous)
        let response = run_future_synchronously(async move {
            initiator.initiate_dance(context, request).await
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

/// Drive an async future to completion from synchronous host/client code.
///
/// Behavior:
/// - If a Tokio runtime is already running on this thread, execute via
///   `block_in_place(|| futures_executor::block_on(...))` so we do not nest runtimes.
/// - If no runtime is running, execute directly with `futures_executor::block_on(...)`.
///
/// Note: `block_in_place` requires a Tokio multi-thread runtime. If this helper is
/// called from a current-thread Tokio runtime, Tokio will panic.
pub fn run_future_synchronously<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    // Choice: return T to avoid widening the sync API surface; if we want to propagate
    // runtime setup errors instead of panicking, we can change the signature to
    // -> Result<T, HolonError> later.

    // If already inside a Tokio runtime, drive the future there without requiring static.
    if Handle::try_current().is_ok() {
        return block_in_place(|| block_on(future));
    }

    // Otherwise, run directly with futures_executor (no Tokio runtime required).
    block_on(future)
}
