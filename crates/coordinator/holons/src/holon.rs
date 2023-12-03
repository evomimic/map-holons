use hdi::prelude::{ActionHash};
use crate::all_holon_nodes::*;
use crate::helpers::get_holon_node_from_record;
use crate::holon_errors::HolonError;
use crate::holon_node::*;
use crate::holon_types::{Holon, HolonState};
use hdk::entry::get;
use hdk::prelude::*;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName, PropertyValue};

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
    pub fn with_property_value(
        &mut self,
        property: PropertyName,
        value: PropertyValue,
    ) -> &mut Self {
        self.property_map.insert(property, value);
        match self.state {
            HolonState::Fetched => self.state = HolonState::Changed,
            _ => {}
        }
        self
    }
    // // TODO: add error checking and HolonError result
    // // Possible Errors: Unrecognized Property Name
    // pub fn remove_property_value(&mut self, property: PropertyName) -> &mut Self {
    //     self.property_map.remove(&property);
    //     match self.state {
    //         HolonState::Fetched => self.state = HolonState::Changed,
    //         _ => {}
    //     }
    //     self
    // }

    pub fn get_id(&self) -> ActionHash {
        // TODO: Add better handling if saved_node is None
        let node = self.saved_node.clone().unwrap();
        node.as_ref().action_address().clone()
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
            }
            HolonState::Fetched => {
                // Holon hasn't been changed since it was fetched
                return Ok(self);
            }
            HolonState::Changed => {
                // TODO: request update

                return Ok(self);
            }
        }
    }

    /// fetch_holon gets a specific HolonNode from the persistent store based on its ActionHash
    /// it then "inflates" the HolonNode into a Holon and returns it
    /// Not currently extern... because fetches will be mediated by the cache

    pub fn fetch_holon(id: ActionHash) -> Result<Holon, HolonError> {
        let holon_node_record = get(id.clone(), GetOptions::default())?;
        return if let Some(node) = holon_node_record {
            let mut holon = Holon::try_from_node(node)?;
            holon.state = HolonState::Fetched;
            // could go get relationship map, descriptor, holon_space here;
            Ok(holon)
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(id.to_string()))
        };
    }

    pub fn delete_holon(id: ActionHash) -> Result<ActionHash, HolonError> {
        let result = delete_holon_node(id);
        match result {
            Ok(result) => Ok(result),
            Err(error) => Err(HolonError::WasmError(error.to_string())),
        }
    }

    pub fn get_all_holons() -> Result<Vec<Holon>, HolonError> {
        let records = get_all_holon_nodes(());
        match records {
            Ok(records) => {
                let mut holons = Vec::<Holon>::new();
                for holon_node_record in records.clone() {
                    let holon = Holon::try_from_node(holon_node_record.clone())?;
                    holons.push(holon);
                }
                Ok(holons)
            }
            Err(error) => Err(HolonError::WasmError(error.to_string())),
        }
    }
}
// =======
// use hdk::prelude::*;
// use holons_integrity::*;
// #[hdk_extern]
// pub fn create_holon(holon: Holon) -> ExternResult<Record> {
//     let holon_hash = create_entry(&EntryTypes::Holon(holon.clone()))?;
//     let record = get(holon_hash.clone(), GetOptions::default())?
//         .ok_or(
//             wasm_error!(
//                 WasmErrorInner::Guest(String::from("Could not find the newly created Holon"))
//             ),
//         )?;
//     let path = Path::from("all_holons");
//     create_link(path.path_entry_hash()?, holon_hash.clone(), LinkTypes::AllHolons, ())?;
//     Ok(record)
// }
// #[hdk_extern]
// pub fn get_holon(original_holon_hash: ActionHash) -> ExternResult<Option<Record>> {
//     let links = get_links(original_holon_hash.clone(), LinkTypes::HolonUpdates, None)?;
//     let latest_link = links
//         .into_iter()
//         .max_by(|link_a, link_b| link_a.timestamp.cmp(&link_b.timestamp));
//     let latest_holon_hash = match latest_link {
//         Some(link) => ActionHash::from(link.target.clone()),
//         None => original_holon_hash.clone(),
//     };
//     get(latest_holon_hash, GetOptions::default())
// }
// #[derive(Serialize, Deserialize, Debug)]
// pub struct UpdateHolonInput {
//     pub original_holon_hash: ActionHash,
//     pub previous_holon_hash: ActionHash,
//     pub updated_holon: Holon,
// }
// #[hdk_extern]
// pub fn update_holon(input: UpdateHolonInput) -> ExternResult<Record> {
//     let updated_holon_hash = update_entry(
//         input.previous_holon_hash.clone(),
//         &input.updated_holon,
//     )?;
//     create_link(
//         input.original_holon_hash.clone(),
//         updated_holon_hash.clone(),
//         LinkTypes::HolonUpdates,
//         (),
//     )?;
//     let record = get(updated_holon_hash.clone(), GetOptions::default())?
//         .ok_or(
//             wasm_error!(
//                 WasmErrorInner::Guest(String::from("Could not find the newly updated Holon"))
//             ),
//         )?;
//     Ok(record)
// }
// #[hdk_extern]
// pub fn delete_holon(original_holon_hash: ActionHash) -> ExternResult<ActionHash> {
//     delete_entry(original_holon_hash)
//
// }
