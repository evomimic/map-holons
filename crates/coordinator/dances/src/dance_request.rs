use hdk::prelude::*;
use holons::holon_reference::HolonReference;
use shared_types_holon::{HolonId, MapString, PropertyMap};
use crate::staging_area::StagingArea;


#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct DanceRequest {
    pub dance_name: MapString, // unique key within Offered Holon Type
    pub offering_holon: HolonReference,
    pub handler: HolonId, // the space that can handle this request
    pub body: RequestBody,
    pub dance_type: DanceType, // Action, Command or Query?
    pub descriptor: HolonReference, // space_id+holon_id of DanceDescriptor
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
pub struct RequestBody {
    pub parameters: PropertyMap,  // input parameters for this request
}






