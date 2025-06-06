use crate::core_shared_objects::{
    HolonCollection, HolonError, RelationshipName, StagedRelationshipMap,
};
use hdk::prelude::*;
use integrity_core_types::{HolonNode, LocalId, PropertyName, PropertyValue, PropertyMap};
use base_types::{BaseValue, MapInteger, MapString};
use std::fmt;
use std::rc::Rc;

#[derive(Debug)]
pub enum AccessType {
    Abandon,
    Clone,
    Commit,
    Read,
    Write,
}
impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessType::Abandon => write!(f, "Abandon"),
            AccessType::Clone => write!(f, "Clone"),
            AccessType::Commit => write!(f, "Commit"),
            AccessType::Read => write!(f, "Read"),
            AccessType::Write => write!(f, "Write"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Holon {
    pub version_sequence_count: MapInteger, // used to add to hash content for creating TemporaryID
    pub state: HolonState,                  // only relevant for staged holons
    pub validation_state: ValidationState,  // only relevant for staged holons
    original_id: Option<LocalId>,
    pub saved_node: Option<Record>, // The last saved state of HolonNode. None = not yet created
    pub property_map: PropertyMap,
    pub staged_relationship_map: StagedRelationshipMap, // only populated for staged holons
    // pub holon_space: HolonReference,
    // pub dancer : Dancer,
    pub errors: Vec<HolonError>, // only relevant for staged holons
}

/// Type used for testing in order to match the essential content of a Holon
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EssentialHolonContent {
    pub property_map: PropertyMap,
    // pub relationship_map: RelationshipMap,
    key: Option<MapString>,
    pub errors: Vec<HolonError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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

#[derive(Debug, Clone)]
pub struct HolonSummary {
    pub key: Option<String>,
    pub local_id: Option<String>,
    pub state: HolonState,
    pub validation_state: ValidationState,
}

impl fmt::Display for HolonSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "HolonSummary {{ key: {:?}, local_id: {:?}, state: {}, validation_state: {:?} }}",
            self.key, self.local_id, self.state, self.validation_state,
        )
    }
}

impl Holon {
    // CONSTRUCTORS //

    /// Stages a new empty holon.
    pub fn new() -> Holon {
        Holon {
            version_sequence_count: MapInteger(1),
            state: HolonState::New,
            validation_state: ValidationState::NoDescriptor,
            original_id: None,
            saved_node: None,
            property_map: PropertyMap::new(),
            staged_relationship_map: StagedRelationshipMap::new(),
            errors: Vec::new(),
        }
    }

    /// Clones a new version of the self Holon, that can be staged for building and eventual commit.
    /// The clone retains lineage to its predecessor. If self has an original id, it is copied into
    /// the cloned version. Otherwise, the cloned holon's original_id is set to self's action_hash
    pub fn new_version(&self) -> Result<Holon, HolonError> {
        trace!("Entering Holon::new_version, here is the Holon before cloning: {:#?}", self);
        let mut holon = self.clone_holon()?;
        holon.state = HolonState::Changed;
        let original_id = self.get_original_id()?;
        if original_id.is_some() {
            holon.set_original_id(original_id)?;
        } else {
            holon.set_original_id(Some(self.get_local_id()?))?;
        }

        Ok(holon)
    }

    // METHODS //

    pub fn abandon_staged_changes(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Abandon)?;

