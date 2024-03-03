use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::holon_errors::HolonError;
use shared_types_holon::MapString;
use crate::context::HolonsContext;
use crate::holon::{Holon};
use crate::staged_reference::StagedReference;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CommitManager {
    staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    index: HashMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
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
    /// Stages the provided holon and returns a reference-counted reference to it
    /// If the holon has a key, the function updates the index to allow the staged holon to be retrieved by key
    pub fn new()->CommitManager {
        CommitManager {
            staged_holons: Vec::new(),
            index: Default::default(),
        }
    }
    pub fn stage_holon(&mut self, context: &HolonsContext, holon: Holon) -> StagedReference {
        let rc_holon = Rc::new(RefCell::new(holon.clone())); // Cloning the object for Rc
        self.staged_holons.push(Rc::clone(&rc_holon));
        let mut key: Option<MapString> = None;
        if let Some(the_key) = holon.get_key().unwrap() {
            key = Some(the_key.clone());
            self.index.insert(the_key, self.staged_holons.len() - 1);
        }
        StagedReference {
            key,
            rc_holon
        }
    }

    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the specified key
    /// NOTE: Only staged holons are searched and some holon types do not defined unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    pub fn get_holon_by_key(&self, context: &HolonsContext, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        if let Some(&index) = self.index.get(&key) {
            Some(Rc::clone(&self.staged_holons[index]))
        } else {
            None
        }
    }
    pub fn clear_staged_objects(&mut self) {
        self.staged_holons.clear();
        self.index.clear();
    }
    pub fn commit(&mut self, context: &HolonsContext) -> CommitResponse {
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
// pub fn commit(context: &HolonsContext, commit_manager: &mut CommitManager) -> CommitResponse {
//     let mut errors: Vec<HolonError> = Vec::new();
//     for rc_holon in &commit_manager.staged_holons {
//         // Dereference the Rc and clone the RefCell to access the object
//         let holon = rc_holon.borrow().clone(); // Clone the object inside RefCell
//         let outcome = holon.commit();
//
//         if let Err(e) = outcome { errors.push(e) };
//     }
//
//    commit_manager.clear_staged_objects();
//
//     let commit_response = if errors.is_empty() {
//         CommitResponse {
//             status: CommitRequestStatus::Success,
//         }
//     } else {
//         CommitResponse {
//             status: CommitRequestStatus::Error(errors),
//         }
//     };
//     commit_response
// }

