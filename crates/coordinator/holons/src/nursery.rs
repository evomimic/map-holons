use std::{cell::{Ref, RefCell, RefMut}, collections::BTreeMap, rc::Rc};
use shared_types_holon::MapString;
use crate::{holon::Holon, holon_error::HolonError};


#[derive(Debug, Clone,PartialEq, Eq)]
pub struct Nursery {
    staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
    keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}  

pub trait HolonsNursery {
    fn new() -> Nursery {
        Nursery {
            staged_holons: Vec::new(),
            keyed_index: BTreeMap::new()
        }
    }
    fn new_from_stage(staged_holons: Vec<Rc<RefCell<Holon>>>, keyed_index: BTreeMap<MapString, usize>) -> Nursery {
        Nursery {
            staged_holons,
            keyed_index
        }
    }

    fn add_new_holon(nursery: &mut Nursery, holon: Holon) -> Result<usize, HolonError>{
        let holon_index = nursery.staged_holons.len() - 1;
        let holon_key: Option<MapString> = holon.get_key()?;
        if let Some(key) = holon_key.clone() {
            nursery.keyed_index.insert(key.clone(), holon_index);  
        }
        let holon_rc = Rc::new(RefCell::new(holon));
        nursery.staged_holons.push(Rc::clone(&holon_rc));
        Ok(holon_index)
    }
    fn get_holon_by_key(nursery: &Nursery, key: MapString) -> Option<Rc<RefCell<Holon>>> {
        if let Some(index) = nursery.keyed_index.get(&key) {
            Some(Rc::clone(&nursery.staged_holons[*index]))
        } else {
            None
        }
    }
    fn get_holon_by_index(nursery: &Nursery, index: usize) -> Result<Ref<Holon>, HolonError> {
        if index < nursery.staged_holons.len() {
            let holon_ref= &nursery.staged_holons[index];
            match holon_ref.try_borrow() {
                Ok(holon) => Ok(holon),
                Err(_) => Err(HolonError::FailedToBorrow("Failed to borrow holon".into()))
            }
        } else {
            Err(HolonError::IndexOutOfRange(index.to_string()))?
        }
    }
    fn get_mut_holon_by_index(nursery: &Nursery, index: usize) -> Result<RefMut<Holon>, HolonError> {
        if index < nursery.staged_holons.len() {
            let holon_ref= &nursery.staged_holons[index];
            match holon_ref.try_borrow_mut() {
                Ok(holon) => Ok(holon),
                Err(_) => Err(HolonError::FailedToBorrow("Failed to borrow mutable holon".into()))
            }
        } else {
            Err(HolonError::IndexOutOfRange(index.to_string()))?
        }
    }
    fn get_holon_stage(nursery: &Nursery) -> Vec<Rc<RefCell<Holon>>> {
        nursery.staged_holons.clone()
    }

    fn clear(nursery: &mut Nursery) {
        nursery.staged_holons.clear();
        nursery.keyed_index.clear();
    }
}
