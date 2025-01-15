use crate::{Holon, HolonError};
use shared_types_holon::HolonId;
use std::fmt::Debug;

pub trait HolonResolver: Debug {
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError>;
}
