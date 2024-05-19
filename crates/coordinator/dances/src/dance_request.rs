use crate::staging_area::StagingArea;
use hdk::prelude::*;
use holons::commit_manager::StagedIndex;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::smart_reference::SmartReference;
use holons::staged_reference::StagedReference;
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
    CommandMethod(StagedIndex), // a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
}
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum PortableReference {
    Saved(HolonId),
    Staged(StagedIndex),
}
impl PortableReference {
    /// This function converts a PortableReference into a HolonReference with "None" used
    /// for all the "optional" fields.
    pub fn to_holon_reference(self) -> HolonReference {
        match self {
            PortableReference::Saved(holon_id) => {
                let smart_ref = SmartReference {
                    holon_id,
                    key: None,
                    rc_holon: None,
                    smart_property_values: None,
                };
                HolonReference::Smart(smart_ref)
            }
            PortableReference::Staged(staged_index) => {
                let staged_ref = StagedReference {
                    key: None,
                    holon_index: staged_index,
                };
                HolonReference::Staged(staged_ref)
            }
        }
    }
}


#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum RequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<PortableReference>),
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

    pub fn new_target_holons(
        relationship_name: RelationshipName,
        holons_to_add: Vec<PortableReference>,
    ) -> Self {Self::TargetHolons(relationship_name, holons_to_add)}

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
