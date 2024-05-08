use hdk::prelude::*;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;
use holons::smart_collection::SmartCollection;
use shared_types_holon::{HolonId, MapString, PropertyMap};
use crate::staging_area::{StagingArea,StagedIndex};


#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within Offered Holon Type
    // pub offering_holon: HolonReference,
    // pub handler: HolonId, // the space that can handle this request
    pub body: RequestBody,
    // pub dance_type: DanceType, // Action, Command or Query?
    //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
    pub staging_area: Option<StagingArea>,

}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum DanceType {
    Query,
    Command,
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    ParameterValues(PropertyMap),
    Index(StagedIndex),
}
// pub struct RequestBody {
//     pub parameters: PropertyMap,  // input parameters for this request
// }
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
    pub fn new(dance_name:MapString, body: RequestBody)->Self {
        Self {
            dance_name,
            body,
            staging_area: None,
        }
    }
}






