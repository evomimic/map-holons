use std::any::Any;
use std::fmt::Debug;

use super::{HolonReference, SmartReference, StagedReference};
use crate::core_shared_objects::{CommitResponse, Holon, HolonCollection};
use crate::reference_layer::HolonsContextBehavior;
use crate::RelationshipMap;
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};

/// The `HolonServiceApi` defines the uniform service boundary for working with holons
/// across both client-side and guest-side contexts.
///
/// ## Architectural role
/// This trait is the abstraction layer between higher-level holon logic and the underlying
/// execution environment. By standardizing the API surface, it allows:
/// - **Guest implementations** to translate calls into persistence operations
///   (e.g., commit staged holons, fetch saved holons, delete by id).
/// - **Client implementations** to translate the same calls into `Dance` requests,
///   forwarding them to the guest through the in-container boundary.
///
/// In both cases, callers interact with a single consistent interface without needing
/// to know whether they are running in the client process or within the guest zome
/// and persistence layer.
///
/// ## Avoiding circular dependencies
/// `holons_core` depends only on this trait, not on any specific implementation.
/// This eliminates circular dependencies between the core type system and the
/// environment-specific service logic. Concrete implementations are provided in:
/// - `holons_client` (TypeScript/Rust bridge using `Dance` forwarding)
/// - `holons_guest` (Rust guest zome backed by the DHT persistence layer)
///
/// ## Reference-oriented design
/// Operations use `HolonReference` (with `TransientReference`, `StagedReference`,
/// and `SmartReference` variants) rather than directly moving holons across
/// the boundary. This minimizes serialization overhead and aligns with the
/// MAP model where the nursery, cache, and transient stores are authoritative
/// for holon state. Queries return references; property and relationship data
/// is retrieved or projected lazily.
///
/// ## Summary
/// `HolonServiceApi` is the key extensibility point in the MAP architecture:
/// - Provides a uniform contract for holon operations.
/// - Cleanly separates client/guest responsibilities.
/// - Enables infrastructure services (undo/redo, logging, metrics).
/// - Avoids circular dependencies in `holons_core`.
/// - Ensures reference-oriented ergonomics across all environments.

pub trait HolonServiceApi: Debug + Any {
    fn as_any(&self) -> &dyn Any;

    ///
    //fn install_app(&self) -> Result<AppInstallation, HolonError>;
    /// This function commits the staged holons to the persistent store
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError>;

    /// This function deletes the saved holon identified by  from the persistent store
    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError>;

    fn fetch_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError>;

    fn fetch_holon(&self, id: &HolonId) -> Result<Holon, HolonError>;

    fn fetch_related_holons(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;

    /// Retrieves all persisted Holons, as a HolonCollection
    fn get_all_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError>;

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining
    /// lineage to the Holon its cloned from.
    fn stage_new_from_clone(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError>;

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_version(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError>;
}
