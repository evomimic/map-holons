#![allow(unused_variables)]
use holons_core::core_shared_objects::{
    CommitResponse, Holon, HolonCollection, HolonError, RelationshipName,
};
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};
use holons_core::{HolonReference, SmartReference, StagedReference};
use shared_types_holon::{HolonId, LocalId, MapString};

#[derive(Debug, Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {
    //fn install_app(&self) -> Result<AppInstallation, HolonError> {
    ///   ZomeClient::install_app()
    //}

    fn commit(&self, _context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        //let request = build_commit_dance_request(&SessionState::default())?;
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

    fn get_all_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCollection, HolonError> {
        todo!()
    }

    fn stage_new_from_clone(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: HolonReference,
        _new_key: MapString,
    ) -> Result<StagedReference, HolonError> {
        todo!()
    }

    fn stage_new_version(
        &self,
        _context: &dyn HolonsContextBehavior,
        _original_holon: SmartReference,
    ) -> Result<StagedReference, HolonError> {
        todo!()
    }
}
