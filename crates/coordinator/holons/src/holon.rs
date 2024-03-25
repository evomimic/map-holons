use std::fmt;

use derive_new::new;
use hdi::prelude::ActionHash;

use hdk::prelude::*;

use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName};
use shared_types_holon::value_types::BaseValue;
use shared_types_holon::{HolonId, MapString, PropertyValue};

use crate::context::HolonsContext;
use crate::helpers::get_holon_node_from_record;
use crate::holon_error::HolonError;
use crate::holon_node::UpdateHolonNodeInput;
use crate::holon_node::*;
use crate::relationship::RelationshipMap;
use crate::smart_reference::SmartReference;
use crate::{all_holon_nodes::*, property_map};

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct Holon {
    pub state: HolonState,
    pub validation_state: ValidationState,
    pub saved_node: Option<Record>, // The last saved state of HolonNode. None = not yet created
    pub predecessor: Option<SmartReference>, // Linkage to previous Holon version. None = cloned template
    pub property_map: PropertyMap,
    pub relationship_map: RelationshipMap,
    key: Option<MapString>,
    // pub descriptor: HolonReference,
    // pub holon_space: HolonReference,
    // pub dancer : Dancer,
    pub errors: Vec<HolonError>,
}

// Move to id staged holons via index should mean that derived implementations of PartialEq and Eq
// /// The PartialEq and Eq traits need to be implemented for Holon to support Vec operations of the CommitManager.
// /// NOTE: Holons types are NOT required to have a Key, so we can't rely on key for identity.
// /// * For *retrieved Holons*, the HolonId can serve as a unique id for purposes of comparison
// /// * But *staged holons* don't have a HolonId. In this case, identity is determined on _saved_node_ and property_values
// impl Eq for Holon {}
//
// impl PartialEq for Holon {
//     fn eq(&self, other: &Self) -> bool {
//         match (&self.state, &other.state) {
//             (HolonState::Fetched, HolonState::Fetched) => {
//                 if let (Some(self_address),
//                     Some(other_address)) =
//                     (self.saved_node.as_ref().map(|record| record.action_address()),
//                      other.saved_node.as_ref().map(|record| record.action_address())) {
//                     return self_address == other_address;
//                 }
//                 false // If action addresses are not present, they are not equal
//             }
//             (HolonState::Changed, HolonState::Changed) => {
//                 self.saved_node == other.saved_node
//             }
//             (HolonState::New, HolonState::New) => {
//                 self.property_map == other.property_map
//             }
//             _ => false, // In all other cases, Holons are not equal
//         }
//     }
// }

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum HolonState {
    New,
    Fetched,
    Changed,
    Saved,
    // SaveInProgress,
}

