use crate::reference_layer::{
    HolonReference, HolonsContextBehavior, SmartReference, StagedReference,
};

use crate::core_shared_objects::{Holon, HolonError};
use shared_types_holon::MapString;

pub trait HolonStagingBehavior {
    /// Does a lookup by key on staged holons. Note HolonTypes are not required to offer a "key"
    fn get_staged_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError>;
    //fn get_mut_holon_by_index(&self, holon_index: StagedIndex) -> Result<RefMut<Holon>, HolonError>

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
    fn stage_new_holon(
        &self,
        context: &dyn HolonsContextBehavior,
        holon: Holon,
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
