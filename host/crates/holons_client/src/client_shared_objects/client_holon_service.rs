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
//! The client holon service also decides which fetched relationships can be cached. It never touches
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

use base_types::{MapBytes, MapString};
use core_types::{HolonError, HolonId};
use futures_executor::block_on;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{ResponseBody, ResponseStatusCode};
use holons_core::query_layer::{Node, NodeCollection, QueryExpression};
use holons_core::reference_layer::TransientReference;
use holons_core::{
    core_shared_objects::{Holon, HolonCollection},
    reference_layer::{HolonServiceApi, SmartReference},
    HolonCollectionApi, HolonReference, ReadableHolon, RelationshipCachePolicy, RelationshipMap,
    StagedReference,
};
use integrity_core_types::{LocalId, RelationshipName};
use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::{atomic::Ordering, Arc, RwLock};
use std::time::Instant;
use tokio::runtime::Handle;
use tokio::task::block_in_place;
use tracing::info;
use type_names::{CoreRelationshipTypeName, ToRelationshipName};

use crate::dahn::{DahnMaterializer, DancerPackageCatalog};

/// Client services and schema resolutions belong to one running holon space.
/// Clones share that lifetime; construct a new service for another space.
#[derive(Debug, Clone)]
pub struct ClientHolonService {
    dahn_materializer: DahnMaterializer,
    dancer_package_catalog: DancerPackageCatalog,
    prepared_generation: Arc<std::sync::atomic::AtomicU64>,
    cache_clock_origin: Instant,
    descriptor_flights: Arc<(
        std::sync::Mutex<
            std::collections::HashSet<holons_core::core_shared_objects::transactions::TxId>,
        >,
        std::sync::Condvar,
    )>,
}

