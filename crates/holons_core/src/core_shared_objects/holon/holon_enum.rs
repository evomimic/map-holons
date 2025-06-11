use crate::core_shared_objects::HolonError;

use base_types::MapString;
use hdk::prelude::*;
use integrity_core_types::{HolonNode, LocalId, PropertyMap, PropertyName, PropertyValue};

use super::holon_utils::EssentialHolonContent;
use super::state::AccessType;
use super::{HolonBehavior, SavedHolon, StagedHolon, TransientHolon};

/// Enum representing the three Holon phases: `Transient`, `Staged`, and `Saved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Holon {
    Transient(TransientHolon),
    Staged(StagedHolon),
    Saved(SavedHolon),
}

// ==================================
//   ASSOCIATED METHODS (IMPL BLOCK)
// ==================================
impl Holon {
    /// Constructs a new `TransientHolon`.
    pub fn new_transient() -> Self {
        Holon::Transient(TransientHolon::new())
    }

    /// Gets inner TransientHolon object for Transient variant
    pub fn into_transient(self) -> Result<TransientHolon, HolonError> {
        match self {
            Holon::Transient(transient_holon) => Ok(transient_holon),
            _ => Err(HolonError::InvalidTransition("Holon variant must be Transient".to_string())),
        }
    }
}

// ================================
//   HOLONBEHAVIOR IMPLEMENTATION
// ================================
impl HolonBehavior for Holon {
    // ====================
    //    DATA ACCESSORS
    // ====================

    fn clone_holon(&self) -> Result<TransientHolon, HolonError> {
        match self {
            Holon::Transient(h) => h.clone_holon(),
            Holon::Staged(h) => h.clone_holon(),
            Holon::Saved(h) => h.clone_holon(),
        }
    }

    fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        match self {
            Holon::Transient(h) => h.essential_content(),
            Holon::Staged(h) => h.essential_content(),
            Holon::Saved(h) => h.essential_content(),
        }
    }

    fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        match self {
            Holon::Transient(h) => h.get_key(),
            Holon::Staged(h) => h.get_key(),
            Holon::Saved(h) => h.get_key(),
        }
    }

    fn get_local_id(&self) -> Result<LocalId, HolonError> {
        match self {
            Holon::Transient(h) => h.get_local_id(),
            Holon::Staged(h) => h.get_local_id(),
            Holon::Saved(h) => h.get_local_id(),
        }
    }

    fn get_original_id(&self) -> Option<LocalId> {
        match self {
            Holon::Transient(h) => h.get_original_id(),
            Holon::Staged(h) => h.get_original_id(),
            Holon::Saved(h) => h.get_original_id(),
        }
    }

    fn get_property_value(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            Holon::Transient(h) => h.get_property_value(property_name),
            Holon::Staged(h) => h.get_property_value(property_name),
            Holon::Saved(h) => h.get_property_value(property_name),
        }
    }

    fn get_versioned_key(&self) -> Result<MapString, HolonError> {
        match self {
            Holon::Transient(h) => h.get_versioned_key(),
            Holon::Staged(h) => h.get_versioned_key(),
            Holon::Saved(h) => h.get_versioned_key(),
        }
    }

    fn into_node(&self) -> HolonNode {
        match self {
            Holon::Transient(h) => h.into_node(),
            Holon::Staged(h) => h.into_node(),
            Holon::Saved(h) => h.into_node(),
        }
    }

    // =================
    //     MUTATORS
    // =================

    /// Updates the Holon's original id.
    fn update_original_id(&mut self, id: Option<LocalId>) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.update_original_id(id),
            Holon::Staged(h) => h.update_original_id(id),
            Holon::Saved(h) => h.update_original_id(id),
        }
    }

    /// Updates the Holon's PropertyMap.
    fn update_property_map(&mut self, map: PropertyMap) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.update_property_map(map),
            Holon::Staged(h) => h.update_property_map(map),
            Holon::Saved(h) => h.update_property_map(map),
        }
    }

    fn increment_version(&mut self) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.increment_version(),
            Holon::Staged(h) => h.increment_version(),
            Holon::Saved(h) => h.increment_version(),
        }
    }

    // ======================
    //     ACCESS CONTROL
    // ======================

    fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self {
            Holon::Transient(h) => h.is_accessible(access_type),
            Holon::Staged(h) => h.is_accessible(access_type),
            Holon::Saved(h) => h.is_accessible(access_type),
        }
    }

    // =====================
    //      DIAGNOSTICS
    // =====================

    fn debug_info(&self) -> String {
        match self {
            Holon::Transient(h) => h.debug_info(),
            Holon::Staged(h) => h.debug_info(),
            Holon::Saved(h) => h.debug_info(),
        }
    }

    // ===============
    //     HELPERS
    // ===============

    fn summarize(&self) -> String {
        match self {
            Holon::Transient(h) => h.summarize(),
            Holon::Staged(h) => h.summarize(),
            Holon::Saved(h) => h.summarize(),
        }
    }
}

