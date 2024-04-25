use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap};
use std::rc::Rc;

use crate::holon_errors::HolonError;
use shared_types_holon::MapString;
use crate::holon::{Holon};
use crate::staged_reference::StagedReference;

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

    pub fn new()->CommitManager {
        CommitManager {
            staged_holons: Vec::new(),
            index: Default::default(),
        }
    }



    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, the function updates the index to allow the staged holon to be retrieved by key
    pub fn stage_holon(&mut self, holon: Holon) -> StagedReference {
        let rc_holon = Rc::new(RefCell::new(holon.clone())); // Cloning the object for Rc
        self.staged_holons.push(Rc::clone(&rc_holon));
        let holon_index = self.staged_holons.len() - 1;
        let mut key: Option<MapString> = None;
        if let Some(the_key) = holon.get_key().unwrap() {
            key = Some(the_key.clone());
            self.index.insert(the_key, holon_index);
        }
        StagedReference {
            key,
            holon_index,
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
        if let Some(&index) = self.index.get(&key) {
            Some(Rc::clone(&self.staged_holons[index]))
        } else {
            None
        }
    }
    pub fn get_holon(&self, reference: &StagedReference) -> Result<Ref<Holon>, HolonError> {
        let holons = &self.staged_holons;
        let holon_ref = holons.get(reference.holon_index)
            .ok_or_else(|| HolonError::IndexOutOfRange(reference.holon_index.to_string()))?;

        match holon_ref.try_borrow() {
            Ok(holon) => Ok(holon),
            Err(_) => Err(HolonError::FailedToBorrow("Holon Reference from staged_holons vector".to_string()))
        }
    }

    pub fn get_mut_holon(&self, staged_reference: &StagedReference) -> Result<RefMut<Holon>, HolonError> {
        return if let Some(holon) = self.staged_holons.get(staged_reference.holon_index) {
            if let Ok(holon_ref) = holon.try_borrow_mut() {
                Ok(holon_ref)
            } else {
                Err(HolonError::FailedToBorrow("for StagedReference".to_string()))
            }
        } else {
            Err(HolonError::InvalidHolonReference("Invalid holon index".to_string()))
        }
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
    pub fn commit(&mut self) -> CommitResponse {
        let mut errors: Vec<HolonError> = Vec::new();
        for rc_holon in self.staged_holons.clone() {
            // Dereference the Rc and clone the RefCell to access the object
            let holon = rc_holon.borrow().clone(); // Clone the object inside RefCell
            let outcome = holon.commit();

            if let Err(e) = outcome { errors.push(e) };
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
}



