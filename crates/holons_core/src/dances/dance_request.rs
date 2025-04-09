use crate::core_shared_objects::{Holon, KeyPropertyMap, RelationshipName};
use serde::{Deserialize, Serialize};

use crate::dances::SessionState;
use crate::query_layer::{NodeCollection, QueryExpression};
use crate::{HolonReference, StagedReference};
use shared_types_holon::{HolonId, LocalId, MapString, PropertyMap};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within the (single) dispatch table
    pub dance_type: DanceType,
    pub body: RequestBody,
    pub state: Option<SessionState>,
    //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DanceType {
    Standalone,                     // i.e., a dance not associated with a specific holon
    QueryMethod(NodeCollection), // a read-only dance originated from a specific, already persisted, holon
    CommandMethod(StagedReference), // a mutating method operating on a specific staged_holon identified by StagedReference
    CloneMethod(KeyPropertyMap),    // a specific method for cloning a Holon
    NewVersionMethod(HolonId), // a SmartReference only method for cloning a Holon as new version by linking to the original Holon it was cloned from via PREDECESSOR relationship
    DeleteMethod(LocalId),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<HolonReference>),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReference),
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

    pub fn new_staged_reference(staged_reference: StagedReference) -> Self {
        Self::StagedRef(staged_reference)
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
        state: Option<SessionState>,
    ) -> Self {
        Self {
            dance_name,
            dance_type,
            body,
            state: state.or(Some(SessionState::default())), // Default if None
        }
    }
    /// Gets a reference to the session state, or `None` if not set.
    pub fn get_state(&self) -> Option<&SessionState> {
        self.state.as_ref()
    }

    /// Gets a mutable reference to the session state, or `None` if not set.
    pub fn get_state_mut(&mut self) -> Option<&mut SessionState> {
        self.state.as_mut()
    }

    /// Summarizes the DanceRequest for logging purposes.
    ///
    /// Handles cases where session state is `None` by providing a placeholder message.
    pub fn summarize(&self) -> String {
        format!(
            "DanceRequest {{ \n  dance_name: {:?}, dance_type: {:?}, \n  body: {}, \n  state: {} }}\n",
            self.dance_name.to_string(),
            self.dance_type,
            self.body.summarize(),
            self.state
                .as_ref()
                .map_or_else(|| "None".to_string(), |state| state.summarize()),
        )
    }
}
