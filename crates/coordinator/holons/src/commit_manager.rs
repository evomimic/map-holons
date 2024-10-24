use hdk::prelude::*;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::rc::Rc;

// use crate::cache_manager::HolonCacheManager;
use crate::context::HolonsContext;
use crate::holon::{Holon, HolonState};
use crate::holon_collection::CollectionState;
use crate::holon_error::HolonError;
use crate::json_adapter::as_json;
use crate::staged_reference::StagedReference;
use shared_types_holon::{LocalId, MapInteger, MapString};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitManager {
    pub staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    pub keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}
/// a StagedIndex identifies a StagedHolon by its position within the staged_holons vector
pub type StagedIndex = usize;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitResponse {
    pub status: CommitRequestStatus,
    pub commits_attempted: MapInteger,
    // could the order of these Vecs cause challenges with identifying Holons in relation to their staged_index?
    pub saved_holons: Vec<Holon>, // should this be index? where else used?
    pub abandoned_holons: Vec<Holon>, // should this be index?
}
impl CommitResponse {
    /// This helper method returns true if the supplied CommitResponse indicates that the commit
    /// was complete and false otherwise
    pub fn is_complete(&self) -> bool {
        match self.status {
            CommitRequestStatus::Complete => true,
            CommitRequestStatus::Incomplete => false,
        }
    }
    pub(crate) fn find_local_id_by_key(&self, k: &MapString) -> Result<LocalId, HolonError> {
        for holon in &self.saved_holons {
            if let Some(key) = holon.get_key()? {
                // Check if the key matches the given key `k`
                if &key == k {
                    // Return the LocalId if a match is found
                    return holon.get_local_id();
                }
            }
        }
        // Return an error if no matching Holon is found
        Err(HolonError::HolonNotFound(format!(
            "No saved Holon with key {:?} was found in commit response",
            k.to_string(),
        )))
    }
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
    pub fn new() -> CommitManager {
        CommitManager { staged_holons: Vec::new(), keyed_index: Default::default() }
    }

    pub fn clear_staged_objects(&mut self) {
        self.staged_holons.clear();
        self.keyed_index.clear();
    }

    /// This function attempts to persist the state of all staged_holons AND their relationships.
    ///
    /// The commit is performed in two passes: (1) staged_holons, (2) their relationships.
    ///
    /// In the first pass,
    /// * if a staged_holon commit succeeds,
    ///     * change holon's state to `Saved`
    ///     * populate holon's saved_node
    ///     * add the holon to the saved_nodes vector in the CommitResponse
    /// * if a staged_holon commit fails,
    ///     * leave holon's state unchanged
    ///     * leave holon's saved_node unpopulated
    ///     * push the error into the holon's errors vector
    ///     * do NOT add the holon to the saved_nodes vector in the CommitResponse
    ///
    /// If ANY staged_holon commit fails:
    /// * The 2nd pass (to commit the staged_holon's relationships) is SKIPPED
    /// * the overall return status in the CommitResponse is set to `Incomplete`
    /// * the function returns.
    ///
    /// Otherwise, the 2nd pass is performed.
    /// * If ANY attempt to add a relationship generates an Error, the error is pushed into the
    /// source holon's `errors` vector and processing continues
    ///
    ///
    /// If relationship commits succeed for ALL staged_holons,
    ///     * The commit_manager's staged_holons are cleared
    ///     * The Commit Response returns a `Complete` status
    ///
    /// NOTE: The CommitResponse returns clones of any successfully
    /// committed holons, even if the response status is `Incomplete`.
    ///

