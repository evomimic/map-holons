use holons_core::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, RelationshipMap, RelationshipName,
};
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};
use shared_types_holon::{HolonId, LocalId};
use std::rc::Rc;

/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
    fn commit(&self, _context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        todo!()
    }

    fn delete_holon(&self, _local_id: &LocalId) -> Result<(), HolonError> {
        todo!()
    }

    fn fetch_holon(&self, _id: &HolonId) -> Result<Holon, HolonError> {
        todo!()
    }

    fn fetch_related_holons(
        &self,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        todo!()
    }

    fn fetch_all_populated_relationships(
        &self,
        _source_id: HolonId,
    ) -> Result<Rc<RelationshipMap>, HolonError> {
        todo!()
    }
}