// impl Holon {
//     // CONSTRUCTORS //

//     /// Stages a new empty holon.
//     pub fn new() -> Holon {
//         Holon {
//             version_sequence_count: MapInteger(1),
//             state: HolonState::New,
//             validation_state: ValidationState::NoDescriptor,
//             original_id: None,
//             record: None,
//             property_map: PropertyMap::new(),
//             staged_relationship_map: StagedRelationshipMap::new(),
//             errors: Vec::new(),
//         }
//     }

//     /// Clones a new version of the self Holon, that can be staged for building and eventual commit.
//     /// The clone retains lineage to its predecessor. If self has an original id, it is copied into
//     /// the cloned version. Otherwise, the cloned holon's original_id is set to self's action_hash
//     pub fn new_version(&self) -> Result<Holon, HolonError> {
//         trace!("Entering Holon::new_version, here is the Holon before cloning: {:#?}", self);
//         let mut holon = self.clone_holon()?;
//         holon.state = HolonState::Changed;
//         let original_id = self.get_original_id()?;
//         if original_id.is_some() {
//             holon.set_original_id(original_id)?;
//         } else {
//             holon.set_original_id(Some(self.get_local_id()?))?;
//         }

//         Ok(holon)
//     }

//     // METHODS //

//     pub fn abandon_staged_changes(&mut self) -> Result<(), HolonError> {
//         self.is_accessible(AccessType::Abandon)?;

//         self.state = HolonState::Abandoned;
//         Ok(())
//     }

// /// Clone an existing Holon and return a Holon that can be staged for building and eventual commit.
// pub fn clone_holon(&self) -> Result<Holon, HolonError> {
//     let mut holon = Holon::new();

//     // Retain the record Option
//     holon.record = self.record.clone();

//     // Copy the existing holon's PropertyMap into the new Holon
//     holon.property_map = self.property_map.clone();

//     // Update in place each relationship's HolonCollection State to Staged
//     holon.staged_relationship_map = self.staged_relationship_map.clone_for_new_source()?;

//     Ok(holon)
// }

//     #[deprecated]
//     pub fn get_all_holons() -> Result<Vec<Holon>, HolonError> {
//         Err(HolonError::NotImplemented("get_all_holons is no longer supported".to_string()))
//     }

//     // /// This method gets ALL holons related to this holon via ANY relationship this holon is
//     // /// EITHER the SOURCE_FOR or TARGET_OF. It returns a RelationshipMap containing
//     // /// one entry for every relationship that has related holons. NOTE: this means that the
//     // /// holon collection will have at least one member for every entry in the returned map.
//     // ///
//     // /// A side effect of this function is that this holon's cached `relationship_map` will be
//     // /// fully loaded.
//     // ///
//     // /// TODO: Reconsider the need for this function... it is potentially very expensive
//     // /// TODO: Conform to *at-most-once* semantics
//     // ///       Currently there is no way to tell whether a previous load_all has occurred
//     // ///
//     //
//     // pub fn get_all_related_holonsDEPRECATED(
//     //     &mut self,
//     // ) -> Result<StagedRelationshipMap, HolonError> {
//     //     Err(HolonError::NotImplemented("get_all_related_holons is not yet implemented".to_string()))