impl fmt::Display for HolonState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HolonState::New => write!(f, "New"),
            HolonState::Fetched => write!(f, "Fetched"),
            HolonState::Changed => write!(f, "Changed"),
            HolonState::Saved => write!(f, "Saved"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ValidationState {
    NoDescriptor,
    ValidationRequired,
    Validated,
    Invalid,
}

pub trait HolonFieldGettable {
    fn get_property_value(
        &mut self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError>;

    fn get_key(&mut self, context: &HolonsContext) -> Result<Option<MapString>, HolonError>;

    // fn query_relationship(&self, context: HolonsContext, relationship_name: RelationshipName, query_spec: Option<QuerySpec>-> SmartCollection;
}

impl Holon {
    /// Stages a new empty holon.
    pub fn new() -> Holon {
        Holon {
            state: HolonState::New,
            validation_state: ValidationState::NoDescriptor,
            saved_node: None,
            predecessor: None,
            property_map: PropertyMap::new(),
            relationship_map: RelationshipMap::new(),
            key: None,
            errors: Vec::new(),
        }
    }

    /// This function bypasses the cache (it should be retired in favor of fetch_holon once cache is implemented
    /// TODO: replace with cache aware function
    pub fn get_holon(id: HolonId) -> Result<Option<Holon>, HolonError> {
        let holon_node_record = get_holon_node(id.0.clone())?;
        return if let Some(node) = holon_node_record {
            let mut holon = Holon::try_from_node(node)?;
            holon.state = HolonState::Fetched;
            Ok(Some(holon))
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(id.0.to_string()))
        };
    }

    pub fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        self.property_map
            .get(property_name)
            .cloned()
            .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
    }

    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        Ok(self.key.clone())
    }

    pub fn into_node(self) -> HolonNode {
        HolonNode {
            property_map: self.property_map.clone(),
        }
    }

    /// try_from_node inflates a Holon from a HolonNode.
    /// Since Implemented here to avoid conflicts with hdk::core's implementation of TryFrom Trait
    pub fn try_from_node(holon_node_record: Record) -> Result<Holon, HolonError> {
        let holon_node = get_holon_node_from_record(holon_node_record.clone())?;

        let holon = Holon {
            state: HolonState::Fetched,
            validation_state: ValidationState::Validated,
            saved_node: Some(holon_node_record),
            predecessor: None,
            property_map: holon_node.property_map,
            relationship_map: RelationshipMap::new(),
            key: None,
            errors: Vec::new(),
        };

        // TODO: populate predecessor from link to previous record for this Holon

        // TODO: populate `key` from the property map once we have Descriptors/Constraints available

        // TODO: Populate RelationshipMap from links

        Ok(holon)
    }

    // NOTE: this function doesn't check if supplied RelationshipName is a valid outbound
    // relationship for the self holon. It probably  needs to be possible to suspend
    // this checking while the type system is being bootstrapped, since the descriptors
    // required by the validation may not yet exist.
    // TODO: Add conditional validation checking when adding relationships
    // pub fn add_related_holon(
    //     &mut self,
    //     name: RelationshipName,
    //     target: RelationshipTarget,
    // ) -> &mut Self {
    //     self.relationship_map.insert(name, target);
    //     match self.state {
    //         HolonState::Fetched => self.state = HolonState::Changed,
    //         _ => {}
    //     }
    //     self
    // }
    // NOTE: this function doesn't check if supplied PropertyName is a valid property
    // for the self holon. It probably needs to be possible to suspend
    // this checking while the type system is being bootstrapped, since the descriptors
    // required by the validation may not yet exist.
    // TODO: Add conditional validation checking when adding properties
    // TODO: add error checking and HolonError result
    // Possible Errors: Unrecognized Property Name
    pub fn with_property_value(&mut self, property: PropertyName, value: BaseValue) -> &mut Self {
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

    pub fn set_key_manually(&mut self, key: MapString) {
        self.key = Some(key);
    }

    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        let node = self.saved_node.clone();
        if let Some(record) = node {
            Ok(HolonId(record.action_address().clone()))
        } else {
            Err(HolonError::HolonNotFound("Node is empty".to_string()))
        }
    }

    /// commit() saves a staged holon to the persistent store.
    /// If the staged holon is `New`, it creates a HolonNode and SmartLinks
    /// If the staged holon is `Changed`, it persists a new version of HolonNode and its SmartLinks
    /// If the staged holon is `Fetched`, it does nothing
    /// If there are no errors, this function creates the HolonId of the newly saved Holon
    /// The state of the original holon (i.e., self) is updated to `Fetched` so that commits are
    /// idempotent.
    pub fn commit(&mut self, context: &HolonsContext) -> Result<HolonId, HolonError> {
        match self.state {
            HolonState::New => {
                // Create a new HolonNode from this Holon and request it be created
                let result = create_holon_node(self.clone().into_node());
                match result {
                    Ok(record) => {
                        let holon_id = HolonId(record.action_address().clone());
                        // Iterate through the holon's relationship map, invoking commit on each
                        for (name, target) in self.relationship_map.0.clone() {
                            target.commit(context, holon_id.clone(), name.clone())?;
                        }

                        self.state = HolonState::Saved;

                        Ok(holon_id)
                    }
                    Err(error) => Err(HolonError::from(error)),
                }
            }

            HolonState::Changed => {
                if let Some(ref node) = self.saved_node {
                    let input = UpdateHolonNodeInput {
                        // TEMP solution for original hash is to keep it the same //
                        original_holon_node_hash: node.action_address().clone(), // TODO: find way to populate this correctly
                        previous_holon_node_hash: node.action_address().clone(),
                        updated_holon_node: self.clone().into_node(),
                    };
                    let result = update_holon_node(input);
                    match result {
                        Ok(record) => {
                            let holon_id = HolonId(record.action_address().clone());
                            for (name, target) in self.clone().relationship_map.0 {
                                target.commit(context, holon_id.clone(), name.clone())?;
                            }
                            self.saved_node = Some(record);

                            Ok(holon_id)
                        }
                        Err(error) => Err(HolonError::from(error)),
                    }
                } else {
                    Err(HolonError::HolonNotFound(
                        "Must have a saved node in order to update".to_string(),
                    ))
                }
            }
            _ => {
                // For either Fetched or Saved no save is needed, just return HolonId

                let node = self.saved_node.clone();
                if let Some(record) = node {
                    return Ok(HolonId(record.action_address().clone()));
                } else {
                    Err(HolonError::HolonNotFound(
                        "Expected Holon to have a saved_node, but it doesn't".to_string(),
                    ))
                }
            }
        }
    }

    pub fn delete_holon(id: HolonId) -> Result<ActionHash, HolonError> {
        let result = delete_holon_node(id.0);
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
    // pub fn commit(mut self, context: &HolonsContext) -> Result<Self, HolonError> {
    //     //let mut holon = self.clone(); // avoid doing this?
    //     match self.state {
    //         HolonState::New => {
    //             // Create a new HolonNode from this Holon and request it be created
    //             let result = create_holon_node(self.clone().into_node());
    //             match result {
    //                 Ok(record) => {
    //                     let holon_id = HolonId(record.action_address().clone());
    //                     for (name, target) in self.relationship_map.0.clone() {
    //                         target.commit(context, holon_id.clone(), name.clone())?;
    //                     }
    //                     self.saved_node = Some(record);
    //                     self.state = HolonState::Fetched;
    //
    //                     Ok(self)
    //                 }
    //                 Err(error) => Err(HolonError::from(error)),
    //             }
    //         }
    //         HolonState::Fetched => {
    //             // Holon hasn't been changed since it was fetched
    //             return Ok(self);
    //         }
    //         HolonState::Changed => {
    //             if let Some(node) = self.saved_node.clone() {
    //                 let input = UpdateHolonNodeInput {
    //                     // TEMP solution for original hash is to keep it the same //
    //                     original_holon_node_hash: node.action_address().clone(), // TODO: find way to populate this correctly
    //                     previous_holon_node_hash: node.action_address().clone(),
    //                     updated_holon_node: self.clone().into_node(),
    //                 };
    //                 let result = update_holon_node(input);
    //                 match result {
    //                     Ok(record) => {
    //                         let holon_id = HolonId(record.action_address().clone());
    //                         for (name, target) in self.clone().relationship_map.0 {
    //                             target.commit(context, holon_id.clone(), name.clone())?;
    //                         }
    //                         self.saved_node = Some(record);
    //
    //                         Ok(self)
    //                     }
    //                     Err(error) => Err(HolonError::from(error)),
    //                 }
    //             } else {
    //                 Err(HolonError::HolonNotFound(
    //                     "Must have a saved node in order to update".to_string(),
    //                 ))
    //             }
    //         }
    //     }
    // }

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
}
