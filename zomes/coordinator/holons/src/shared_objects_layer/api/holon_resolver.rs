use crate::shared_objects_layer::{Holon, HolonError};
use shared_types_holon::LocalId;
use std::fmt::Debug;

pub trait HolonResolver: Debug {
    fn fetch_holon(&self, local_id: &LocalId) -> Result<Holon, HolonError>;
}