//     // self.is_accessible(AccessType::Read)?;
//     // // let relationship_map = self.relationship_map.clone();
//     //
//     // let mut result_map =
//     //     self.load_all_related_holons.BTreeMap::new();
//     //
//     // if let Some(name) = relationship_name {
//     //     // A specific relationship_name was provided, so get the related holons that are the
//     //     // target of that specific relationship
//     //
//     //     result_map.insert(name, HolonCollection::new_existing());
//     //
//     //     let count = self.load_relationship(&name)?;
//     //     if count.0 > 0 {
//     //         // Some related holons were loaded, fetch them and add to result
//     //         let collection_option = self.relationship_map.0.get(&name); // Dereference the name here
//     //         return if let Some(collection) = collection_option {
//     //             let mut map = BTreeMap::new();
//     //             map.insert(name.clone(), collection.clone());
//     //             Ok(RelationshipMap(map))
//     //         } else {
//     //             // No related holons, return
//     //         }
//     //
//     //
//     //         Ok(RelationshipMap(result_map))
//     //     }
//     // }
//     // }

//     // NOTE: Holon does NOT  implement HolonReadable Trait because the functions defined by that
//     // Trait include a context parameter.

//     pub fn get_property_value(
//         &self,
//         property_name: &PropertyName,
//     ) -> Result<Option<PropertyValue>, HolonError> {
//         self.is_accessible(AccessType::Read)?;
//         self.property_map
//             .get(property_name)
//             .cloned()
//             .ok_or_else(|| HolonError::EmptyField(property_name.to_string()))
//     }
//     /// **NOTE: This method is only intended for Staged Holons**
//     /// This method retrieves the HolonCollection for the specified relationship from the
//     /// `staged_relationship_map`. If there is no entry for the specified relationship, it
//     /// returns an empty HolonCollection
//     pub fn get_staged_relationship(
//         &self,
//         relationship_name: &RelationshipName,
//     ) -> Result<Rc<HolonCollection>, HolonError> {
//         // Check if the holon is accessible with the required access type
//         self.is_accessible(AccessType::Read)?;

//         // Retrieve the collection for the given relationship name or return an empty collection
//         let collection = self.staged_relationship_map.get_related_holons(relationship_name);

//         // Wrap the collection in an Rc and return
//         Ok(collection)
//     }

//     /// Returns the current state of the Holon.
//     ///
//     /// # Semantics
//     /// The state indicates the lifecycle stage of the holon, such as whether it has been fetched
//     /// from the persistent store, staged for changes, saved after committing changes, or abandoned.
//     ///
//     /// # Usage
//     /// Use this method to inspect the current state of the holon. DO NOT use this method to
//     /// make decisions about whether certain operations (e.g., reading, writing, committing) are
//     /// permissible. Use `is_accessible()` for this purpose instead.
//     pub fn get_state(&self) -> HolonState {
//         self.state.clone()
//     }

//     pub fn get_versioned_key(&self) -> Result<MapString, HolonError> {
//         let key = self
//             .get_key()?
//             .ok_or(HolonError::InvalidParameter("Holon must have a key".to_string()))?;

//         Ok(MapString(key.0 + &self.version_sequence_count.0.to_string()))
//     }

//     pub fn into_node(self) -> HolonNode {
//         HolonNode { original_id: self.original_id.clone(), property_map: self.property_map.clone() }
//     }

//     pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
//         match self.state {
//             HolonState::Abandoned => match access_type {
//                 AccessType::Abandon | AccessType::Commit | AccessType::Read => Ok(()),
//                 AccessType::Clone | AccessType::Write => Err(HolonError::NotAccessible(
//                     format!("{:?}", access_type),
//                     format!("{:?}", self.state),
//                 )),
//             },
//             HolonState::Changed => match access_type {
//                 AccessType::Abandon
//                 | AccessType::Clone
//                 | AccessType::Commit
//                 | AccessType::Read
//                 | AccessType::Write => Ok(()),
//             },
//             HolonState::Fetched => match access_type {
//                 AccessType::Clone | AccessType::Read | AccessType::Write => Ok(()), // Write access is ok for cached Holons
//                 AccessType::Abandon | AccessType::Commit => Err(HolonError::NotAccessible(
//                     format!("{:?}", access_type),
//                     format!("{:?}", self.state),
//                 )),
//             },
//             HolonState::New => match access_type {
//                 AccessType::Abandon
//                 | AccessType::Clone
//                 | AccessType::Commit
//                 | AccessType::Read
//                 | AccessType::Write => Ok(()),
//             },
//             HolonState::Saved => match access_type {
//                 AccessType::Read | AccessType::Commit => Ok(()),
//                 AccessType::Abandon | AccessType::Clone | AccessType::Write => {
//                     Err(HolonError::NotAccessible(
//                         format!("{:?}", access_type),
//                         format!("{:?}", self.state),
//                     ))
//                 }
//             },
//         }
//     }

