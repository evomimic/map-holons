use super::{HolonReference, HolonsContextBehavior};
use crate::core_shared_objects::holon::AccessType;
use base_types::{MapInteger, MapString};
use core_types::HolonError;
use std::fmt::Debug;
use tracing::warn;

pub trait HolonCollectionApi: Debug + Send + Sync {
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

    /// Adds references using precomputed keys, avoiding any key lookups during mutation.
    fn add_references_with_keys(
        &mut self,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;

    fn get_count(&self) -> MapInteger;

    fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError>;

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError>;

    fn remove_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    /// Removes references using precomputed keys, rebuilding the keyed index without holon lookups.
    fn remove_references_with_keys(
        &mut self,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;
}
