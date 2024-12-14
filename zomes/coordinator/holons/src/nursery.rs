use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use shared_types_holon::MapString;
use crate::{holon::Holon, holon_error::HolonError};


#[derive(Debug, Clone,PartialEq, Eq)]
pub struct Nursery {
     staged_holons: Vec<Rc<RefCell<Holon>>>, // Contains all holons staged for commit
     keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
}  

pub trait NurseryBehavior {
    
    /// This function finds and returns a shared reference (Rc<RefCell<Holon>>) to the staged holon matching the
    /// specified key.
    /// NOTE: Only staged holons are searched and some holon types do not define unique keys
    /// This means that:
    ///    (1) even if this function returns `None` a holon with the specified key may exist in the DHT
    ///    (2) There might be some holons staged for update that you cannot find by key
    ///
    fn get_holon_index_by_key(&self, key: MapString) ->   Result<usize, HolonError>;
    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError>;
    //fn get_mut_holon_by_index(&self, index: usize) -> Result<RefMut<Holon>, HolonError>;
    fn get_all_holons(&self) -> Vec<Rc<RefCell<Holon>>>;
    fn get_stage_key_index(&self) -> BTreeMap<MapString, usize>;
}


impl NurseryBehavior for Nursery {

    fn get_holon_index_by_key(&self, key: MapString) -> Result<usize, HolonError> { //Option<Rc<RefCell<Holon>>> {
        if let Some(index) = self.keyed_index.get(&key) {
            Ok(*index)
            //Some(Rc::clone(&self.staged_holons[*index]))
        } else {
            Err(HolonError::HolonNotFound(key.to_string()))?
        }
    }

    fn get_holon_by_index(&self, index: usize) -> Result<Rc<RefCell<Holon>>, HolonError> {
        if index < self.staged_holons.len() {
            let holon_ref= &self.staged_holons[index];
           // match holon_ref.try_borrow() {
           ////     Ok(holon) => Ok(holon),
              //  Err(_) => Err(HolonError::FailedToBorrow("Failed to borrow holon".into()))
            //}
            Ok(Rc::clone(holon_ref))
         } else {
             Err(HolonError::IndexOutOfRange(index.to_string()))?
         }
    }

    //fn get_mut_holon_by_index(&self, index: usize) -> Result<RefMut<Holon>, HolonError>{        todo!()
    //}

    fn get_all_holons(&self) -> Vec<Rc<RefCell<Holon>>> {
        self.staged_holons.clone()
    }

    fn get_stage_key_index(&self) -> BTreeMap<MapString, usize> {
        self.keyed_index.clone()
    }
}

impl Nursery {
    pub fn new() -> Nursery {
        Nursery {
            staged_holons: Vec::new(),
            keyed_index: BTreeMap::new()
        }
    }
    pub fn new_from_stage(staged_holons: Vec<Rc<RefCell<Holon>>>, keyed_index: BTreeMap<MapString, usize>) -> Nursery {
        Nursery {
            staged_holons,
            keyed_index
        }
    }
    pub fn add_new_holon(&mut self, holon: Holon) -> Result<usize, HolonError> {
        let rc_holon = Rc::new(RefCell::new(holon.clone()));
        self.staged_holons.push(Rc::clone(&rc_holon));
        let holon_index = &self.staged_holons.len() - 1;
        let holon_key: Option<MapString> = holon.get_key()?;
        if let Some(key) = holon_key.clone() {
            self.keyed_index.insert(key.clone(), holon_index);  
        }
        Ok(holon_index)
    }
    pub fn clear_stage(&mut self) {
        self.staged_holons.clear();
        self.keyed_index.clear();
    }
}
