use crate::core_shared_objects::HolonError;
use shared_types_holon::{MapInteger, MapString};
use std::fmt::Debug;

use super::{HolonReference, HolonsContextBehavior};
// use crate::HolonReference;

pub trait HolonCollectionApi: Debug {
    fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn add_reference_with_key(
        &mut self,
        key: Option<&MapString>,
        reference: &HolonReference,
    ) -> Result<(), HolonError>;

    fn get_count(&self) -> MapInteger;

    fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError>;

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError>;

    fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;
}
