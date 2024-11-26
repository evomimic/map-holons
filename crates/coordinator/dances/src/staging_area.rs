use hdk::prelude::*;
use holons::helpers::summarize_holons;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use shared_types_holon::MapString;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct StagingArea {
    staged_holons: Vec<Holon>,         // Contains all holons staged for commit
    index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}


impl StagingArea {
    pub fn empty() -> Self {
        StagingArea { staged_holons: Vec::new(), index: BTreeMap::new() }
    }

    // Function to create StagingArea from the holon references and index
    pub fn new_from_references(rc_holons:Vec<Rc<RefCell<Holon>>>, index:BTreeMap<MapString, usize>) -> Self {
        let staged_holons: Vec<Holon> = rc_holons.iter().map(|holon_rc| holon_rc.borrow().clone()).collect();
        StagingArea { staged_holons, index }
    }

    pub fn get_holon(&self, staged_index: usize) -> Result<Holon, HolonError> {
        if staged_index < self.staged_holons.len() {
            Ok(self.staged_holons[staged_index].clone())
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }

    pub fn get_holon_mut(&mut self, staged_index: usize) -> Result<&mut Holon, HolonError> {
        if staged_index < self.staged_holons.len() {
            Ok(&mut self.staged_holons[staged_index])
        } else {
            Err(HolonError::IndexOutOfRange(staged_index.to_string()))
        }
    }

    pub fn get_staged_holons(&self) -> Vec<Holon> {
        self.staged_holons.clone()
    }

    pub fn get_staged_rc_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.staged_holons.iter().map(|holon| Rc::new(RefCell::new(holon.clone()))).collect()
    }

    pub fn get_staged_index(&self) -> BTreeMap<MapString, usize> {
        self.index.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.staged_holons.is_empty()
    }

    //Method to summarize the StagingArea into a String for logging purposes
    pub fn summarize(&self) -> String {
        format!(
            "\n   {:?} holon(s) in staging area: {{ Staged Holons {} }}",
            &self.get_staged_holons().len(),
            summarize_holons(&self.staged_holons)
        )
    }
}
