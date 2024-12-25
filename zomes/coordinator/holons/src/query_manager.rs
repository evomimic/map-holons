use std::{cell::RefCell, rc::Rc};

use shared_types_holon::MapString;

use crate::{
    holon::Holon,
    holon_error::HolonError,
    space_manager::{HolonSpaceManager, HolonStagingBehavior},
    staged_reference::StagedReference,
};

#[allow(dead_code)]
pub struct QueryManager {
    space_manager: Rc<RefCell<HolonSpaceManager>>, // Shared ownership
}

#[allow(dead_code)]
trait HolonSpaceFacade {
    fn get_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError>;
    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError>;
}

impl QueryManager {
    pub fn new(space_manager: Rc<RefCell<HolonSpaceManager>>) -> QueryManager {
        QueryManager { space_manager }
    }
}

impl HolonSpaceFacade for QueryManager {
    fn get_holon_by_key(&self, key: MapString) -> Result<StagedReference, HolonError> {
        let manager = self.space_manager.borrow();
        manager.get_holon_by_key(key)
    }

    fn stage_new_holon(&self, holon: Holon) -> Result<StagedReference, HolonError> {
        let manager = self.space_manager.borrow();
        manager.stage_new_holon(holon)
    }
}
