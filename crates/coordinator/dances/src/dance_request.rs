use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::commit_manager::StagedIndex;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;
use holons::query::{NodeCollection, QueryExpression};
use holons::relationship::RelationshipName;

use shared_types_holon::{HolonId, LocalId, MapString, PropertyMap};

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within the (single) dispatch table
    pub dance_type: DanceType,
    pub body: RequestBody,
    pub staging_area: StagingArea,
    //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum DanceType {
    Standalone,                  // i.e., a dance not associated with a specific holon
    QueryMethod(NodeCollection), // a read-only dance originated from a specific, already persisted, holon
    CommandMethod(StagedIndex), // a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
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
}

impl DanceRequest {
    pub fn new(
        dance_name: MapString,
        dance_type: DanceType,
        body: RequestBody,
        staging_area: StagingArea,
    ) -> Self {
        Self {
            dance_name,
            dance_type,
            body,
            staging_area,
        }
    }
}
