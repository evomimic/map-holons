use crate::{Holon, HolonError};
use shared_types_holon::{HolonId, MapString};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub trait StateMobility {
    fn export_staged_holons(&self) -> Vec<Rc<RefCell<Holon>>>;
    //(formerly named get_staged_holons)

    fn export_keyed_index(&self) -> BTreeMap<MapString, usize>;
    // (formerly named get_stage_key_index)

    fn fetch_holon(&self, id: HolonId) -> Result<Holon, HolonError>;
}
