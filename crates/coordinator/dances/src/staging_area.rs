use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use hdk::prelude::*;
use holons::commit_manager::{CommitManager, StagedIndex};
use holons::holon::{Holon, HolonState};
use holons::holon_error::HolonError;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapString};

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct StagingArea {
    pub staged_holons:Vec<Holon>, // Contains all holons staged for commit
    index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}

impl StagingArea {

    pub fn new()->Self {
        StagingArea {
            staged_holons: Vec::new(),
            index: BTreeMap::new(),
        }
    }

    pub fn get_holon_mut(&mut self, staged_index: usize) -> Result<&mut Holon, HolonError> {
        if staged_index < self.staged_holons.len() {
            Ok(&mut self.staged_holons[staged_index])
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }

    // Function to create StagingArea from CommitManager
    pub fn from_commit_manager(commit_manager: &CommitManager) -> Self {
        let staged_holons: Vec<Holon> = commit_manager.staged_holons.iter().map(|holon_rc| holon_rc.borrow().clone()).collect();
        StagingArea {
            staged_holons,
            index: commit_manager.keyed_index.clone(),
        }

    }

    // Function to create CommitManager from StagingArea
    pub fn to_commit_manager(&self) -> CommitManager {
        let staged_holons: Vec<Rc<RefCell<Holon>>> = self.staged_holons.iter().map(|holon| Rc::new(RefCell::new(holon.clone()))).collect();
        CommitManager {
            staged_holons,
            keyed_index: self.index.clone(),
        }
    }
}