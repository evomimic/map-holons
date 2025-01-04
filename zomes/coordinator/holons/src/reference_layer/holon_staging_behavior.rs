use crate::reference_layer::staged_reference::StagedIndex;
use crate::reference_layer::{HolonsContextBehavior, StagedReference};
use crate::shared_objects_layer::{CommitResponse, Holon, HolonError};
use shared_types_holon::MapString;

pub trait HolonStagingBehavior {
    /// This function commits the staged holons to the persistent store
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError>;

    /// Does a lookup by key on staged holons. Note HolonTypes are not required to offer a "key"
    fn get_staged_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError>;
    //fn get_mut_holon_by_index(&self, holon_index: StagedIndex) -> Result<RefMut<Holon>, HolonError>

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the keyed_index to allow the staged holon
    /// to be retrieved by key
    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError>;

    /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    fn to_validated_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError>;
}
