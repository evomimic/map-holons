use crate::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, RelationshipMap, RelationshipName,
};
use crate::reference_layer::HolonsContextBehavior;
use shared_types_holon::{HolonId, LocalId};
use std::fmt::Debug;
use std::rc::Rc;

pub trait HolonServiceApi: Debug {
    /// This function commits the staged holons to the persistent store
    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError>;

    /// This function deletes the saved holon identified by  from the persistent store
    ///
    fn delete_holon(&self, local_id: &LocalId) -> Result<(), HolonError>;

    fn fetch_holon(&self, id: &HolonId) -> Result<Holon, HolonError>;

    fn fetch_related_holons(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;

    fn fetch_all_populated_relationships(
        &self,
        source_id: HolonId,
    ) -> Result<Rc<RelationshipMap>, HolonError>;
}
