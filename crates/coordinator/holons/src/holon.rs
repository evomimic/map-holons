use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use derive_new::new;
use hdi::prelude::ActionHash;

use hdk::prelude::*;

use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName, PropertyValue};
use shared_types_holon::{HolonId, MapInteger, MapString};

use shared_types_holon::value_types::BaseValue;
use shared_validation::ValidationError;

use crate::context::HolonsContext;
use crate::helpers::get_holon_node_from_record;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_node::UpdateHolonNodeInput;
use crate::holon_node::*;
use crate::holon_reference::{HolonReference};
use crate::relationship::{RelationshipMap, RelationshipName};
use crate::smart_reference::SmartReference;
use crate::smartlink::{get_all_relationship_links, get_relationship_links};
use crate::{all_holon_nodes::*};

#[derive(Debug)]
pub enum AccessType {
    Read,
    Write,
    Abandon,
    Commit,
}
impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessType::Read => write!(f, "Read"),
            AccessType::Write => write!(f, "Write"),
            AccessType::Abandon => write!(f, "Abandon"),
            AccessType::Commit => write!(f, "Commit"),
        }
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct Holon {
    pub state: HolonState,
    pub validation_state: ValidationState,
    pub saved_node: Option<Record>, // The last saved state of HolonNode. None = not yet created
    pub predecessor: Option<SmartReference>, // Linkage to previous Holon version. None = cloned template
    pub property_map: PropertyMap,
    pub relationship_map: RelationshipMap,
    pub descriptor: Option<HolonReference>,
    // pub holon_space: HolonReference,
    // pub dancer : Dancer,
    pub errors: Vec<HolonError>,
}

/// Type used for testing in order to match the essential content of a Holon
#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct EssentialHolonContent {
    pub property_map: PropertyMap,
    //pub relationship_map: RelationshipMap,
    key: Option<MapString>,
    pub descriptor: Option<HolonReference>,
    pub errors: Vec<HolonError>,
}

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub enum HolonState {
    New,
    Fetched,
    Changed,
    Saved,
    Abandoned,
}

impl fmt::Display for HolonState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HolonState::New => write!(f, "New"),
            HolonState::Fetched => write!(f, "Fetched"),
            HolonState::Changed => write!(f, "Changed"),
            HolonState::Saved => write!(f, "Saved"),
            HolonState::Abandoned => write!(f, "Abandoned"),
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

