use crate::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, RelationshipName,
};
use crate::reference_layer::HolonsContextBehavior;
use shared_types_holon::{HolonId, LocalId};
use std::fmt::Debug;

use super::{HolonReference, SmartReference, StagedReference};

pub trait HolonServiceApi: Debug {
    ///
    //fn install_app(&self) -> Result<AppInstallation, HolonError>;
    /// This function commits the staged holons to the persistent store
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError>;

    /// This function deletes the saved holon identified by  from the persistent store
    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError>;

    fn fetch_holon(&self, id: &HolonId) -> Result<Holon, HolonError>;

    fn fetch_related_holons(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining
    /// lineage to the Holon its cloned from.
    fn stage_new_from_clone(
        &self,
        context: &dyn HolonsContextBehavior,
        original_holon: HolonReference,
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
