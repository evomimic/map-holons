use std::any::Any;
use std::fmt::Debug;

use super::{HolonReference, SmartReference, StagedReference, TransientReference};
use crate::core_shared_objects::{CommitResponse, Holon, HolonCollection};
use crate::dances::DanceCallServiceApi; // temporary
use crate::reference_layer::HolonsContextBehavior;
use crate::RelationshipMap;
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};

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

    ///
    //fn install_app(&self) -> Result<AppInstallation, HolonError>;
    /// This function commits the staged holons to the persistent store
    fn commit_internal(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<CommitResponse, HolonError>;

    /// This function deletes the saved holon identified by  from the persistent store
    fn delete_holon_internal(&self, local_id: &LocalId) -> Result<(), HolonError>;

    fn fetch_all_related_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError>;

    fn fetch_holon_internal(&self, id: &HolonId) -> Result<Holon, HolonError>;

    fn fetch_related_holons_internal(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;

    /// Retrieves all persisted Holons, as a HolonCollection
    fn get_all_holons_internal(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError>;

    /// Execute a Holon Loader import using a HolonLoaderBundle (transient) reference.
    /// Returns a transient reference to a HolonLoadResponse holon.
    fn load_holons_internal(
        &self,
        ctx: &dyn HolonsContextBehavior,
        bundle: TransientReference,
        dance: Option<&dyn DanceCallServiceApi>, // temp param, Option for guest side
    ) -> Result<TransientReference, HolonError>;

    /// Creates a new Holon in transient state, without any lineage to prior Holons.
    fn new_holon_internal(
        &self,
        ctx: &dyn HolonsContextBehavior,
        key: Option<MapString>,
        dance: Option<&dyn DanceCallServiceApi>, // temp param, Option for guest side
    ) -> Result<TransientReference, HolonError>;

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining
    /// lineage to the Holon its cloned from.
    fn stage_new_from_clone_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: HolonReference,
        new_key: MapString,
    ) -> Result<StagedReference, HolonError>;

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_version_internal(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError>;
}
