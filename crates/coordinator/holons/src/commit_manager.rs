use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use hdk::prelude::*;

// use crate::cache_manager::HolonCacheManager;
use crate::context::HolonsContext;
use crate::holon::{Holon, HolonState};
use crate::holon_error::HolonError;
use crate::relationship::RelationshipMap;
use crate::relationship::RelationshipTarget;
use crate::smart_reference::SmartReference;
use crate::staged_reference::StagedReference;
use shared_types_holon::{HolonId, MapInteger, MapString};
use crate::holon_reference::HolonReference;
use crate::smart_collection::SmartCollection;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitManager {
    pub staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    pub index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}
/// a StagedIndex identifies a StagedHolon by its position within the staged_holons vector
pub type StagedIndex = MapInteger;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitResponse {
    pub status: CommitRequestStatus,
    pub commits_attempted: MapInteger,
    pub saved_holons:Vec<Holon>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
/// *Complete* means all staged holons have been committed and staged_holons cleared
///
/// *Incomplete* means one or more of the staged_holons could not be committed.
/// For details, iterate through the staged_holons vector.
/// Holon's with a `Saved` status have been committed,
/// Holon's with a `New` or `Changed` state had error(s), see the Holon's errors vector for details
pub enum CommitRequestStatus {
    Complete,
    Incomplete,
}

impl CommitManager {
    /// This function attempts to persist the state of all `staged_holons`.
    ///
    /// If ALL commits succeed, it clears all staged objects and returns a `Complete` status
    ///
    /// Otherwise, it returns an `Incomplete` status and preserves its staged_holons so the caller
    /// can see which holons were saved and what errors were encountered for those that were not
    /// saved.
    ///
    /// NOTE: The CommitResponse returns clones of any successfully
    /// committed holons, even if the response status is `Incomplete`.
    ///
    pub fn commit(&mut self, context: &HolonsContext) -> CommitResponse {
        trace!("Entering commit...");

        // Initialize the request_status to Complete, assuming all commits will succeed
        // If any commit errors are encountered, reset request_status to `Incomplete`

        let mut response = CommitResponse {
            status: CommitRequestStatus::Complete,
            commits_attempted: MapInteger(self.staged_holons.len() as i64),
            saved_holons: Vec::new(),
        };
                // Invoke commit on each of the staged_holons
        // If successful, add an "unloaded" SmartReference to it to saved_holons
        // Otherwise, set request_status to Incomplete
        for rc_holon in self.staged_holons.clone() {
            let outcome = rc_holon.borrow_mut().clone().commit(context);
            match outcome {
                Ok(holon)=> {
                    response.saved_holons.push(holon);
                }
                Err(_error)=> {
                    response.status = CommitRequestStatus::Incomplete;
                }
            }
        }

        match response.status {
            CommitRequestStatus::Complete => {
                self.clear_staged_objects();
                response

            }
            CommitRequestStatus::Incomplete => {
                response
            }
        }
    }

    pub fn new() -> CommitManager {
        CommitManager {
            staged_holons: Vec::new(),
            index: Default::default(),
        }
    }

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, the function updates the index to allow the staged holon to be retrieved by key
    pub fn stage_new_holon(&mut self, holon: Holon) -> StagedReference {
        let rc_holon = Rc::new(RefCell::new(holon.clone()));
        self.staged_holons.push(Rc::clone(&rc_holon));
        let holon_index = self.staged_holons.len() - 1;
        let mut key: Option<MapString> = None;
        if let Some(the_key) = holon.get_key().unwrap() {
            key = Some(the_key.clone());
            self.index.insert(the_key, holon_index);
        }
        StagedReference { key, holon_index }
    }


    // Constructor function for creating StagedReference from an index into CommitManagers StagedHolons
    // pub fn get_reference_from_index(&self, index: MapInteger) -> Result<StagedReference, HolonError> {
    //
    //     // Ensure index is valid
    //     let holon_index = index.0 as usize;
    //     if holon_index < 0 || holon_index > self.staged_holons.len() {
    //         Err(HolonError::IndexOutOfRange(index.0.to_string()))
    //     }
        // let key = rc_holon.borrow().get_key()?;

    //     Ok(StagedReference { key, holon_index })
    // }

    pub fn clone_holon(
        &mut self,
        context: &HolonsContext,
        existing_holon: &mut SmartReference,
    ) -> Result<StagedReference, HolonError> {
        // Create a new empty Holon
        let mut holon = Holon::new();

        // Add the new holon into the CommitManager's staged_holons list, remembering its index
        let index = self.staged_holons.len();
        self.staged_holons
            .push(Rc::new(RefCell::new(holon.clone())));

        // Return a staged reference to the staged holon
        let staged_reference = StagedReference {
            key: existing_holon.key.clone(),
            holon_index: index,
        };

        // Copy the existing holon's PropertyMap into the new holon
        holon.property_map = existing_holon.get_property_map(context)?;

        // Iterate through existing holon's RelationshipMap
        // For each RelationshipTarget, create a new StagedCollection in the new holon, from the existing holon's SmartCollection
        let existing_relationship_map = existing_holon.get_relationship_map(context)?;
        holon.relationship_map = RelationshipMap::new();
        for (relationship_name, relationship_target) in existing_relationship_map.0 {
            let mut new_relationship_target = RelationshipTarget {
                editable: None,
                cursors: Vec::new(),
            };
            // for now populate 0th cursor
            new_relationship_target.stage_collection(
                staged_reference.clone_reference(),
                relationship_target.cursors[0].clone(),
            );

            holon
                .relationship_map
                .0
                .insert(relationship_name, new_relationship_target);
        }

        Ok(staged_reference)
    }

    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    /// specified key.
    /// NOTE: Only staged holons are searched and some holon types do not define unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    pub fn get_holon_by_key(&self, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        if let Some(&index) = self.index.get(&key) {
            Some(Rc::clone(&self.staged_holons[index]))
        } else {
            None
        }
    }

    // pub fn get_staged_reference(&self, index:StagedIndex)->Result<StagedReference, HolonError> {
    //     self.staged_holons.get(index.0 as usize)
    // }
    pub fn get_holon(&self, reference: &StagedReference) -> Result<Ref<Holon>, HolonError> {
        let holons = &self.staged_holons;
        let holon_ref = holons
            .get(reference.holon_index)
            .ok_or_else(|| HolonError::IndexOutOfRange(reference.holon_index.to_string()))?;

        match holon_ref.try_borrow() {
            Ok(holon) => Ok(holon),
            Err(_) => Err(HolonError::FailedToBorrow(
                "Holon Reference from staged_holons vector".to_string(),
            )),
        }
    }
    pub fn get_mut_holon_by_index(
        &self,
        holon_index: StagedIndex,
    ) -> Result<RefMut<Holon>, HolonError> {
        return if let Some(holon) = self.staged_holons.get(holon_index.0 as usize) {
            if let Ok(holon_ref) = holon.try_borrow_mut() {
                Ok(holon_ref)
            } else {
                Err(HolonError::FailedToBorrow(
                    "for StagedReference".to_string(),
                ))
            }
        } else {
            Err(HolonError::InvalidHolonReference(
                "Invalid holon index".to_string(),
            ))
        };
    }
    pub fn get_mut_holon(
        &self,
        staged_reference: &StagedReference,
    ) -> Result<RefMut<Holon>, HolonError> {
        return if let Some(holon) = self.staged_holons.get(staged_reference.holon_index) {
            if let Ok(holon_ref) = holon.try_borrow_mut() {
                Ok(holon_ref)
            } else {
                Err(HolonError::FailedToBorrow(
                    "for StagedReference".to_string(),
                ))
            }
        } else {
            Err(HolonError::InvalidHolonReference(
                "Invalid holon index".to_string(),
            ))
        };
    }
    pub fn clear_staged_objects(&mut self) {
        self.staged_holons.clear();
        self.index.clear();
    }


    /// Stages a new version of an existing holon for update, retaining the linkage to the holon version it is derived from by populating its (new) predecessor field existing_holon value provided.
    pub fn edit_holon(
        &mut self,
        context: &HolonsContext,
        existing_holon: &mut SmartReference,
    ) -> Result<StagedReference, HolonError> {
        // Create empty Holon
        let mut holon = Holon::new();

        // Set state to fetched, set predecessor to existing_holon
        holon.state = HolonState::Fetched;
        holon.predecessor = Some(existing_holon.clone_reference());

        // Add the new holon into the CommitManager's staged_holons list, remembering its index
        let index = self.staged_holons.len();
        self.staged_holons
            .push(Rc::new(RefCell::new(holon.clone())));

        // Return a staged reference to the staged holon
        let staged_reference = StagedReference {
            key: existing_holon.key.clone(),
            holon_index: index,
        };

        // Copy the existing holon's PropertyMap into the new holon
        holon.property_map = existing_holon.get_property_map(context)?;

        // Iterate through existing holon's RelationshipMap
        // For each RelationshipTarget, create a new StagedCollection in the new holon, from the existing holon's SmartCollection
        let existing_relationship_map = existing_holon.get_relationship_map(context)?;
        holon.relationship_map = RelationshipMap::new();
        for (relationship_name, relationship_target) in existing_relationship_map.0 {
            let mut new_relationship_target = RelationshipTarget {
                editable: None,
                cursors: Vec::new(),
            };
            // *Note: temp implementation, populate 0th cursor. TODO: set strategy for how to determine which SmartCollection (cursor) to choose
            new_relationship_target.stage_collection(
                staged_reference.clone_reference(),
                relationship_target.cursors[0].clone(),
            );

            holon
                .relationship_map
                .0
                .insert(relationship_name, new_relationship_target);
        }

        Ok(staged_reference)
    }
}


