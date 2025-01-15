use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use crate::HolonError;
use shared_types_holon::MapString;

pub trait TransientCollectionBehavior {
    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError>;
    fn add_reference(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holon_ref: HolonReference,
    ) -> Result<(), HolonError>;
    fn add_references(
        &mut self,
        context: &dyn HolonsContextBehavior,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;
}
