use crate::context::HolonsContext;
use crate::holon::{AccessType, HolonGettable};
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::relationship::RelationshipName;
use crate::smartlink::{save_smartlink, SmartLink};
use hdk::prelude::*;
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyMap, PropertyName};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum CollectionState {
    Fetched,   // links have been fetched from the persistent store for this collection
    Staged,    // the links for this collection have not been persisted
    Saved,     // a staged collection for which SmartLinks have been successfully committed
    Abandoned, // a previously staged collection that was abandoned prior to being committed
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct HolonCollection {
    state: CollectionState,
    members: Vec<HolonReference>,
    keyed_index: BTreeMap<MapString, usize>, // usize is an index into the members vector
}

impl HolonCollection {
    pub fn new_staged() -> Self {
        HolonCollection {
            state: CollectionState::Staged,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }
    pub fn new_existing() -> Self {
        HolonCollection {
            state: CollectionState::Fetched,
            members: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
        match access_type {
            AccessType::Read => match self.state {
                CollectionState::Fetched | CollectionState::Staged | CollectionState::Saved => {
                    Ok(())
                }
                CollectionState::Abandoned => Err(HolonError::NotAccessible(
                    "Read".to_string(),
                    format!("{:?}", self.state),
                )),
            },
            AccessType::Write => match self.state {
                CollectionState::Staged => Ok(()),
                _ => Err(HolonError::NotAccessible(
                    "Write".to_string(),
                    format!("{:?}", self.state),
                )),
            },
        }
    }

    // pub fn is_accessible(&self, access_type: AccessType) -> Result<(), HolonError> {
    //     match access_type {
    //         AccessType::Read => {
    //             if self.state == CollectionState::Abandoned {
    //                 Err(HolonError::NotAccessible(
    //                     "Read".to_string(),
    //                     format!("{:?}", self.state),
    //                 ))
    //             } else {
    //                 Ok(())
    //             }
    //         }
    //         AccessType::Write => match self.state {
    //             CollectionState::Staged => Ok(()),
    //             _ => Err(HolonError::NotAccessible(
    //                 "Write".to_string(),
    //                 format!("{:?}", self.state),
    //             )),
    //         },
    //     }
    // }

    pub fn to_staged(&self) -> Result<HolonCollection, HolonError> {
        self.is_accessible(AccessType::Read)?;
        if self.state == CollectionState::Fetched {
            Ok(HolonCollection {
                state: CollectionState::Staged,
                members: self.members.clone(),
                keyed_index: self.keyed_index.clone(),
            })
        } else {
            Err(HolonError::InvalidParameter("CollectionState".to_string()))
        }
    }

    pub fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let index = self.keyed_index.get(key);
        if let Some(index) = index {
            Ok(Some(self.members[*index].clone()))
        } else {
            Ok(None)
        }
    }

    pub fn add_references(
        &mut self,
        context: &HolonsContext,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        for holon in holons {
            let index = self.members.len();
            self.members.push(holon.clone());
            let key = holon.get_key(context)?;
            if let Some(key) = key {
                self.keyed_index.insert(key, index);
            }
        }

        Ok(())
    }

    /// This method creates smartlinks from the specified source_id for the specified relationship name
    /// to each holon its collection that has a holon_id.
    pub fn save_smartlinks_for_collection(
        &self,
        context: &HolonsContext,
        source_id: HolonId,
        name: RelationshipName,
    ) -> Result<(), HolonError> {
        debug!(
            "Calling commit on each HOLON_REFERENCE in the collection for {:#?}.",
            name.0.clone()
        );
        for holon_reference in &self.members {
            // Only commit references to holons with id's (i.e., Saved)
            if let Ok(target_id) = holon_reference.get_holon_id(context) {
                let key_option = holon_reference.get_key(context)?;
                let input: SmartLink = if let Some(key) = key_option {
                    let mut prop_vals: PropertyMap = BTreeMap::new();
                    prop_vals.insert(
                        PropertyName(MapString("key".to_string())),
                        BaseValue::StringValue(key),
                    );
                    SmartLink {
                        from_address: source_id.clone(),
                        to_address: target_id,
                        relationship_name: name.clone(),
                        smart_property_values: Some(prop_vals),
                    }
                } else {
                    SmartLink {
                        from_address: source_id.clone(),
                        to_address: target_id,
                        relationship_name: name.clone(),
                        smart_property_values: None,
                    }
                };

                save_smartlink(input)?;
            }
        }
        Ok(())
    }

    /// The method
    pub fn commit_relationship(
        &self,
        context: &HolonsContext,
        source_id: HolonId,
        name: RelationshipName,
    ) -> Result<(), HolonError> {
        self.save_smartlinks_for_collection(context, source_id.clone(), name.clone())?;

        Ok(())
    }
}
