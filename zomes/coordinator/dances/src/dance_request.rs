use hdk::prelude::*;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;
use holons::query::{NodeCollection, QueryExpression};
use holons::relationship::RelationshipName;
use holons::space_manager::HolonSpaceManager;
use holons::staged_reference::StagedIndex;

use crate::session_state::SessionState;
use shared_types_holon::{HolonId, LocalId, MapString, PropertyMap};

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within the (single) dispatch table
    pub dance_type: DanceType,
    pub body: RequestBody,
    // pub staging_area: StagingArea,
    state: SessionState,
    //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum DanceType {
    Standalone,                  // i.e., a dance not associated with a specific holon
    QueryMethod(NodeCollection), // a read-only dance originated from a specific, already persisted, holon
    CommandMethod(StagedIndex), // a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
    CloneMethod(HolonReference), // a specific method for cloning a Holon
    NewVersionMethod(HolonId), // a SmartReference only method for cloning a Holon as new version by linking to the original Holon it was cloned from via PREDECESSOR relationship
    DeleteMethod(LocalId),
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<HolonReference>),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    Index(StagedIndex),
    QueryExpression(QueryExpression),
}

impl RequestBody {
    pub fn new() -> Self {
        Self::None // Assuming 'None' is the default variant
    }

    pub fn new_holon(holon: Holon) -> Self {
        Self::Holon(holon)
    }

    pub fn new_parameter_values(parameters: PropertyMap) -> Self {
        Self::ParameterValues(parameters)
    }

    pub fn new_target_holons(
        relationship_name: RelationshipName,
        holons_to_add: Vec<HolonReference>,
    ) -> Self {
        Self::TargetHolons(relationship_name, holons_to_add)
    }

    pub fn new_index(index: StagedIndex) -> Self {
        Self::Index(index)
    }

    pub fn new_query_expression(query_expression: QueryExpression) -> Self {
        Self::QueryExpression(query_expression)
    }
    pub fn summarize(&self) -> String {
        match &self {
            RequestBody::Holon(holon) => format!("  Holon summary: {}", holon.summarize()),
            RequestBody::TargetHolons(relationship_name, holons) => format!(
                "  relationship: {:?}, {{\n    holon_references: {:?} }} ",
                relationship_name, holons
            ),
            RequestBody::HolonId(holon_id) => format!("  HolonId: {:?}", holon_id),

            _ => format!("{:#?}", self), // Use full debug for other response bodies
        }
    }
}

impl DanceRequest {
    pub fn new(
        dance_name: MapString,
        dance_type: DanceType,
        body: RequestBody,
        state: SessionState,
    ) -> Self {
        Self { dance_name, dance_type, body, state }
    }
    pub fn get_state(&self) -> &SessionState {
        &self.state
    }
    // Optionally, you can provide a mutable getter for state if needed
    pub fn get_state_mut(&mut self) -> &mut SessionState {
        &mut self.state
    }

    // Method to initialize the HolonsContext + HolonSpaceManger from the session state
    pub fn init_context_from_state(&self) -> HolonsContext {
        let staged_holons = self.get_state().get_staging_area().get_staged_rc_holons(); //from_stage_to_nursery();
        let stage_index = self.get_state().get_staging_area().get_staged_index();
        let local_space_holon = self.get_state().get_local_holon_space();
        let space_manager =
            HolonSpaceManager::new_from_session(staged_holons, stage_index, local_space_holon);
        //let space_manager = match local_space_holon {
        //   None  => HolonSpaceManager::new(),
        //   Some(local_space_holon) => {
        //       debug!("Space manager created from session state in dance request");
        //      HolonSpaceManager::new_from_session(nursery, local_space_holon)
        //  }
        //};
        HolonsContext::init_context(space_manager)
    }
    // Method to summarize the DanceResponse for logging purposes
    pub fn summarize(&self) -> String {
        format!(
            "DanceRequest {{ \n  dance_name: {:?}, dance_type: {:?}, \n  body: {}, \n  state: {} }}\n",
            self.dance_name.to_string(),
            self.dance_type,
            self.body.summarize(),
            self.state.summarize(),
        )
    }
}