// impl HolonGettable for Holon {
//     fn get_property_value(
//         &self,
//         _context: &HolonsContext,
//         property_name: &PropertyName,
//     ) -> Result<PropertyValue, HolonError> {
//         self.is_accessible(AccessType::Read)?;
//         self.property_map
//             .get(property_name)
//             .cloned()
//             .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
//     }
//
//     fn get_key(&self, _context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
//         self.is_accessible(AccessType::Read)?;
//         let key = self
//             .property_map
//             .get(&PropertyName(MapString("key".to_string())));
//         if let Some(key) = key {
//             let string_value: String = key.try_into().map_err(|_| {
//                 HolonError::UnexpectedValueType(format!("{:?}", key), "MapString".to_string())
//             })?;
//             Ok(Some(MapString(string_value)))
//         } else {
//             Ok(None)
//         }
//     }
//
//     fn get_related_holons(
//         &self,
//         _context: &HolonsContext,
//         relationship_name: Option<RelationshipName>,
//     ) -> Result<RelationshipMap, HolonError> {
//         self.is_accessible(AccessType::Read)?;
//         let relationship_map = self.relationship_map.clone();
//         if let Some(name) = relationship_name {
//             let collection_option = relationship_map.0.get(&name);
//             if let Some(collection) = collection_option.clone() {
//                 let mut map = BTreeMap::new();
//                 map.insert(name, collection.clone());
//                 return Ok(RelationshipMap(map));
//             } else {
//                 return Ok(RelationshipMap(BTreeMap::new()));
//             }
//         } else {
//             Ok(relationship_map)
//         }
//     }
// }

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
            descriptor: None,
            errors: Vec::new(),
        }
    }

    pub fn abandon_staged_changes(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Abandon)?;

        self.state = HolonState::Abandoned;
        Ok(())

    }


    /// commit() saves a staged holon to the persistent store.
    ///
    /// If the staged holon is already  `Fetched`, `Saved`, or `Abandoned`, commit does nothing.
    ///
    /// If the staged holon is `New`, commit attempts to create a HolonNode.
    ///
    /// If the staged holon is `Changed`, commit persists a new version of the HolonNode
    ///
    /// If the create or update is successful, the holon's `saved_node` is set from the record
    /// returned, its `state` is changed to `Saved`, so that commits are idempotent, and the
    /// function returns a clone of self.
    ///
    /// If an error is encountered, it is pushed into the holons `errors` vector, the holon's state
    /// is left unchanged and an Err is returned.
    ///

    pub fn commit(&mut self) -> Result<Holon, HolonError> {
        debug!(
            "Entered Holon::commit for holon with key {:#?} in {:#?} state",
            self.get_key()?.unwrap_or_else(|| MapString("<None>".to_string())).0,
            self.state
        );
        match self.state {
            HolonState::New => {
                // Create a new HolonNode from this Holon and request it be created
                trace!("HolonState is New... requesting new HolonNode be created in the DHT");
                let result = create_holon_node(self.clone().into_node());

                match result {
                    Ok(record) => {
                        self.state = HolonState::Saved;
                        self.saved_node = Option::from(record);

                        Ok(self.clone())
                    }
                    Err(error) => {
                        let holon_error = HolonError::from(error);
                        self.errors.push(holon_error.clone());
                        Err(holon_error)
                    }
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
                    debug!("Requesting HolonNode be updated in the DHT");
                    let result = update_holon_node(input);
                    match result {
                        Ok(record) => {
                            self.state = HolonState::Saved;
                            self.saved_node = Option::from(record);
                            Ok(self.clone())
                        }
                        Err(error) => {
                            let holon_error = HolonError::from(error);
                            self.errors.push(holon_error.clone());
                            Err(holon_error)
                        }
                    }
                } else {
                    let holon_error = HolonError::HolonNotFound(
                        "Holon marked Changed, but has no saved_node".to_string(),
                    );
                    self.errors.push(holon_error.clone());
                    Err(holon_error)
                }
            }

            _ => {
                // No save needed for Fetched, Saved, Abandoned, or Transient, just return Holon
                debug!("Skipping commit for holon in {:#?} state", self.state);

                Ok(self.clone())
            }
        }
    }
    /// commit_relationship() saves a `Saved` holon's relationships as SmartLinks. It should only be invoked
    /// AFTER staged_holons have been successfully committed.
    ///
    /// If the staged holon is `Fetched`, `New`, or `Changed` commit does nothing.
    ///
    /// If the staged holon is `Saved`, commit_relationship iterates through the holon's
    /// `relationship_map` and calls commit on each member's HolonCollection.
    ///
    /// If all commits are successful, the function returns a clone a self. Otherwise, the
    /// function returns an error.
    ///
    pub fn commit_relationships(&mut self, context: &HolonsContext) -> Result<Holon, HolonError> {
        debug!("Entered Holon::commit_relationships");
        match self.state {
            HolonState::Saved => {
                match self.saved_node.clone() {
                    Some(record) => {
                        let source_holon_id = record.action_address().clone();
                        // Iterate through the holon's relationship map, invoking commit on each
                        for (name, holon_collection) in self.relationship_map.0.clone() {
                            debug!("COMMITTING {:#?} relationship", name.0.0.clone());
                            holon_collection.commit_relationship(
                                context,
                                HolonId::from(source_holon_id.clone()),
                                name.clone(),
                            )?;
                        }

                        Ok(self.clone())
                    }
                    None => Err(HolonError::HolonNotFound(
                        "Holon marked Saved, but has no saved_node".to_string(),
                    )),
                }
            }

            _ => {
                // Ignore all other states, just return self
                Ok(self.clone())
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

    pub fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        let key = self.get_key()?;
        Ok(EssentialHolonContent {
            property_map: self.property_map.clone(),
            //relationship_map: self.relationship_map.clone(),
            descriptor: self.descriptor.clone(),
            key,
            errors: self.errors.clone(),
        })
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


    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let node = self.saved_node.clone();
        if let Some(record) = node {
            Ok(HolonId(record.action_address().clone()))
        } else {
            Err(HolonError::HolonNotFound("Node is empty".to_string()))
        }
    }

    // NOTE: Holon does NOT  implement HolonGettableTrait because the functions defined by that
    // Trait include a context parameter.

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let key = self
            .property_map
            .get(&PropertyName(MapString("key".to_string())));
        if let Some(key) = key {
            let string_value: String = key.try_into().map_err(|_| {
                HolonError::UnexpectedValueType(format!("{:?}", key), "MapString".to_string())
            })?;
            Ok(Some(MapString(string_value)))
        } else {
            Ok(None)
        }
    }

    pub fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        self.is_accessible(AccessType::Read)?;
        self.property_map
            .get(property_name)
            .cloned()
            .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
    }
    /// This method returns a HolonCollection containing the holons (if any) that are related
    /// to the source holon via the specified relationship_name. Prior to this call, the holons
    /// for the specified relationship may or may not have been loaded. So it first ensures they
    /// have been loaded before retrieving and returning the HolonCollection for this relationship.
    ///
    /// NOTE: Even if there are no holons related via that relationship, an entry will be added to
    /// the relationship_map for that relationship (referencing a possibly empty HolonCollection).
    ///

    pub fn get_related_holons(
        &mut self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Check if the holon is accessible with the required access type
        self.is_accessible(AccessType::Read)?;
        debug!("Entered get_related_holons for source holon({:?})-{:?}>",
            self.get_key(),
            relationship_name);

        // Load the relationship and get the count
        let _count = self.load_relationship(relationship_name)?;

        // Retrieve the collection for the given relationship name
        let collection = self.relationship_map.0.get(relationship_name)
            .ok_or(HolonError::HolonNotFound(format!(
                "Even after load_relationships, no collection found for relationship: {:?}",
                relationship_name
            )))?;

        // Return the collection wrapped in a Rc
        Ok(Rc::new(collection.clone()))
    }


        /// This method gets ALL holons related to this holon via ANY relationship this holon is
        /// EITHER the SOURCE_FOR or TARGET_OF. It returns a RelationshipMap containing
        /// one entry for every relationship that has related holons. NOTE: this means that the
        /// holon collection will have at least one member for every entry in the returned map.
        ///
        /// A side effect of this function is that this holon's cached `relationship_map` will be
        /// fully loaded.
        ///
        /// TODO: Reconsider the need for this function... it is potentially very expensive
        /// TODO: Conform to *at-most-once* semantics
        ///       Currently there is no way to tell whether a previous load_all has occurred
        ///

        pub fn get_all_related_holons(
            &mut self,
        ) -> Result<RelationshipMap, HolonError> {

            Err(HolonError::NotImplemented("get_all_related_holons is not yet implemented".to_string()))


            // self.is_accessible(AccessType::Read)?;
            // // let relationship_map = self.relationship_map.clone();
            //
            // let mut result_map =
            //     self.load_all_related_holons.BTreeMap::new();
            //
            // if let Some(name) = relationship_name {
            //     // A specific relationship_name was provided, so get the related holons that are the
            //     // target of that specific relationship
            //
            //     result_map.insert(name, HolonCollection::new_existing());
            //
            //     let count = self.load_relationship(&name)?;
            //     if count.0 > 0 {
            //         // Some related holons were loaded, fetch them and add to result
            //         let collection_option = self.relationship_map.0.get(&name); // Dereference the name here
            //         return if let Some(collection) = collection_option {
            //             let mut map = BTreeMap::new();
            //             map.insert(name.clone(), collection.clone());
            //             Ok(RelationshipMap(map))
            //         } else {
            //             // No related holons, return
            //         }
            //
            //
            //         Ok(RelationshipMap(result_map))
            //     }
            // }
    }
    pub fn get_related_holons_deprecated(
        &self,
        relationship_name: Option<RelationshipName>,
    ) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let relationship_map = self.relationship_map.clone();
        if let Some(name) = relationship_name {
            let collection_option = relationship_map.0.get(&name); // Dereference the name here
            return if let Some(collection) = collection_option {
                let mut map = BTreeMap::new();
                map.insert(name.clone(), collection.clone());
                Ok(RelationshipMap(map))
            } else {
                Ok(RelationshipMap(BTreeMap::new()))
            }
        } else {
            Ok(relationship_map)
        }
    }


    /// Returns the current state of the Holon.
    ///
    /// # Semantics
    /// The state indicates the lifecycle stage of the holon, such as whether it has been fetched
    /// from the persistent store, staged for changes, saved after committing changes, or abandoned.
    ///
    /// # Usage
    /// Use this method to inspect the current state of the holon. DO NOT use this method to
    /// make decisions about whether certain operations (e.g., reading, writing, committing) are
    /// permissible. Use `is_accessible()` for this purpose instead.
    pub fn get_state(&self)->HolonState {
        self.state.clone()
    }

    pub fn into_node(self) -> HolonNode {
        HolonNode {
            property_map: self.property_map.clone(),
        }
    }

    pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.state {
            HolonState::New => match access_type {
                AccessType::Read |
                AccessType::Write |
                AccessType::Abandon |
                AccessType::Commit => Ok(()),
            },
            HolonState::Fetched => match access_type {
                AccessType::Read => Ok(()),
                AccessType::Write |
                AccessType::Abandon |
                AccessType::Commit => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state)))
                }
            }
            HolonState::Changed => match access_type {
                AccessType::Read |
                AccessType::Write |
                AccessType::Abandon |
                AccessType::Commit => Ok(()),
            }
            HolonState::Saved => match access_type {
                AccessType::Write |
                AccessType::Abandon => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state)))
                }
                AccessType::Read |
                AccessType::Commit => Ok(()),
            }
            HolonState::Abandoned => match access_type {
                AccessType::Read |
                AccessType::Write => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state)))
                }
                |
                AccessType::Commit |
                AccessType::Abandon => Ok(()),
            }

        }
    }

    // pub fn into_node(self) -> HolonNode {
    //     HolonNode {
    //         property_map: self.property_map.clone(),
    //         key,
    //         errors: self.errors.clone(),
    //     }
    // }

    /// Ensures that the holon's `relationship_map` includes an entry for the specified relationship
    /// and returns a count of the number of holons in the holon collection for the specified
    /// relationship.
    ///
    /// If the initial `get` on the relationship_map reveals there is not already an entry for the
    /// specified relationship_name, the behavior depends upon the state of the holon.
    ///
    /// For *staged* holons, an entry containing an empty HolonCollection is added to the
    /// holon's relationship_map and a count of 0 is returned.
    ///
    /// For *previously saved* holons, this function retrieves any related holons via their
    /// SmartLinks and adds an entry for the relationship to the holon's relationship map. That
    /// entry's  collection contains the retrieved holons (if any). The count of this collection
    /// is then returned.
    ///
    /// This method conforms to *at-most-once* semantics, by if the SmartLinks have already been
    /// retrieved for this relationship before retrieving them again.
    fn load_relationship(
        &mut self,
        relationship_name: &RelationshipName,
    ) -> Result<MapInteger, HolonError> {
        let relationship_entry_option = self.relationship_map.0.get(relationship_name);

        match relationship_entry_option {
            Some(collection) => Ok(collection.get_count()),
            None => {
                // No entry found for this relationship

                match self.get_state() {
                    HolonState::New => {
                        // Initialize a new holon_collection
                        let collection = HolonCollection::new_staged();

                        // Add an entry for this relationship to relationship_map
                        self.relationship_map.0.insert(relationship_name.clone(), collection.clone());
                        Ok(collection.get_count())
                    },
                    HolonState::Fetched |
                    HolonState::Changed => {
                        // Initialize a new holon_collection
                        let mut collection = HolonCollection::new_existing();

                        // fetch the smartlinks for this relationship (if any)
                        let smartlinks =
                            get_relationship_links(self.get_id()?.0, relationship_name)?;

                        for smartlink in smartlinks {
                            let holon_reference = smartlink.to_holon_reference();
                            collection.add_reference_with_key(smartlink.get_key().as_ref(), &holon_reference)?;
                        }
                        // Add an entry for this relationship to relationship_map
                        let count = collection.get_count();
                        self.relationship_map.0.insert(relationship_name.clone(), collection);
                        Ok(count)
                    }

                    _ => Err(HolonError::NotAccessible(
                        format!("{:?}", AccessType::Read), // TODO: Consider adding `LoadLinks` AccessType
                        format!("{:?}", self.state)))
                }
            }
        }
    }


    /// Populates a full RelationshipMap by retrieving all SmartLinks for which this holon is the
    /// source. The map returned will ONLY contain entries for relationships that have at least
    /// one related holon (i.e., none of the holon collections returned via the result map will have
    /// zero members).

    pub fn load_all_relationships(
        &mut self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();

        let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> =
            BTreeMap::new();
        let smartlinks = get_all_relationship_links(self.get_id()?.0)?;
        debug!("Retrieved {:?} smartlinks", smartlinks.len());

        for smartlink in smartlinks {
            let reference = smartlink.to_holon_reference();

            // The following:
            // 1) adds an entry for relationship name if not already present (via `entry` API)
            // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
            // 3) pushes the new HolonReference into the vector -- without having to clone the vector

            reference_map
                .entry(smartlink.relationship_name)
                .or_insert_with(Vec::new)
                .push(reference);
        }

        // Now create the result

        for (map_name, holons) in reference_map {
            let mut collection = HolonCollection::new_existing();
            collection.add_references(context, holons)?;
            relationship_map.insert(map_name, collection);
        }

        Ok(RelationshipMap(relationship_map))
    }

    /// This private method is used to populate a holon's descriptor field by retrieving a
    /// reference to it from the holon's relationships. This function returns:
    /// `Ok(Some(HolonReference))` -- where HolonReference refers to the retrieved descriptor
    /// `Ok(None)` -- If the holon doesn't have a related descriptor.
    /// `Err<HolonError>` -- if any errors are encountered
    fn populate_descriptor(&mut self) -> Result<Option<HolonReference>, HolonError> {
        // Define the "DESCRIBED_BY" relationship key -- TODO: get this name as Enum variant
        let relationship_name = RelationshipName(MapString("DESCRIBED_BY".into()));

        // Attempt to load the relationship and get the count of related descriptors
        let descriptor_count = self.load_relationship(&relationship_name)?;

        match descriptor_count.0 {
            0 => Ok(None),
            1 => {
                if let Some(collection) = self.relationship_map.get_collection_for_relationship(&relationship_name) {
                    let descriptor = collection.get_by_index(0)?;
                    self.descriptor = Some(descriptor.clone());
                    Ok(Some(descriptor))
                } else {
                    // This case should be unreachable since descriptor_count.0 == 1
                    Err(HolonError::HolonNotFound(format!(
                        "Descriptor expected but not found for relationship: {:?}",
                        relationship_name
                    )))
                }
            },
            _ => Err(HolonError::IndexOutOfRange("Expected only a single descriptor".to_string())),
        }
    }


    // /// This private method is used to populate a holon's descriptor field by retrieving a
    // /// reference to it from the holon's relationships. This function returns:
    // /// `Ok(Some(HolonReference))` -- where HolonReference refers to the retrieved descriptor
    // /// `Ok(None)` --If the holon doesn't have a related descriptor.
    // /// `Err<HolonError>` -- if any errors are encountered
    // fn populate_descriptor(&mut self) -> Result<Option<HolonReference>, HolonError> {
    //     // Define the "DESCRIBED_BY" relationship key -- TODO: get this name as Enum variant
    //     let relationship_name = RelationshipName(MapString("DESCRIBED_BY".into()));
    //
    //     let descriptor_count = self.load_relationship(&relationship_name)?;
    //
    //     if descriptor_count.0 == 1 {
    //         let collection_option =
    //             self.relationship_map.get_collection_for_relationship(&relationship_name);
    //         if let Some(collection) = collection_option {
    //             let descriptor = collection.get_by_index(0)?;
    //             self.descriptor = Some(descriptor.clone());
    //             Ok(Some(descriptor.clone()))
    //         }  else {
    //         if descriptor_count.0 == 0 {
    //             Ok(None)
    //         } else {
    //             Err(HolonError::IndexOutOfRange("Expected only a single descriptor".to_string()))
    //         }
    //
    //     }
    //
    // }



    /// try_from_node inflates a Holon from a HolonNode.
    /// Since Implemented here to avoid conflicts with hdk::core's implementation of TryFrom Trait
    pub fn try_from_node(holon_node_record: Record) -> Result<Holon, HolonError> {
        let holon_node = get_holon_node_from_record(holon_node_record.clone())?;

        let mut holon = Holon {
            state: HolonState::Fetched,
            validation_state: ValidationState::Validated,
            saved_node: Some(holon_node_record),
            predecessor: None,
            property_map: holon_node.property_map,
            relationship_map: RelationshipMap::new(),
            descriptor: None,
            errors: Vec::new(),
        };

        // TODO: Populate RelationshipMap from links

        // TODO: Populate Descriptor from links
        holon.populate_descriptor()?;

        // TODO: populate predecessor from link to previous record for this Holon

        // TODO: populate `key` from the property map once we have Descriptors/Constraints available



        Ok(holon)
    }
    // NOTE: this function doesn't check if supplied PropertyName is a valid property
    // for the self holon. It probably needs to be possible to suspend
    // this checking while the type system is being bootstrapped, since the descriptors
    // required by the validation may not yet exist.
    // TODO: Add conditional validation checking when adding properties
    // TODO: add error checking and HolonError result
    // Possible Errors: Unrecognized Property Name
    pub fn with_property_value(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.property_map.insert(property, value);
        match self.state {
            HolonState::Fetched => {
                self.state = HolonState::Changed;
            }
            _ => {}
        }
        Ok(self)
    }


}
