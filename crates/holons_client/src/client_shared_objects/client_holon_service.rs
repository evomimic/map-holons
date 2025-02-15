use dances_core::dance_request::DanceRequest;
use dances_core::dance_response::{DanceResponse, ResponseBody};
use dances_core::session_state::SessionState;
use hdi::prelude::ExternIO;
use hdk::prelude::fake_action_hash;
use holochain_client::ConductorApiError;
use holon_dance_builders::commit_dance::build_commit_dance_request;
use holon_dance_builders::delete_holon_dance::build_delete_holon_dance_request;
use holon_dance_builders::get_holon_by_id_dance::build_get_holon_by_id_dance_request;
use holon_dance_builders::query_relationships_dance::build_query_relationships_dance_request;
use holons_core::core_shared_objects::{
    CollectionState, CommitResponse, Holon, HolonCollection, HolonError, RelationshipMap, RelationshipName
};
use holons_core::reference_layer::{HolonServiceApi, HolonsContextBehavior};
use holons_core::{HolonReference, SmartReference};
use holons_guest::query_layer::{Node, NodeCollection, QueryExpression};
use shared_types_holon::{HolonId, LocalId};
use std::rc::Rc;
use tokio::runtime::Runtime;


use crate::zome_client::{self, ZomeClient};
use crate::AppSessionData;


/// A concrete implementation of the `HolonResolver` trait for resolving local Holons.
/// note i am using tokio block_on here for sync to async .. this is a temporary.. 
/// if these function are not to be changed to async. then i would sugget we implement a async thread inside the sync code, so as not to block the main thread 
#[derive(Debug,Clone)]
pub struct ClientHolonService;

impl HolonServiceApi for ClientHolonService {

    fn commit(&self, _context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
        let request = build_commit_dance_request(&SessionState::empty())?;
        let response = dance_zome_call(request)?;
        let commit_response:CommitResponse = response.into();
        Ok(commit_response)
    }

    fn delete_holon(&self, _local_id: &LocalId) -> Result<(), HolonError> {
        let request = build_delete_holon_dance_request(&SessionState::empty(), LocalId(fake_action_hash(234)))?;
        let _response = dance_zome_call(request)?;
        Ok(())
    }

    fn fetch_holon(&self, _id: &HolonId) -> Result<Holon, HolonError> {
        let request = build_get_holon_by_id_dance_request(&SessionState::empty(), _id.clone())?;
        let response = dance_zome_call(request)?;
        match response.body {
            ResponseBody::Holon(holon) => return Ok(holon),
            _ => return Err(HolonError::HolonNotFound("Invalid response body".to_string())),    
        } 
    }

    fn fetch_related_holons(
        &self,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let holon_reference: HolonReference =
        HolonReference::Smart(SmartReference::new(_source_id.to_owned(), None));
        let node_collection =
        NodeCollection { members: vec![Node::new(holon_reference, None)], query_spec: None };
        let request = build_query_relationships_dance_request(
            &SessionState::empty(), 
            node_collection, QueryExpression{ relationship_name:_relationship_name.clone()})?;
        let response = dance_zome_call(request)?;
        let node_collection = match response.body {
            ResponseBody::Collection(nodes) => nodes,
            _ => return Err(HolonError::HolonNotFound("Invalid response body".to_string())),    
        };
        Ok(HolonCollection::from_parts(CollectionState::Fetched, node_collection.to_holon_references())) 
    }

    fn fetch_all_populated_relationships(
        &self,
        _source_id: HolonId,
    ) -> Result<Rc<RelationshipMap>, HolonError> {
        unimplemented!()
    }


}
//todo: map the errors to the appropriate HolonError
fn dance_zome_call(request:DanceRequest) -> Result<DanceResponse, HolonError> {
    let rt = Runtime::new().unwrap();
    let app_data:AppSessionData = rt.block_on(<zome_client::AppSessionData as ZomeClient>::init("map_holons".to_string(),9999))
    .map_err(|_err: ConductorApiError| HolonError::CommitFailure("sdf".to_string()))?;

    let cell_id = app_data.get_cell_id_by_role(None)
    .map_err(|_err| HolonError::CommitFailure("sdf".to_string()))?;
    let encoded_request = ExternIO::encode(request)
    .map_err(|_err| HolonError::CommitFailure("sdf".to_string()))?;
    let response = rt.block_on(app_data.zomecall(cell_id,"dances","dance", encoded_request))
    .map_err(|_err| HolonError::CommitFailure("sdf".to_string()))?;
    let decoded_response: DanceResponse = ExternIO::decode(&response)
    .map_err(|_err| HolonError::CommitFailure("sdf".to_string()))?;
    Ok(decoded_response)

}