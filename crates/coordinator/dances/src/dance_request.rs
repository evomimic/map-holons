use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::commit_manager::StagedIndex;
use holons::holon::Holon;
use shared_types_holon::{HolonId, MapString, PropertyMap};

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
    Standalone,           // i.e., a dance not associated with a specific holon
    QueryMethod(HolonId), // a read-only dance originated from a specific, already persisted, holon
    Command(StagedIndex), // a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
}
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    Index(StagedIndex),
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

    pub fn new_index(index: StagedIndex) -> Self {
        Self::Index(index)
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
