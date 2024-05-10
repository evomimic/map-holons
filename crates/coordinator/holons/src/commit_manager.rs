use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;
use hdk::prelude::info;

// use crate::cache_manager::HolonCacheManager;
use crate::context::HolonsContext;
use crate::holon::{Holon, HolonState};
use crate::holon_error::HolonError;
use crate::relationship::RelationshipMap;
use crate::relationship::RelationshipTarget;
use crate::smart_reference::SmartReference;
use crate::staged_reference::StagedReference;
use shared_types_holon::{MapInteger, MapString};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitManager {
    pub staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    pub index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitResponse {
    pub status: CommitRequestStatus,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CommitRequestStatus {
    Success,
    Error(Vec<HolonError>),
}

impl CommitManager {
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

        // Add the new holon into the CommitManager's staged_holons list, remebering its index
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
    /// This function iterates through the staged holons, committing each one.
    /// Any errors encountered are accumulated in an errors vector.
    /// Once all staged holons have been committed (successfully or not), the staged_holons vector and the
    /// index are cleared.
    ///
    /// The CommitResponse returned by this function returns Success if no errors were encountered.
    /// Otherwise, the CommitResponse will contain an error status and the vector of errors.
    pub fn commit(&mut self, context: &HolonsContext) -> CommitResponse {
        info!("Entering commit...");
        let mut errors: Vec<HolonError> = Vec::new();
        for rc_holon in self.staged_holons.clone() {
            // Dereference the Rc and clone the RefCell to access the object
            // let mut holon = rc_holon.borrow_mut(); // Clone the object inside RefCell
            // let outcome = holon.commit(context);
            //
            // if let Err(e) = outcome {
            //     errors.push(e)
            // };
            let outcome = rc_holon.borrow_mut().clone().commit(context);
            if let Err(e) = outcome.clone() {
                errors.push(e)
            };
        }

        self.clear_staged_objects();

        let commit_response = if errors.is_empty() {
            CommitResponse {
                status: CommitRequestStatus::Success,
            }
        } else {
            CommitResponse {
                status: CommitRequestStatus::Error(errors),
            }
        };
        commit_response
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

        // Add the new holon into the CommitManager's staged_holons list, remebering its index
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