    pub fn commit(context: &HolonsContext) -> CommitResponse {
        debug!("Entering commit...");

        // Initialize the request_status to Complete, assuming all commits will succeed
        // If any commit errors are encountered, reset request_status to `Incomplete`
        let mut response = CommitResponse {
            status: CommitRequestStatus::Complete,
            commits_attempted: MapInteger(
                context.commit_manager.borrow().staged_holons.len() as i64
            ),
            saved_holons: Vec::new(),
            abandoned_holons: Vec::new(),
        };

        // FIRST PASS: Commit Staged Holons
        {
            info!("\n\nStarting FIRST PASS... commit staged_holons...");
            let commit_manager = context.commit_manager.borrow();
            for rc_holon in commit_manager.staged_holons.clone() {
                trace!(" In commit_manager... getting ready to call commit()");
                let outcome = rc_holon.borrow_mut().commit();
                match outcome {
                    Ok(holon) => match holon.state {
                        HolonState::Abandoned => {
                            // should these be index?
                            //if !response.abandoned_holons.contains(&holon) {
                            response.abandoned_holons.push(holon);
                            //}
                        }
                        HolonState::Saved => {
                            response.saved_holons.push(holon);
                        }
                        _ => {}
                    },
                    Err(error) => {
                        response.status = CommitRequestStatus::Incomplete;
                        warn!("Attempt to commit holon returned error: {:?}", error.to_string());
                    }
                }
            }
        }

        if response.status == CommitRequestStatus::Incomplete {
            return response;
        }

        // SECOND PASS: Commit relationships
        {
            info!("\n\nStarting 2ND PASS... commit relationships for the saved staged_holons...");
            let commit_manager = context.commit_manager.borrow();
            for rc_holon in commit_manager.staged_holons.clone() {
                let outcome = rc_holon.borrow_mut().commit_relationships(context);
                if let Err(error) = outcome {
                    rc_holon.borrow_mut().errors.push(error.clone());
                    response.status = CommitRequestStatus::Incomplete;
                    warn!("Attempt to commit relationship returned error: {:?}", error.to_string());
                }
            }
        }

        info!("\n\n VVVVVVVVVVV   SAVED HOLONS AFTER COMMIT VVVVVVVVV\n");

        for saved_holon in &response.saved_holons {
            info!("{}", as_json(saved_holon));
        }

        // Handle the final status of the commit process

        {
            let mut commit_manager = context.commit_manager.borrow_mut();
            if response.status == CommitRequestStatus::Complete {
                commit_manager.clear_staged_objects();
                response
            } else {
                response
            }
        }
    }

    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    /// specified key.
    /// NOTE: Only staged holons are searched and some holon types do not define unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    pub fn get_holon_by_key(&self, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        if let Some(&index) = self.keyed_index.get(&key) {
            Some(Rc::clone(&self.staged_holons[index]))
        } else {
            None
        }
    }

    /// Private helper function the encapsulates the logic for getting a mutable reference to a
    /// holon from a Staged
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

    /// Private helper function that encapsulates the logic for getting a mutable reference to a
    /// holon from a StagedReference
    fn get_mut_holon_internal(
        &self,
        holon_index: Option<StagedIndex>,
    ) -> Result<RefMut<Holon>, HolonError> {
        if let Some(index) = holon_index {
            if let Some(holon) = self.staged_holons.get(index) {
                return if let Ok(holon_refcell) = holon.try_borrow_mut() {
                    Ok(holon_refcell)
                } else {
                    Err(HolonError::FailedToBorrow("for StagedReference".to_string()))
                };
            }
        }
        Err(HolonError::InvalidHolonReference("Invalid holon index".to_string()))
    }

    pub fn get_mut_holon(
        &self,
        staged_reference: &StagedReference,
    ) -> Result<RefMut<Holon>, HolonError> {
        self.get_mut_holon_internal(Some(staged_reference.holon_index))
    }

    pub fn get_mut_holon_by_index(
        &self,
        holon_index: StagedIndex,
    ) -> Result<RefMut<Holon>, HolonError> {
        self.get_mut_holon_internal(Some(holon_index))
    }

    // pub fn get_mut_holon_by_index(
    //     &self,
    //     holon_index: StagedIndex,
    // ) -> Result<RefMut<Holon>, HolonError> {
    //     return if let Some(holon) = self.staged_holons.get(holon_index.0 as usize) {
    //         if let Ok(holon_ref) = holon.try_borrow_mut() {
    //             Ok(holon_ref)
    //         } else {
    //             Err(HolonError::FailedToBorrow(
    //                 "for StagedReference".to_string(),
    //             ))
    //         }
    //     } else {
    //         Err(HolonError::InvalidHolonReference(
    //             "Invalid holon index".to_string(),
    //         ))
    //     };
    // }
    // pub fn get_mut_holon(
    //     &self,
    //     staged_reference: &StagedReference,
    // ) -> Result<RefMut<Holon>, HolonError> {
    //     return if let Some(holon) = self.staged_holons.get(staged_reference.holon_index) {
    //         if let Ok(holon_ref) = holon.try_borrow_mut() {
    //             Ok(holon_ref)
    //         } else {
    //             Err(HolonError::FailedToBorrow(
    //                 "for StagedReference".to_string(),
    //             ))
    //         }
    //     } else {
    //         Err(HolonError::InvalidHolonReference(
    //             "Invalid holon index".to_string(),
    //         ))
    //     };
    // }

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the CommitManager's keyed_index to allow the staged holon
    /// to be retrieved by key

    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, update the CommitManager's keyed_index to allow the staged holon
    /// to be retrieved by key
    pub fn stage_new_holon(&mut self, holon: Holon) -> Result<StagedReference, HolonError> {
        let mut cloned_holon = holon.clone();
        for (_relationship_name, collection) in cloned_holon.relationship_map.0.iter_mut() {
            let state = collection.get_state();
            match state {
                CollectionState::Fetched => {
                    collection.to_staged()?;
                }
                CollectionState::Staged => {}
                CollectionState::Saved | CollectionState::Abandoned => {
                    return Err(HolonError::InvalidParameter(format!(
                        "CollectionState::{:?}",
                        state
                    )))
                }
            }
        }

        let rc_holon = Rc::new(RefCell::new(cloned_holon));
        self.staged_holons.push(Rc::clone(&rc_holon));
        trace!("Added to StagingArea, Holon: {:#?}", rc_holon);
        let holon_index = self.staged_holons.len() - 1;
        let holon_key: Option<MapString> = holon.get_key()?;
        if let Some(key) = holon_key.clone() {
            self.keyed_index.insert(key.clone(), holon_index);
        }
        trace!("Success! Holon staged, with key: {:?}, at index: {:?}", holon_key, holon_index);

        Ok(StagedReference { holon_index })
    }

    /// This function converts a StagedIndex into a StagedReference
    /// Returns HolonError::IndexOutOfRange if index is out range for staged_holons vector
    /// Returns HolonError::NotAccessible if the staged holon is in an Abandoned state
    /// TODO: The latter is only reliable if staged_holons is made private
    pub fn to_staged_reference(
        &self,
        staged_index: StagedIndex,
    ) -> Result<StagedReference, HolonError> {
        if let Some(staged_holon) = self.staged_holons.get(staged_index) {
            let holon = staged_holon.borrow();
            if let HolonState::Abandoned = holon.state {
                return Err(HolonError::NotAccessible(
                    "to_staged_reference".to_string(),
                    "Abandoned".to_string(),
                ));
            }
            Ok(StagedReference { holon_index: staged_index })
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }
}