//     // pub fn into_node(self) -> HolonNode {
//     //     HolonNode {
//     //         property_map: self.property_map.clone(),
//     //         key,
//     //         errors: self.errors.clone(),
//     //     }
//     // }

//     pub fn is_deletable(&mut self) -> Result<(), HolonError> {
//         // This method should be moved outside of Holon where cached relationships can be accessed

//         // let related_holons = self.get_all_related_holons()?;
//         // if !related_holons.0.is_empty() {
//         //     let relationships = related_holons
//         //         .0
//         //         .keys()
//         //         .map(|name| name.0 .0.clone())
//         //         .collect::<Vec<String>>()
//         //         .join(", ");

//         //     Err(HolonError::DeletionNotAllowed(relationships))
//         // } else {
//         //     Ok(())
//         // }
//         Ok(()) // always return Ok until support for get_all_related_holons
//     }

//     /// Populates a full RelationshipMap by retrieving all SmartLinks for which this holon is the
//     /// source. The map returned will ONLY contain entries for relationships that have at least
//     /// one related holon (i.e., none of the holon collections returned via the result map will have
//     /// zero members).
//     // pub fn fetch_all_related_holons(
//     //     &mut self,
//     //     context: &dyn HolonsContextBehavior,
//     // ) -> Result<(), HolonError> {
//     //     debug!("Loading all relationships...");
//     //     let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();
//     //
//     //     let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
//     //     let smartlinks = get_all_relationship_links(self.get_local_id()?)?;
//     //     debug!("Retrieved {:?} smartlinks", smartlinks.len());
//     //
//     //     for smartlink in smartlinks {
//     //         let reference = smartlink.to_holon_reference();
//     //
//     //         // The following:
//     //         // 1) adds an entry for relationship name if not already present (via `entry` API)
//     //         // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
//     //         // 3) pushes the new HolonReference into the vector -- without having to clone the vector
//     //
//     //         reference_map
//     //             .entry(smartlink.relationship_name)
//     //             .or_insert_with(Vec::new)
//     //             .push(reference);
//     //     }
//     //
//     //     // Now create the result
//     //
//     //     for (map_name, holons) in reference_map {
//     //         let mut collection = HolonCollection::new_existing();
//     //         collection.add_references(context, holons)?;
//     //         relationship_map.insert(map_name, collection);
//     //     }
//     //     self.relationship_map = RelationshipMap(relationship_map);
//     //
//     //     Ok(())
//     // }

//     /// Ensures that the holon's `relationship_map` includes an entry for the specified relationship
//     /// and returns a count of the number of holons in the holon collection for the specified
//     /// relationship.
//     ///
//     /// If the initial `get` on the relationship_map reveals there is not already an entry for the
//     /// specified relationship_name, the behavior depends upon the state of the holon.
//     ///
//     /// For *staged* holons, an entry containing an empty HolonCollection is added to the
//     /// holon's relationship_map and a count of 0 is returned.
//     ///
//     /// For *previously saved* holons, this function retrieves any related holons via their
//     /// SmartLinks and adds an entry for the relationship to the holon's relationship map. That
//     /// entry's  collection contains the retrieved holons (if any). The count of this collection
//     /// is then returned.
//     ///
//     /// This method conforms to *at-most-once* semantics, by if the SmartLinks have already been
//     /// retrieved for this relationship before retrieving them again.
//     // fn load_relationship(
//     //     &mut self,
//     //     relationship_name: &RelationshipName,
//     // ) -> Result<MapInteger, HolonError> {
//     //     let relationship_entry_option = self.relationship_map.0.get(relationship_name);
//     //
//     //     match relationship_entry_option {
//     //         Some(collection) => Ok(collection.get_count()),
//     //         None => {
//     //             // No entry found for this relationship
//     //
//     //             match self.get_state() {
//     //                 HolonState::New | HolonState::Changed => {
//     //                     // Initialize a new holon_collection
//     //                     let collection = HolonCollection::new_staged();
//     //
//     //                     // Add an entry for this relationship to relationship_map
//     //                     self.relationship_map
//     //                         .0
//     //                         .insert(relationship_name.clone(), collection.clone());
//     //                     Ok(collection.get_count())
//     //                 }
//     //                 HolonState::Fetched => {
//     //                     //Initialize a new holon_collection
//     //                     let mut collection = HolonCollection::new_existing();
//     //
//     //                     // fetch the smartlinks for this relationship (if any)
//     //                     let smartlinks =
//     //                         get_relationship_links(self.get_local_id()?.0, relationship_name)?;
//     //                     debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);
//     //
//     //                     for smartlink in smartlinks {
//     //                         let holon_reference = smartlink.to_holon_reference();
//     //                         collection.add_reference_with_key(
//     //                             smartlink.get_key().as_ref(),
//     //                             &holon_reference,
//     //                         )?;
//     //                     }
//     //                     //Add an entry for this relationship to relationship_map
//     //                     let count = collection.get_count();
//     //                     debug!("Created Collection: {:#?}", collection);
//     //                     self.relationship_map.0.insert(relationship_name.clone(), collection);
//     //                     Ok(count)
//     //                 }
//     //
//     //                 _ => Err(HolonError::NotAccessible(
//     //                     format!("{:?}", AccessType::Read), // TODO: Consider adding `LoadLinks` AccessType
//     //                     format!("{:?}", self.state),
//     //                 )),
//     //             }
//     //         }
//     //     }
//     // }

