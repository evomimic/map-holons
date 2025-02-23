use dances_core::session_state::SessionState;
use holon_dance_builders::commit_dance::*;
use holons_core::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, RelationshipMap, RelationshipName,
};
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};
use shared_types_holon::{HolonId, LocalId};
use std::rc::Rc;


/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
#[derive(Debug,Clone)]
pub struct ClientHolonService;// {
    //app_installation: AppInstallation
//}

impl HolonServiceApi for ClientHolonService {

    //fn install_app(&self) -> Result<AppInstallation, HolonError> {
     ///   ZomeClient::install_app()
    //}

    fn commit(&self, _context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        //let request = build_commit_dance_request(&SessionState::empty())?;
       // let response: DanceResponse = conductor.call(&cell.zome("dances"), "dance", valid_request).await;
       // _context.get_space_manager()
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
