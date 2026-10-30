use super::{HolonReference, SmartReference, TransientReference};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{Holon, HolonCollection};
use crate::RelationshipMap;
use crate::StagedReference;
use base_types::{MapBytes, MapString};
use core_types::{HolonError, HolonId, KeyMatch, LocalId, RelationshipName, SmartLink};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// The HolonServiceApi trait defines the public service interface for Holon operations
/// in MAP. Its primary purpose is to provide a **shared abstraction** between client
/// and guest contexts while isolating differences in their implementations.
///
/// - `holons_core` depends only on this trait (never directly on client or guest), so it can be shared across client and guest.
/// - The **client implementation** typically builds a dance and delegates
///   to the guest.
/// - The **guest implementation** executes the dance by calling into the persistence
///   layer. This is where the "meat" of the operation resides, and it is the only
///   place where Holochain dependencies are imported.
/// - This indirection lets us avoid circular dependencies and keeps Holochain-specific
///   code out of `holons_core`.
///
/// In other words, this trait defines the "what" of Holon operations, while the
/// client and guest provide the "how" for their respective contexts.
pub trait HolonServiceApi: Debug + Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    /// Changes only on explicit adoption of a new prepared policy view.
    fn relationship_cache_generation(&self) -> u64 {
        0
    }

    /// Monotonic service-local milliseconds for bounded cache reuse. No clock
    /// means bounded entries cannot be retained. Guests need no OS clock.
    fn relationship_cache_time_millis(&self) -> Option<u64> {
        None
    }

    /// Resolves a saved relationship's retention policy, independently of membership expiry.
    /// `Some` is authoritative for the enclosing cache lifetime. `None` means
    /// unresolved: fetch fresh for this read and retry policy resolution later.
    /// Called after a fetch, with no cache lock held, only when no policy is retained.
    /// The default resolves declared policies. Request-local guests override it
    /// with unconditional reuse; clients also resolve inverse descriptor policies.
    fn relationship_cache_policy(
        &self,
        _context: &Arc<TransactionContext>,
        _source_holon_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<Option<crate::RelationshipCachePolicy>, HolonError> {
        Ok(None)
    }

    /// Prepares missing schema above the instance-cache policy seam. Guests need no preparation.
    fn prepare_relationship_policy_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source: &HolonId,
        _name: &RelationshipName,
    ) -> Result<(), HolonError> {
        Ok(())
    }

    /// Acquires a prepared HolonType projection. Hosts coordinate session misses;
    /// request-local guests use their already-shipped registry before constructing.
    fn acquire_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<crate::descriptors::EffectiveHolonDescriptor, HolonError> {
        if let Some(descriptor) =
            crate::descriptors::EffectiveHolonDescriptor::find_in_context(context, &target)?
        {
            return Ok(descriptor);
        }
        self.construct_effective_holon_descriptor_internal(context, target)
    }

    /// Explicit pull selects a new projection while previously returned handles remain immutable.
    fn pull_effective_holon_descriptor_internal(
        &self,
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<crate::descriptors::EffectiveHolonDescriptor, HolonError> {
        self.construct_effective_holon_descriptor_internal(context, target)
    }

    /// Constructs a retained schema graph. Clients delegate one typed Dance to
    /// the guest; guests invoke the shared construction algorithm directly.
    fn construct_effective_holon_descriptor_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _target: HolonReference,
    ) -> Result<crate::descriptors::EffectiveHolonDescriptor, HolonError> {
        Err(HolonError::NotImplemented("construct_effective_holon_descriptor_internal".into()))
    }

    /// This function commits the staged holons to the persistent store
    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError>;

    /// This function deletes the saved holon identified by  from the persistent store
    fn delete_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        local_id: &LocalId,
    ) -> Result<(), HolonError>;

    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError>;

    fn fetch_holon_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError>;

    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;

    /// Fetches complete memberships for exactly the requested names, including empty sets.
    /// Providers may consolidate transport without changing selection or freshness semantics.
    fn fetch_relationship_set_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        names: &[RelationshipName],
    ) -> Result<Vec<(RelationshipName, HolonCollection)>, HolonError> {
        names
            .iter()
            .map(|name| {
                Ok((name.clone(), self.fetch_related_holons_internal(context, source, name)?))
            })
            .collect()
    }

    /// Retrieves all persisted Holons, as a HolonCollection
    fn get_all_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError>;

    /// Read-only local artifact availability. Does not verify or issue executable capabilities.
    fn visualizer_artifact_available_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _visualizer: &HolonReference,
    ) -> Result<bool, HolonError> {
        Ok(false)
    }

    /// Resolve committed Visualizer configuration in the current authorized space.
    /// Native clients delegate this read-only operation to the guest in one Dance.
    fn resolve_visualizer_usage_internal(
        &self,
        _context: &Arc<TransactionContext>,
        visualizer: &HolonReference,
        subject_type: &HolonReference,
        slot: &HolonReference,
    ) -> Result<crate::dances::VisualizerUsageResolution, HolonError> {
        crate::dances::visualizer_usage_resolution::resolve_visualizer_usage(
            visualizer,
            subject_type,
            slot,
        )
    }

    /// Materializes the executable artifact for a selected Visualizer.
    ///
    /// A Host-Authoritative service may satisfy this locally; a service whose
    /// execution context cannot access the artifact backend leaves it
    /// unsupported.
    fn materialize_visualizer_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _visualizer: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        Err(HolonError::NotImplemented("materialize_visualizer_internal".to_string()))
    }

    /// Retrieves executable artifact bytes for a capability issued by a
    /// materialization operation. Implementations must treat the handle as
    /// opaque and enforce its scope before returning content.
    fn fetch_artifact_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _handle: &MapString,
    ) -> Result<MapBytes, HolonError> {
        Err(HolonError::NotImplemented("fetch_artifact_internal".to_string()))
    }

    /// Activates the named Dancer package through this execution context's
    /// package catalog and loader strategy.
    fn activate_dancer_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _package_identity: &MapString,
    ) -> Result<(), HolonError> {
        Err(HolonError::NotImplemented("activate_dancer_internal".to_string()))
    }

    /// Expands generic SmartLinks from one local source and relationship.
    fn expand_smartlinks_from_source_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &LocalId,
        _relationship_name: &RelationshipName,
    ) -> Result<Vec<SmartLink>, HolonError> {
        Err(HolonError::NotImplemented("expand_smartlinks_from_source_internal".to_string()))
    }

    /// Expands generic SmartLinks from one local source, filtered by canonical key.
    fn expand_smartlinks_from_source_by_key_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &LocalId,
        _relationship_name: &RelationshipName,
        _key_match: KeyMatch,
    ) -> Result<Vec<SmartLink>, HolonError> {
        Err(HolonError::NotImplemented("expand_smartlinks_from_source_by_key_internal".to_string()))
    }

    /// Returns distinct local lineage roots associated with an exact saved key,
    /// including historical keyed Owns entries. Does not select lineage heads.
    fn get_saved_lineage_roots_by_key_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _key: &base_types::MapString,
    ) -> Result<Vec<LocalId>, HolonError> {
        Err(HolonError::NotImplemented("get_saved_lineage_roots_by_key_internal".into()))
    }

    /// Resolves an exact key in the current space to its sole visible saved lineage head.
    fn get_saved_holon_by_key_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _key: &base_types::MapString,
    ) -> Result<SmartReference, HolonError> {
        Err(HolonError::NotImplemented("get_saved_holon_by_key_internal".to_string()))
    }

    /// Execute a Holon Loader import using a HolonLoadSet (transient) reference.
    /// Returns a transient reference to a HolonLoadResponse holon.
    fn load_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        load_set: TransientReference,
    ) -> Result<TransientReference, HolonError>;

    /// Load a Theme through the Space-authoritative canonical Dance.
    fn load_theme_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _theme: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        Err(HolonError::NotImplemented("Space-authoritative LoadTheme".into()))
    }

    /// Executes a validated canonical loader invocation in the authoritative Space.
    fn invoke_load_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _invocation: crate::dances::DanceInvocation,
    ) -> Result<crate::dances::DanceResponseReference, HolonError> {
        Err(HolonError::NotImplemented("Canonical LoadHolons execution".into()))
    }

    /// Ensures a persisted local space holon exists and returns its reference.
    ///
    /// Default implementation is unsupported outside guest/runtime contexts.
    fn ensure_local_holon_space_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        Err(HolonError::NotImplemented("ensure_local_holon_space_internal".to_string()))
    }
}
