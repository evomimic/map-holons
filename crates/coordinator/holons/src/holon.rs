use hdi::prelude::{ActionHash, Deserialize, Serialize};
use holochain_integrity_types::Record;
use hdk::entry::get;
use hdk::prelude::GetOptions;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName, PropertyValue};
use crate::helpers::get_holon_node_from_record;
use crate::holon_errors::HolonError;
use crate::holon_node::{create_holon_node};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum HolonState {
    New,
    Fetched,
    Changed,
    // CreateInProgress,
    // SaveInProgress,
}


#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Holon {
    pub state: HolonState,
    pub saved_node: Option<Record>, // The last saved state of HolonNode. None = not yet created
    pub property_map: PropertyMap,
    // pub descriptor: HolonReference,
    // pub holon_space: HolonReference,
    // pub outbound_relationships: RelationshipMap,
    //
}

impl Holon {
    pub fn new() -> Holon {
        Holon {
            state: HolonState::New,
            saved_node: None,
            property_map: PropertyMap::new(),
        }
    }
    // Implemented here to avoid conflicts with hdk::core's implementation of TryFrom Trait
    pub fn into_node(self) -> HolonNode {
        HolonNode {
            property_map: self.property_map.clone(),
        }
    }
    pub fn try_from_node(holon_node_record: Record) -> Result<Holon, HolonError> {
        let holon_node = get_holon_node_from_record(holon_node_record.clone())?;
        let holon = Holon {
            state: HolonState::Fetched,
            saved_node: Some(holon_node_record.clone()),
            property_map: holon_node.property_map.clone(),
        };
        Ok(holon)
    }

    // TODO: add error checking and HolonError result
    // Possible Errors: Unrecognized Property Name
    pub fn add_property_value(
        &mut self,
        property: PropertyName,
        value: PropertyValue,
    ) -> &mut Self {
        self.property_map.insert(property, value);
        match self.state {
            HolonState::Fetched => { self.state = HolonState::Changed },
            _ => {}
        }
        self
    }
    // TODO: add error checking and HolonError result
    // Possible Errors: Unrecognized Property Name
    pub fn remove_property_value(
        &mut self,
        property: PropertyName,
    ) -> &mut Self {
        self.property_map.remove(&property);
        match self.state {
            HolonState::Fetched => { self.state = HolonState::Changed },
            _ => {}
        }
        self
    }

    /// commit() creates a HolonNode and SmartLinks if state = New,
    /// updates the HolonNode and SmartLinks if state = Changed,
    /// and just returns the Holon unchanged if state = Fetched,
    pub fn commit(&mut self) -> Result<&mut Self, HolonError> {
        match self.state {
            HolonState::New => {
                // Create a new HolonNode from this Holon and request it be created
                let result = create_holon_node(self.clone().into_node());
                if let Ok(record) = result {
                    self.saved_node = Some(record);
                    self.state = HolonState::Fetched;
                    return Ok(self);
                } else if let Err(error) = result {
                    return Err(HolonError::WasmError(error.to_string()));
                } else {
                    unreachable!()
                };
            },
            HolonState::Fetched => {
                // Holon hasn't been changed since it was fetched
                return Ok(self);
            },
            HolonState::Changed => {
                // request update

                return Ok(self);
            }
        }
    }


    /// fetch_holon gets a specific HolonNode from the persistent store based on its ActionHash
    /// it then "inflates" the HolonNode into a Holon and returns it
    /// Not currently extern... because fetches will be mediated by the cache

    pub fn fetch_holon(
        id: ActionHash,
    ) -> Result<Holon, HolonError> {
        let holon_node_record = get(id.clone(), GetOptions::default())?;
        return if let Some(node) = holon_node_record {
            let mut holon = Holon::try_from_node(node)?;
            holon.state = HolonState::Fetched;
            // could go get relationship map, descriptor, holon_space here;
            Ok(holon)
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(id.to_string()))
        }
    }
}
