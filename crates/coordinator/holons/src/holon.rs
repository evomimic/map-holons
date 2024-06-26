use std::collections::BTreeMap;
use std::fmt;

use derive_new::new;
use hdi::prelude::ActionHash;

use hdk::prelude::*;

use holons_integrity::LinkTypes;
use shared_types_holon::holon_node::{HolonNode, PropertyMap, PropertyName, PropertyValue};
use shared_types_holon::{HolonId, MapString};

use shared_types_holon::value_types::BaseValue;

use crate::all_holon_nodes::*;
use crate::context::HolonsContext;
use crate::helpers::get_holon_node_from_record;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_node::UpdateHolonNodeInput;
use crate::holon_node::*;
use crate::holon_reference::HolonReference;
use crate::relationship::{RelationshipMap, RelationshipName, SmartLinkHolder};
use crate::smart_reference::SmartReference;
use crate::smartlink::{create_link_tag, get_smartlink_from_link};

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
    // pub descriptor: HolonReference,
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



pub trait HolonGettable {
    fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError>;

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError>;

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
            errors: Vec::new(),
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

    /// This function bypasses the cache (it should be retired in favor of fetch_holon once cache is implemented
    // TODO: replace with cache aware function
    // TODO: Throw None case or remove option
    pub fn get_holon(id: HolonId) -> Result<Option<Holon>, HolonError> {
        let holon_node_record = get_holon_node(id.0.clone())?;
        if let Some(node) = holon_node_record {
            let mut holon = Holon::try_from_node(node)?;
            holon.state = HolonState::Fetched;
            Ok(Some(holon))
        } else {
            // no holon_node fetched for specified holon_id
            Err(HolonError::HolonNotFound(id.0.to_string()))
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

    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let node = self.saved_node.clone();
        if let Some(record) = node {
            Ok(HolonId(record.action_address().clone()))
        } else {
            Err(HolonError::HolonNotFound("Node is empty".to_string()))
        }
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
            errors: Vec::new(),
        };

        // TODO: populate predecessor from link to previous record for this Holon

        // TODO: populate `key` from the property map once we have Descriptors/Constraints available

        // TODO: Populate RelationshipMap from links

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

    /// commit() saves a staged holon to the persistent store.
    ///
    /// If the staged holon is already  `Fetched`, `Saved`, or 'Abandoned', commit does nothing.
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
        debug!("Entered Holon::commit for holon in {:#?} state", self.state);
        match self.state {
            HolonState::New => {
                // Create a new HolonNode from this Holon and request it be created
                debug!("HolonState is New... requesting new HolonNode be created in the DHT");
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
                            debug!("COMMITTING {:#?} relationship", name.clone());
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

    pub fn essential_content(&self) -> Result<EssentialHolonContent, HolonError> {
        let key = self.get_key()?;
        Ok(EssentialHolonContent {
            property_map: self.property_map.clone(),
            //relationship_map: self.relationship_map.clone(),
            key,
            errors: self.errors.clone(),
        })
    }

    pub fn abandon_staged_changes(&mut self) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Abandon)?;

        self.state = HolonState::Abandoned;
        Ok(())

    }

}

/// Gets all relationships optionally filtered by name
pub fn get_relationship_links(
    holon_id: ActionHash,
    relationship_name: Option<RelationshipName>,
) -> Result<Vec<Link>, HolonError> {
    let link_tag: Option<LinkTag> = if let Some(name) = relationship_name {
        // smart_property_values is set to None so that no additional filters are applied and all relationships of a given name are retrieved
        Some(create_link_tag(name, None))
    } else {
        None
    };

    let links =
        get_links(holon_id, LinkTypes::SmartLink, link_tag).map_err(|e| HolonError::from(e))?;

    Ok(links)
}

/// Builds a full or partial RelationshipMap for an existing holon identified by `source_holon_id`
/// by retrieving SmartLinks for that holon.
/// If `relationship_name` is supplied, the RelationshipMap returned will only have (at most) a
/// single entry consisting of the HolonCollection for the supplied `relationship_name`.
/// Otherwise, a full RelationshipMap will be populated for the `source_holon_id`.
///
///
///
pub fn load_relationship_map(
    context: &HolonsContext,
    source_holon_id: ActionHash,
    relationship_name: Option<RelationshipName>,
) -> Result<RelationshipMap, HolonError> {
    let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
    let links = get_relationship_links(source_holon_id.clone(), relationship_name)?;

    debug!("Retrieved {:?} links from holochain", links.len());

    for link in links {
        let smartlink = get_smartlink_from_link(source_holon_id.clone(), link.clone())?;
        // let name = RelationshipName(MapString(name_string));

        let target = link.target.into_action_hash().ok_or_else(|| {
            HolonError::HashConversion("Link target".to_string(), "ActionHash".to_string())
        })?;
        let reference = HolonReference::Smart(SmartReference {
            holon_id: HolonId(target),
            smart_property_values: None, // defaulting to None until descriptors ready
        });

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

    let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();

    for (map_name, holons) in reference_map {
        let mut collection = HolonCollection::new_existing();
        collection.add_references(context, holons)?;
        relationship_map.insert(map_name, collection);
    }

    Ok(RelationshipMap(relationship_map))
}