//     pub fn set_original_id(&mut self, original_id: Option<LocalId>) -> Result<(), HolonError> {
//         self.is_accessible(AccessType::Write)?;
//         self.original_id = original_id;
//         Ok(())
//     }

//     // Returns a String summary of the Holon
//     pub fn summarize(&self) -> String {
//         // Attempt to extract key from the property_map (if present), default to "None" if not available
//         let key = match self.get_key() {
//             Ok(Some(key)) => key.0,           // Extract the key from MapString
//             Ok(None) => "<None>".to_string(), // Key is None
//             Err(_) => "<Error>".to_string(),  // Error encountered while fetching key
//         };

//         // Attempt to extract local_id using get_local_id method, default to "None" if not available
//         let local_id = match self.get_local_id() {
//             Ok(local_id) => local_id.0.to_string(), // Convert LocalId to String
//             Err(_) => "<None>".to_string(),         // If local_id is not found or error occurred
//         };

//         // Format the summary string
//         format!(
//             "Holon {{ key: {}, local_id: {}, state: {}, validation_state: {:?} }}",
//             key, local_id, self.state, self.validation_state
//         )
//     }

//     /// try_from_node inflates a Holon from a HolonNode.
//     /// Since Implemented here to avoid conflicts with hdk::core's implementation of TryFrom Trait
//     pub fn try_from_node(holon_node_record: Record) -> Result<Holon, HolonError> {
//         let holon_node = get_holon_node_from_record(holon_node_record.clone())?;

//         let original_id = Some(match holon_node.original_id {
//             Some(id) => id,
//             None => LocalId(holon_node_record.action_address().clone()),
//         });

//         let holon = Holon {
//             version_sequence_count: MapInteger(1),
//             state: HolonState::Fetched,
//             validation_state: ValidationState::Validated,
//             original_id,
//             record: Some(holon_node_record),
//             property_map: holon_node.property_map,
//             staged_relationship_map: StagedRelationshipMap::new(),
//             errors: Vec::new(),
//         };

//         // TODO: Populate Descriptor from links

//         // TODO: populate predecessor from link to previous record for this Holon

//         // TODO: populate `key` from the property map once we have Descriptors/Constraints available

//         Ok(holon)
//     }

//     // NOTE: this function doesn't check if supplied PropertyName is a valid property
//     // for the self holon. It probably needs to be possible to suspend
//     // this checking while the type system is being bootstrapped, since the descriptors
//     // required by the validation may not yet exist.
//     // TODO: Add conditional validation checking when adding properties
//     // TODO: add error checking and HolonError result
//     // Possible Errors: Unrecognized Property Name
//     pub fn with_property_value(
//         &mut self,
//         property: PropertyName,
//         value: Option<BaseValue>,
//     ) -> Result<&mut Self, HolonError> {
//         self.is_accessible(AccessType::Write)?;
//         self.property_map.insert(property, value);
//         match self.state {
//             HolonState::Fetched => {
//                 self.state = HolonState::Changed;
//             }
//             _ => {}
//         }
//         Ok(self)
//     }
// }