        self.state = HolonState::Abandoned;
        Ok(())
    }

    /// Clone an existing Holon and return a Holon that can be staged for building and eventual commit.
    pub fn clone_holon(&self) -> Result<Holon, HolonError> {
        let mut holon = Holon::new();

        // Retain the saved_node Option
        holon.saved_node = self.saved_node.clone();

        // Copy the existing holon's PropertyMap into the new Holon
        holon.property_map = self.property_map.clone();

        // Update in place each relationship's HolonCollection State to Staged
        holon.staged_relationship_map = self.staged_relationship_map.clone_for_new_source()?;

        Ok(holon)
    }

    pub fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        let key = self.get_key()?;
        Ok(EssentialHolonContent {
            property_map: self.property_map.clone(),
            //relationship_map: self.relationship_map.clone(),
            key,
            errors: self.errors.clone(),
        })
    }

    #[deprecated]
    pub fn get_all_holons() -> Result<Vec<Holon>, HolonError> {
        Err(HolonError::NotImplemented("get_all_holons is no longer supported".to_string()))
    }

    // /// This method gets ALL holons related to this holon via ANY relationship this holon is
    // /// EITHER the SOURCE_FOR or TARGET_OF. It returns a RelationshipMap containing
    // /// one entry for every relationship that has related holons. NOTE: this means that the
    // /// holon collection will have at least one member for every entry in the returned map.
    // ///
    // /// A side effect of this function is that this holon's cached `relationship_map` will be
    // /// fully loaded.
    // ///
    // /// TODO: Reconsider the need for this function... it is potentially very expensive
    // /// TODO: Conform to *at-most-once* semantics
    // ///       Currently there is no way to tell whether a previous load_all has occurred
    // ///
    //
    // pub fn get_all_related_holonsDEPRECATED(
    //     &mut self,
    // ) -> Result<StagedRelationshipMap, HolonError> {
    //     Err(HolonError::NotImplemented("get_all_related_holons is not yet implemented".to_string()))

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
    // }

    // NOTE: Holon does NOT  implement HolonReadable Trait because the functions defined by that
    // Trait include a context parameter.

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon.
    /// If key cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(AccessType::Read)?;

        if let Some(Some(inner_value)) =
            self.property_map.get(&PropertyName(MapString("key".to_string())))
        {
            let string_value: String = inner_value.try_into().map_err(|_| {
                HolonError::UnexpectedValueType(
                    format!("{:?}", inner_value),
                    "MapString".to_string(),
                )
            })?;
            Ok(Some(MapString(string_value)))
        } else {
            trace!("Key 'key' either missing or has a None value.");
            Ok(None)
        }
    }

    pub fn get_local_id(&self) -> Result<LocalId, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let node = self.saved_node.clone();
        if let Some(record) = node {
            Ok(LocalId(record.action_address().clone()))
        } else {
            Err(HolonError::HolonNotFound("Node is empty".to_string()))
        }
    }

    pub fn get_original_id(&self) -> Result<Option<LocalId>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        Ok(self.original_id.clone())
    }

    pub fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        self.property_map
            .get(property_name)
            .cloned()
            .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
    }
    /// **NOTE: This method is only intended for Staged Holons**
    /// This method retrieves the HolonCollection for the specified relationship from the
    /// `staged_relationship_map`. If there is no entry for the specified relationship, it
    /// returns an empty HolonCollection
    pub fn get_staged_relationship(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Check if the holon is accessible with the required access type
        self.is_accessible(AccessType::Read)?;

        // Retrieve the collection for the given relationship name or return an empty collection
        let collection = self.staged_relationship_map.get_related_holons(relationship_name);

        // Wrap the collection in an Rc and return
        Ok(collection)
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
    pub fn get_state(&self) -> HolonState {
        self.state.clone()
    }

    pub fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        let key = self
            .get_key()?
            .ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

        Ok(MapString(key.0 + &self.version_sequence_count.0.to_string()))
    }

    pub fn into_node(self) -> HolonNode {
        HolonNode { original_id: self.original_id.clone(), property_map: self.property_map.clone() }
    }

    pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self.state {
            HolonState::Abandoned => match access_type {
                AccessType::Abandon | AccessType::Commit | AccessType::Read => Ok(()),
                AccessType::Clone | AccessType::Write => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            HolonState::Changed => match access_type {
                AccessType::Abandon
                | AccessType::Clone
                | AccessType::Commit
                | AccessType::Read
                | AccessType::Write => Ok(()),
            },
            HolonState::Fetched => match access_type {
                AccessType::Clone | AccessType::Read | AccessType::Write => Ok(()), // Write access is ok for cached Holons
                AccessType::Abandon | AccessType::Commit => Err(HolonError::NotAccessible(
                    format!("{:?}", access_type),
                    format!("{:?}", self.state),
                )),
            },
            HolonState::New => match access_type {
                AccessType::Abandon
                | AccessType::Clone
                | AccessType::Commit
                | AccessType::Read
                | AccessType::Write => Ok(()),
            },
            HolonState::Saved => match access_type {
                AccessType::Read | AccessType::Commit => Ok(()),
                AccessType::Abandon | AccessType::Clone | AccessType::Write => {
                    Err(HolonError::NotAccessible(
                        format!("{:?}", access_type),
                        format!("{:?}", self.state),
                    ))
                }
            },
        }
    }

    // pub fn into_node(self) -> HolonNode {
    //     HolonNode {
    //         property_map: self.property_map.clone(),
    //         key,
    //         errors: self.errors.clone(),
    //     }
    // }

    pub fn is_deletable(&mut self) -> Result<(), HolonError> {
        // This method should be moved outside of Holon where cached relationships can be accessed

        // let related_holons = self.get_all_related_holons()?;
        // if !related_holons.0.is_empty() {
        //     let relationships = related_holons
        //         .0
        //         .keys()
        //         .map(|name| name.0 .0.clone())
        //         .collect::<Vec<String>>()
        //         .join(", ");

        //     Err(HolonError::DeletionNotAllowed(relationships))
        // } else {
        //     Ok(())
        // }
        Ok(()) // always return Ok until support for get_all_related_holons
    }

    /// Populates a full RelationshipMap by retrieving all SmartLinks for which this holon is the
    /// source. The map returned will ONLY contain entries for relationships that have at least
    /// one related holon (i.e., none of the holon collections returned via the result map will have
    /// zero members).
    // pub fn fetch_all_related_holons(
    //     &mut self,
    //     context: &dyn HolonsContextBehavior,
    // ) -> Result<(), HolonError> {
    //     debug!("Loading all relationships...");
    //     let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();
    //
    //     let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
    //     let smartlinks = get_all_relationship_links(self.get_local_id()?)?;
    //     debug!("Retrieved {:?} smartlinks", smartlinks.len());
    //
    //     for smartlink in smartlinks {
    //         let reference = smartlink.to_holon_reference();
    //
    //         // The following:
    //         // 1) adds an entry for relationship name if not already present (via `entry` API)
    //         // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
    //         // 3) pushes the new HolonReference into the vector -- without having to clone the vector
    //
    //         reference_map
    //             .entry(smartlink.relationship_name)
    //             .or_insert_with(Vec::new)
    //             .push(reference);
    //     }
    //
    //     // Now create the result
    //
    //     for (map_name, holons) in reference_map {
    //         let mut collection = HolonCollection::new_existing();
    //         collection.add_references(context, holons)?;
    //         relationship_map.insert(map_name, collection);
    //     }
    //     self.relationship_map = RelationshipMap(relationship_map);
    //
    //     Ok(())
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
    // fn load_relationship(
    //     &mut self,
    //     relationship_name: &RelationshipName,
    // ) -> Result<MapInteger, HolonError> {
    //     let relationship_entry_option = self.relationship_map.0.get(relationship_name);
    //
    //     match relationship_entry_option {
    //         Some(collection) => Ok(collection.get_count()),
    //         None => {
    //             // No entry found for this relationship
    //
    //             match self.get_state() {
    //                 HolonState::New | HolonState::Changed => {
    //                     // Initialize a new holon_collection
    //                     let collection = HolonCollection::new_staged();
    //
    //                     // Add an entry for this relationship to relationship_map
    //                     self.relationship_map
    //                         .0
    //                         .insert(relationship_name.clone(), collection.clone());
    //                     Ok(collection.get_count())
    //                 }
    //                 HolonState::Fetched => {
    //                     //Initialize a new holon_collection
    //                     let mut collection = HolonCollection::new_existing();
    //
    //                     // fetch the smartlinks for this relationship (if any)
    //                     let smartlinks =
    //                         get_relationship_links(self.get_local_id()?.0, relationship_name)?;
    //                     debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);
    //
    //                     for smartlink in smartlinks {
    //                         let holon_reference = smartlink.to_holon_reference();
    //                         collection.add_reference_with_key(
    //                             smartlink.get_key().as_ref(),
    //                             &holon_reference,
    //                         )?;
    //                     }
    //                     //Add an entry for this relationship to relationship_map
    //                     let count = collection.get_count();
    //                     debug!("Created Collection: {:#?}", collection);
    //                     self.relationship_map.0.insert(relationship_name.clone(), collection);
    //                     Ok(count)
    //                 }
    //
    //                 _ => Err(HolonError::NotAccessible(
    //                     format!("{:?}", AccessType::Read), // TODO: Consider adding `LoadLinks` AccessType
    //                     format!("{:?}", self.state),
    //                 )),
    //             }
    //         }
    //     }
    // }

    pub fn set_original_id(&mut self, original_id: Option<LocalId>) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        self.original_id = original_id;
        Ok(())
    }

    // Returns a String summary of the Holon
    pub fn summarize(&self) -> String {
        // Attempt to extract key from the property_map (if present), default to "None" if not available
        let key = match self.get_key() {
            Ok(Some(key)) => key.0,           // Extract the key from MapString
            Ok(None) => "<None>".to_string(), // Key is None
            Err(_) => "<Error>".to_string(),  // Error encountered while fetching key
        };

        // Attempt to extract local_id using get_local_id method, default to "None" if not available
        let local_id = match self.get_local_id() {
            Ok(local_id) => local_id.0.to_string(), // Convert LocalId to String
            Err(_) => "<None>".to_string(),         // If local_id is not found or error occurred
        };

        // Format the summary string
        format!(
            "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
            key, local_id, self.state, self.validation_state
        )
    }

    /// try_from_node inflates a Holon from a HolonNode.
    /// Since Implemented here to avoid conflicts with hdk::core's implementation of TryFrom Trait
    pub fn try_from_node(holon_node_record: Record) -> Result<Holon, HolonError> {
        let holon_node = get_holon_node_from_record(holon_node_record.clone())?;

        let original_id = Some(match holon_node.original_id {
            Some(id) => id,
            None => LocalId(holon_node_record.action_address().clone()),
        });

        let holon = Holon {
            version_sequence_count: MapInteger(1),
            state: HolonState::Fetched,
            validation_state: ValidationState::Validated,
            original_id,
            saved_node: Some(holon_node_record),
            property_map: holon_node.property_map,
            staged_relationship_map: StagedRelationshipMap::new(),
            errors: Vec::new(),
        };

        // TODO: Populate Descriptor from links

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
        value: Option<BaseValue>,
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
fn get_holon_node_from_record(record: Record) -> Result<HolonNode, HolonError> {
    match record.entry() {
        RecordEntry::Present(entry) => HolonNode::try_from(entry.clone())
            .or(Err(HolonError::RecordConversion("HolonNode".to_string()))),
        _ => Err(HolonError::RecordConversion("Record does not have an entry".to_string())),
    }
}