thread_local! { static PREPARATION_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
struct RelationshipSemanticsResolutionGuard;

impl Drop for RelationshipSemanticsResolutionGuard {
    fn drop(&mut self) {
        PREPARATION_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

fn foundational_relationship(name: &RelationshipName) -> bool {
    [
        CoreRelationshipTypeName::DescribedBy,
        CoreRelationshipTypeName::Extends,
        CoreRelationshipTypeName::InstanceRelationships,
    ]
    .iter()
    .any(|entry| entry.to_relationship_name() == *name)
}

impl ClientHolonService {
    fn acquire_descriptor(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::EffectiveHolonDescriptor, HolonError> {
        if !matches!(target, HolonReference::Smart(_)) {
            return self.construct_effective_holon_descriptor_internal(context, target);
        }
        let Some(registry) = context.descriptor_registry() else {
            return self.construct_effective_holon_descriptor_internal(context, target);
        };
        let session = context.space_read_handle().session()?;
        let target_id = target.holon_id()?;
        let owner = registry.context()?;
        let (flights, ready) = &*self.descriptor_flights;
        // Coordinate the carrier as well as the key: pool transport replaces the
        // entire carrier, so independent misses must not send it concurrently.
        let mut active =
            flights.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        loop {
            if let Some(descriptor) = registry.get(&target_id)? {
                return Ok(descriptor);
            }
            if active.insert(owner.tx_id()) {
                break;
            }
            active =
                ready.wait(active).map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        }
        drop(active);
        let result = (|| {
            let _preparation = self.enter_relationship_semantics_resolution();
            let descriptor = self.construct_effective_holon_descriptor_internal(&owner, target)?;
            registry.publish_in_session(target_id.clone(), &descriptor, &session)?;
            registry.get(&target_id)?.ok_or(HolonError::ObsoleteSession)
        })();
        let cleanup = registry.prune_envelopes();
        flights
            .lock()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .remove(&owner.tx_id());
        ready.notify_all();
        result.and_then(|descriptor| cleanup.map(|_| descriptor))
    }

    fn acquire_derived_descriptor(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::DerivedDescriptor, HolonError> {
        if !matches!(target, HolonReference::Smart(_)) {
            return self.construct_derived_descriptor_internal(context, target);
        }
        let Some(registry) = context.descriptor_registry() else {
            return self.construct_derived_descriptor_internal(context, target);
        };
        let session = context.space_read_handle().session()?;
        let target_id = target.holon_id()?;
        let owner = registry.context()?;
        let (flights, ready) = &*self.descriptor_flights;
        // Coordinate the carrier as well as the key: pool transport replaces the
        // entire carrier, so independent misses must not send it concurrently.
        let mut active =
            flights.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        loop {
            if let Some(descriptor) = registry.get_derived(&target_id)? {
                return Ok(descriptor);
            }
            if active.insert(owner.tx_id()) {
                break;
            }
            active =
                ready.wait(active).map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        }
        drop(active);
        let result = (|| {
            let _preparation = self.enter_relationship_semantics_resolution();
            let descriptor = self.construct_derived_descriptor_internal(&owner, target)?;
            registry.publish_derived(target_id, &descriptor, &session)
        })();
        let cleanup = registry.prune_envelopes();
        flights
            .lock()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .remove(&owner.tx_id());
        ready.notify_all();
        result.and_then(|descriptor| cleanup.map(|_| descriptor))
    }

    fn resolving_relationship_semantics(&self) -> bool {
        PREPARATION_DEPTH.with(|depth| depth.get() > 0)
    }

    fn enter_relationship_semantics_resolution(&self) -> RelationshipSemanticsResolutionGuard {
        PREPARATION_DEPTH.with(|depth| depth.set(depth.get() + 1));
        RelationshipSemanticsResolutionGuard
    }

    pub fn new(
        dahn_materializer: DahnMaterializer,
        dancer_package_catalog: DancerPackageCatalog,
    ) -> Self {
        Self {
            dahn_materializer,
            dancer_package_catalog,
            prepared_generation: Default::default(),
            cache_clock_origin: Instant::now(),
            descriptor_flights: Default::default(),
        }
    }

    /// One retrieval request carries both membership and its prepared schema association.
    fn fetch_relationship(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
        name: &RelationshipName,
        bind_descriptor: bool,
    ) -> Result<holons_core::core_shared_objects::relationship_fetch::FetchedRelationship, HolonError>
    {
        use holons_core::core_shared_objects::relationship_fetch::FetchedRelationship;
        let _profile =
            tracing::debug_span!(target: "map_profile", "fetch_relationship", relationship = %name)
                .entered();
        let source = HolonReference::smart_from_id(context.space_read_handle(), source_id.clone());
        let nodes = NodeCollection { members: vec![Node::new(source, None)], query_spec: None };
        let mut query = QueryExpression::new(name.clone());
        query.include_relationship_descriptor = bind_descriptor;
        let request = holon_dance_builders::build_query_relationships_dance_request(nodes, query)?;
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;
        if response.status_code != ResponseStatusCode::OK {
            return Err(response.error.unwrap_or_else(|| {
                HolonError::Misc(format!(
                    "QueryRelationships dance failed: {:?} — {}",
                    response.status_code, response.description.0
                ))
            }));
        }
        match response.body {
            ResponseBody::SavedRelationship(fetched) if bind_descriptor && fetched.descriptor.is_some() => Ok(fetched),
            ResponseBody::NodeCollection(nodes) if !bind_descriptor => {
                let mut collection = HolonCollection::new_existing();
                for node in nodes.members {
                    if let Some(relationships) = node.relationships {
                        if let Some(related) = relationships.0.get(name) {
                            collection.add_references(related.members.iter().map(|node| node.source_holon.clone()).collect())?;
                        }
                    }
                }
                Ok(FetchedRelationship { collection, descriptor: None })
            }
            other => Err(HolonError::InvalidParameter(format!(
                "QueryRelationships: expected descriptor-associated membership={bind_descriptor}, got {other:?}"
            ))),
        }
    }

    pub fn development_default() -> Self {
        Self::new(
            DahnMaterializer::development_default(),
            DancerPackageCatalog::development_default(),
        )
    }
}

impl HolonServiceApi for ClientHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn relationship_cache_generation(&self) -> u64 {
        self.prepared_generation.load(Ordering::Acquire)
    }

    fn relationship_cache_time_millis(&self) -> Option<u64> {
        u64::try_from(self.cache_clock_origin.elapsed().as_millis()).ok()
    }

    fn relationship_cache_policy(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Option<RelationshipCachePolicy>, HolonError> {
        // Ordinary policies arrive with the fetched membership's exact descriptor.
        // This callback is deliberately incapable of discovering schema.
        Ok(foundational_relationship(relationship_name).then_some(RelationshipCachePolicy::Reuse))
    }

    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        // Build commit dance request
        let request = holon_dance_builders::build_commit_dance_request()?;

        // Run the dance (sync → async → sync)
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

        // Any non-OK status is an error
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "Commit dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        // Extract HolonReference → TransientReference
        match response.body {
            ResponseBody::HolonReference(HolonReference::Transient(tref)) => {
                let status =
                    tref.property_value(type_names::CorePropertyTypeName::CommitRequestStatus)?;
                let persisted = !tref
                    .related_holons(type_names::CoreRelationshipTypeName::SavedHolons)?
                    .read()
                    .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                    .get_members()
                    .is_empty();
                // Incomplete can already have persisted nodes/edges. Conservatively
                // retire its adopted view as well; never treat it like rejection.
                if persisted
                    || matches!(status, Some(base_types::BaseValue::StringValue(ref value)) if value.0 == "Incomplete" || (value.0 == "Complete" && !staged_references.is_empty()))
                {
                    context
                        .space_read_handle()
                        .reset_session()
                        .map_err(|error| HolonError::CacheResetAfterCommit(error.to_string()))?;
                }
                Ok(tref)
            }
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
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

        if response.status_code != ResponseStatusCode::OK {
            return Err(map_dance_error("DeleteHolon", &response));
        }

        match response.body {
            ResponseBody::None => context
                .space_read_handle()
                .reset_session()
                .map_err(|error| HolonError::CacheResetAfterCommit(error.to_string())),
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
        let source_reference = HolonReference::Smart(SmartReference::new_from_id(
            context.space_read_handle(),
            source_id.clone(),
        ));
        let node_collection =
            NodeCollection { members: vec![Node::new(source_reference, None)], query_spec: None };

        let request =
            holon_dance_builders::build_fetch_all_related_holons_dance_request(node_collection)?;
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

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
                                result.insert(relationship_name, Arc::new(RwLock::new(collection)));
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

    fn fetch_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        ids: &[HolonId],
    ) -> Result<Vec<holons_core::core_shared_objects::cache_population::SavedHolonFetch>, HolonError>
    {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let request = holon_dance_builders::build_get_holons_by_ids_dance_request(ids.to_vec());
        let started = std::time::Instant::now();
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await });
        tracing::info!(target: "map_profile", requested = ids.len(), guest_invocations = 1, elapsed_us = started.elapsed().as_micros(), "bulk saved hydration");
        let response = response?;
        if response.status_code != ResponseStatusCode::OK {
            return Err(response.error.unwrap_or_else(|| {
                HolonError::Misc(format!(
                    "GetHolonsByIds dance failed: {:?} — {}",
                    response.status_code, response.description.0
                ))
            }));
        }
        match response.body {
            ResponseBody::SavedHolonHydration(entries) => Ok(entries),
            _ => Err(HolonError::InvalidParameter("Expected SavedHolonHydration".into())),
        }
    }

    fn fetch_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        let request = holon_dance_builders::build_get_holon_by_id_dance_request(id.clone())?;
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

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
        Ok(self.fetch_relationship(context, source_id, relationship_name, false)?.collection)
    }

    fn fetch_related_holons_with_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<holons_core::core_shared_objects::relationship_fetch::FetchedRelationship, HolonError>
    {
        let bind_descriptor =
            !foundational_relationship(name) && !self.resolving_relationship_semantics();
        self.fetch_relationship(context, source, name, bind_descriptor)
    }

    fn fetch_relationship_set_with_descriptors_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        names: &[RelationshipName],
        unresolved: &[RelationshipName],
    ) -> Result<
        Vec<(
            RelationshipName,
            holons_core::core_shared_objects::relationship_fetch::FetchedRelationship,
        )>,
        HolonError,
    > {
        names
            .iter()
            .map(|name| {
                let fetched = if unresolved.contains(name) {
                    self.fetch_related_holons_with_descriptor_internal(context, source, name)?
                } else {
                    self.fetch_relationship(context, source, name, false)?
                };
                Ok((name.clone(), fetched))
            })
            .collect()
    }

    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        let request = holon_dance_builders::build_get_all_holons_dance_request()?;

        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

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

    fn visualizer_artifact_available_internal(
        &self,
        _context: &Arc<TransactionContext>,
        visualizer: &HolonReference,
    ) -> Result<bool, HolonError> {
        self.dahn_materializer.is_available(visualizer)
    }

    fn resolve_visualizer_usage_internal(
        &self,
        context: &Arc<TransactionContext>,
        visualizer: &HolonReference,
        subject_type: &HolonReference,
        slot: &HolonReference,
    ) -> Result<holons_core::dances::VisualizerUsageResolution, HolonError> {
        let request = holon_dance_builders::build_resolve_visualizer_usage_dance_request(
            visualizer.clone(),
            subject_type.clone(),
            slot.clone(),
        )?;
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;
        if response.status_code != ResponseStatusCode::OK {
            return Err(response.error.unwrap_or_else(|| {
                HolonError::Misc(format!(
                    "ResolveVisualizerUsage failed: {:?} — {}",
                    response.status_code, response.description.0
                ))
            }));
        }
        match response.body {
            ResponseBody::HolonReference(body) => {
                let resolved =
                    holons_core::dances::VisualizerUsageResolution::from_projection(&body)?;
                if resolved.search_path.first() != Some(subject_type) {
                    return Err(HolonError::InvalidState(
                        "Usage resolution returned a different starting descriptor".into(),
                    ));
                }
                Ok(resolved)
            }
            other => Err(HolonError::InvalidParameter(format!(
                "ResolveVisualizerUsage expected a projection, got {other:?}"
            ))),
        }
    }

    fn materialize_visualizer_internal(
        &self,
        context: &Arc<TransactionContext>,
        visualizer: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        self.dahn_materializer.materialize(context, visualizer)
    }

    fn fetch_artifact_internal(
        &self,
        context: &Arc<TransactionContext>,
        handle: &MapString,
    ) -> Result<MapBytes, HolonError> {
        self.dahn_materializer.fetch_artifact(context, handle)
    }

    fn activate_dancer_internal(
        &self,
        context: &Arc<TransactionContext>,
        package_identity: &MapString,
    ) -> Result<(), HolonError> {
        self.dancer_package_catalog.activate(context, package_identity)
    }

    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<SmartReference, HolonError> {
        let request =
            holon_dance_builders::build_get_saved_holon_by_key_dance_request(key.clone())?;
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;

        if response.status_code != ResponseStatusCode::OK {
            if let Some(error) = response.error {
                return Err(error);
            }
            return Err(map_dance_error("GetSavedHolonByKey", &response));
        }

        match response.body {
            ResponseBody::HolonReference(HolonReference::Smart(reference)) => Ok(reference),
            other => Err(HolonError::InvalidParameter(format!(
                "GetSavedHolonByKey: expected SmartReference, got {:?}",
                other
            ))),
        }
    }

    fn acquire_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::EffectiveHolonDescriptor, HolonError> {
        self.acquire_descriptor(context, target)
    }

    fn pull_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::EffectiveHolonDescriptor, HolonError> {
        context.space_read_handle().reset_session()?;
        self.acquire_descriptor(context, target)
    }

    fn construct_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::EffectiveHolonDescriptor, HolonError> {
        let invocation =
            holons_core::dances::implementations::construct_effective_holon_descriptor::invocation(
                context, target,
            )?;
        let response =
            run_future_synchronously(async move { context.initiate_invocation(invocation).await })?;
        let response = holons_core::dances::DanceResponseReference::new(response)?;
        Ok(holons_core::descriptors::EffectiveHolonDescriptor::from_constructed_holon(
            response.require_response_body()?,
        ))
    }

    fn acquire_derived_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::DerivedDescriptor, HolonError> {
        self.acquire_derived_descriptor(context, target)
    }

    fn construct_derived_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<holons_core::descriptors::DerivedDescriptor, HolonError> {
        let invocation =
            holons_core::dances::implementations::construct_derived_descriptor::invocation(
                context, target,
            )?;
        let response =
            run_future_synchronously(async move { context.initiate_invocation(invocation).await })?;
        let response = holons_core::dances::DanceResponseReference::new(response)?;
        Ok(holons_core::descriptors::DerivedDescriptor::from_constructed_holon(
            response.require_response_body()?,
        ))
    }

    fn load_theme_internal(
        &self,
        context: &Arc<TransactionContext>,
        theme: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        let started = Instant::now();
        let invocation =
            holons_core::dances::implementations::load_theme::invocation(context, theme.clone())?;
        let setup_ms = started.elapsed().as_secs_f64() * 1000.0;
        let response =
            run_future_synchronously(async move { context.initiate_invocation(invocation).await })?;
        let execution_ms = started.elapsed().as_secs_f64() * 1000.0 - setup_ms;
        // Canonical execution validated the response contract in the guest. Read
        // only its transported transient graph here; revalidating descriptor ancestry
        // through DanceResponseReference would introduce additional source graph reads.
        if !matches!(response, HolonReference::Transient(_)) {
            return Err(HolonError::InvalidParameter(
                "LoadTheme response must be transient".into(),
            ));
        }
        let bodies = response
            .related_holons(type_names::CoreRelationshipTypeName::ResponseBody)?
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        if std::env::var_os("MAP_PROFILE").is_some() {
            tracing::info!(target: "map_profile", setup_ms, execution_ms, "LoadTheme canonical guest loading");
        }
        match bodies.as_slice() {
            [body] => Ok(body.clone()),
            _ => Err(HolonError::InvalidParameter("LoadTheme requires one response body".into())),
        }
    }

    fn invoke_load_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        invocation: holons_core::dances::DanceInvocation,
    ) -> Result<holons_core::dances::DanceResponseReference, HolonError> {
        let _guard = context.begin_host_commit_ingress_guard()?;
        let response =
            run_future_synchronously(async move { context.initiate_invocation(invocation).await })?;
        // Descriptor reads may perform synchronous remote lookups. Validate after the
        // transport future has completed so those reads cannot nest its executor.
        let response = holons_core::dances::DanceResponseReference::new(response)?;
        let reference: HolonReference = response.clone().into();
        reset_after_load(context, &reference)?;
        Ok(response)
    }

    fn load_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        set: TransientReference, // HolonLoadSet type
    ) -> Result<TransientReference, HolonError> {
        // 1) Build the dance request for the loader.
        let request_started_at = Instant::now();
        let request = holon_dance_builders::build_load_holons_dance_request(set)?;
        let request_millis = request_started_at.elapsed().as_millis();

        // 2) Bridge async → sync (ClientHolonService is synchronous)
        let dance_started_at = Instant::now();
        let response =
            run_future_synchronously(async move { context.initiate_dance(request).await })?;
        let dance_millis = dance_started_at.elapsed().as_millis();

        // 3) Check the status
        if response.status_code != ResponseStatusCode::OK {
            return Err(HolonError::Misc(format!(
                "LoadHolons dance failed: {:?} — {}",
                response.status_code, response.description.0
            )));
        }

        // 4) Extract the returned holon
        match response.body {
            ResponseBody::HolonReference(HolonReference::Transient(tref)) => {
                info!(
                    "[PERF-688] loader_dance_client: request_build_ms={} initiate_dance_round_trip_ms={}",
                    request_millis,
                    dance_millis,
                );
                reset_after_load(context, &tref.clone().into())?;
                Ok(tref)
            }
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

/// Maps a non-OK `DanceResponse` to a structured `HolonError`, preserving the
/// error kind where a 1-to-1 `ResponseStatusCode` → `HolonError` variant exists.
///
/// Only `NotFound` and `NotImplemented` have unambiguous reverse mappings:
///   - `NotFound`       → `HolonError::HolonNotFound`
///   - `NotImplemented` → `HolonError::NotImplemented`
///
/// All other status codes (`BadRequest`, `Conflict`, `ServerError`, etc.) are
/// many-to-one: multiple `HolonError` variants collapse into the same status
/// code on the guest side (see `From<HolonError> for ResponseStatusCode`), so
/// there is no reliable way to reconstruct the original variant. These fall
/// through to `HolonError::Misc` until the dance bridge is replaced by native
/// command dispatch.
/// Loading performs guest-side Commits; remote observations alone never invoke this hook.
fn reset_after_load(
    context: &Arc<TransactionContext>,
    response: &HolonReference,
) -> Result<(), HolonError> {
    let status = response.property_value(type_names::CorePropertyTypeName::LoadCommitStatus)?;
    if matches!(status, Some(base_types::BaseValue::StringValue(ref value)) if value.0 == "Complete" || value.0 == "Incomplete")
    {
        context
            .space_read_handle()
            .reset_session()
            .map_err(|error| HolonError::CacheResetAfterCommit(error.to_string()))?;
    }
    Ok(())
}

fn map_dance_error(operation: &str, response: &holons_core::dances::DanceResponse) -> HolonError {
    let description = &response.description.0;
    match response.status_code {
        ResponseStatusCode::NotFound => {
            HolonError::HolonNotFound(format!("{operation}: {description}"))
        }
        ResponseStatusCode::NotImplemented => {
            HolonError::NotImplemented(format!("{operation}: {description}"))
        }
        _ => HolonError::Misc(format!(
            "{operation} dance failed: {:?} — {description}",
            response.status_code
        )),
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

#[cfg(test)]
mod tests;
